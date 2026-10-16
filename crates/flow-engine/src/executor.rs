//! 后台执行器：逐动作执行、进度上报、取消、失败逆序回滚。
//!
//! 线程模型：调用方在后台线程运行 [`execute`]；UI 线程只通过 channel 收进度、
//! 通过 `AtomicBool` 发取消请求。全程无跨线程共享可变状态。

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crossbeam_channel::Sender;

use crate::actions::Action;
use crate::platform::{
    create_shortcut, delete_registry_key, delete_registry_value, remove_shortcut,
    shortcut_dir_path, write_registry_value,
};

/// 进度事件（每完成一个动作发送一次）。
#[derive(Debug, Clone)]
pub struct ProgressEvent {
    pub done: usize,
    pub total: usize,
    pub message: String,
}

#[derive(Debug, Clone)]
pub enum Outcome {
    Success,
    Failed { error: String, rolled_back: bool },
    Cancelled { rolled_back: bool },
}

impl Outcome {
    pub fn ok(&self) -> bool {
        matches!(self, Outcome::Success)
    }
}

/// 执行过程中创建的工件（回滚依据）。
#[derive(Debug, Clone, PartialEq)]
pub enum Created {
    File(PathBuf),
    Dir(PathBuf),
    Shortcut(PathBuf),
    RegValue { path: String, name: String },
    RegKey { path: String },
}

/// 执行结果：结局 + 创建工件 + 日志。
#[derive(Debug)]
pub struct ExecReport {
    pub outcome: Outcome,
    pub created: Vec<Created>,
    pub log: Vec<String>,
}

/// 运行动作序列（在后台线程调用）。
pub fn execute(actions: Vec<Action>, tx: Sender<ProgressEvent>, cancel: Arc<AtomicBool>) -> ExecReport {
    let total = actions.len();
    let mut created: Vec<Created> = Vec::new();
    let mut log: Vec<String> = Vec::new();
    let mut done = 0usize;

    for action in &actions {
        if cancel.load(Ordering::Relaxed) {
            let had_created = !created.is_empty();
            log.push("收到取消请求，开始回滚…".into());
            rollback(&mut created, &mut log);
            return ExecReport {
                outcome: Outcome::Cancelled {
                    rolled_back: had_created,
                },
                created,
                log,
            };
        }
        let desc = action.describe();
        match run_action(action, &mut created) {
            Ok(()) => {
                done += 1;
                log.push(format!("[OK] {desc}"));
                let _ = tx.send(ProgressEvent {
                    done,
                    total,
                    message: desc,
                });
            }
            Err(e) => {
                let had_created = !created.is_empty();
                log.push(format!("[ERR] {desc}：{e}"));
                log.push("开始回滚…".into());
                rollback(&mut created, &mut log);
                return ExecReport {
                    outcome: Outcome::Failed {
                        error: format!("{desc}：{e}"),
                        rolled_back: had_created,
                    },
                    created,
                    log,
                };
            }
        }
    }
    ExecReport {
        outcome: Outcome::Success,
        created,
        log,
    }
}

fn run_action(action: &Action, created: &mut Vec<Created>) -> Result<(), String> {
    match action {
        Action::CopyDir { from, to } => copy_dir(from, to, created),
        Action::CopyFile { from, to } => {
            ensure_parent(to)?;
            std::fs::copy(from, to).map_err(|e| format!("复制失败：{e}"))?;
            created.push(Created::File(to.clone()));
            Ok(())
        }
        Action::CreateDir { path } => {
            std::fs::create_dir_all(path).map_err(|e| format!("创建目录失败：{e}"))?;
            created.push(Created::Dir(path.clone()));
            Ok(())
        }
        Action::WriteFile { path, content } => {
            ensure_parent(path)?;
            std::fs::write(path, content).map_err(|e| format!("写入失败：{e}"))?;
            created.push(Created::File(path.clone()));
            Ok(())
        }
        Action::RemovePath { path } => remove_path(path),
        Action::RemoveIfEmpty { path } => {
            if path.exists() {
                std::fs::remove_dir(path).map_err(|e| format!("删除目录失败：{e}"))
            } else {
                Ok(())
            }
        }
        Action::CreateShortcut {
            name,
            target,
            args,
            icon,
            dir,
        } => {
            let dir_path = shortcut_dir_path(*dir).ok_or("当前平台不支持该快捷方式位置")?;
            let ext = if cfg!(windows) { "lnk" } else { "desktop" };
            let path = dir_path.join(format!("{name}.{ext}"));
            create_shortcut(&path, target, args.as_deref(), icon.as_deref())?;
            created.push(Created::Shortcut(path));
            Ok(())
        }
        Action::RemoveShortcut { name, dir } => {
            let dir_path = shortcut_dir_path(*dir).ok_or("当前平台不支持该快捷方式位置")?;
            let ext = if cfg!(windows) { "lnk" } else { "desktop" };
            let path = dir_path.join(format!("{name}.{ext}"));
            if path.exists() {
                remove_shortcut(&path)
            } else {
                log::info!("快捷方式不存在，跳过：{}", path.display());
                Ok(())
            }
        }
        Action::RegWrite { path, name, value } => {
            write_registry_value(path, name, value)?;
            created.push(Created::RegValue {
                path: path.clone(),
                name: name.clone(),
            });
            Ok(())
        }
        Action::RegDelete { path, name } => match name {
            Some(n) => delete_registry_value(path, n),
            None => {
                delete_registry_key(path)?;
                created.push(Created::RegKey { path: path.clone() });
                Ok(())
            }
        },
        Action::Run { cmd, args, wait } => {
            let mut c = std::process::Command::new(cmd);
            c.args(args);
            if *wait {
                let status = c
                    .status()
                    .map_err(|e| format!("启动进程失败：{e}"))?;
                if status.success() {
                    Ok(())
                } else {
                    Err(format!("进程退出码异常：{status}"))
                }
            } else {
                c.spawn().map_err(|e| format!("启动进程失败：{e}"))?;
                Ok(())
            }
        }
        Action::CopyUninstaller { from, to } => {
            ensure_parent(to)?;
            std::fs::copy(from, to).map_err(|e| format!("复制卸载器失败：{e}"))?;
            created.push(Created::File(to.clone()));
            Ok(())
        }
    }
}

fn ensure_parent(path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|e| format!("创建父目录失败：{e}"))?;
        }
    Ok(())
}

fn copy_dir(from: &Path, to: &Path, created: &mut Vec<Created>) -> Result<(), String> {
    if !from.exists() {
        return Err(format!("源目录不存在：{}", from.display()));
    }
    if !from.is_dir() {
        return Err(format!("源不是目录：{}", from.display()));
    }
    std::fs::create_dir_all(to).map_err(|e| format!("创建目标目录失败：{e}"))?;
    created.push(Created::Dir(to.to_path_buf()));
    for entry in std::fs::read_dir(from).map_err(|e| format!("读取目录失败：{e}"))? {
        let entry = entry.map_err(|e| format!("读取目录项失败：{e}"))?;
        let src = entry.path();
        let dst = to.join(entry.file_name());
        if src.is_dir() {
            copy_dir(&src, &dst, created)?;
        } else {
            std::fs::copy(&src, &dst).map_err(|e| format!("复制 {} 失败：{e}", src.display()))?;
            created.push(Created::File(dst));
        }
    }
    Ok(())
}

fn remove_path(path: &Path) -> Result<(), String> {
    if !path.exists() {
        log::info!("路径不存在，跳过：{}", path.display());
        return Ok(());
    }
    if path.is_dir() {
        std::fs::remove_dir_all(path).map_err(|e| format!("删除目录失败：{e}"))
    } else {
        std::fs::remove_file(path).map_err(|e| format!("删除文件失败：{e}"))
    }
}

/// 逆序回滚已创建工件（尽力而为，错误记日志不阻断）。
fn rollback(created: &mut Vec<Created>, log: &mut Vec<String>) {
    for item in created.iter().rev() {
        match item {
            Created::File(p) => {
                match std::fs::remove_file(p) {
                    Ok(()) => log.push(format!("回滚：删除文件 {}", p.display())),
                    Err(e) => log.push(format!("回滚：删除文件 {} 失败：{e}", p.display())),
                }
            }
            Created::Dir(p) => {
                // 仅删除空目录；非空由文件回滚后自然变空
                if let Ok(()) = std::fs::remove_dir(p) { log.push(format!("回滚：删除目录 {}", p.display())) }
            }
            Created::Shortcut(p) => {
                let _ = std::fs::remove_file(p);
                log.push(format!("回滚：删除快捷方式 {}", p.display()));
            }
            Created::RegValue { path, name } => {
                let _ = delete_registry_value(path, name);
                log.push(format!("回滚：删除注册表值 {path}\\{name}"));
            }
            Created::RegKey { path } => {
                let _ = delete_registry_key(path);
                log.push(format!("回滚：删除注册表键 {path}"));
            }
        }
    }
    created.clear();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicBool;

    fn tmpdir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "flow-engine-test-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn run(actions: Vec<Action>) -> ExecReport {
        let (tx, _rx) = crossbeam_channel::unbounded();
        execute(actions, tx, Arc::new(AtomicBool::new(false)))
    }

    #[test]
    fn copies_files_and_dirs() {
        let base = tmpdir("copy");
        let src = base.join("src");
        std::fs::create_dir_all(src.join("sub")).unwrap();
        std::fs::write(src.join("a.txt"), "a").unwrap();
        std::fs::write(src.join("sub").join("b.txt"), "b").unwrap();
        let dst = base.join("dst");
        let report = run(vec![Action::CopyDir {
            from: src,
            to: dst.clone(),
        }]);
        assert!(report.outcome.ok(), "{:?}", report.outcome);
        assert!(dst.join("a.txt").exists());
        assert!(dst.join("sub").join("b.txt").exists());
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn failure_rolls_back() {
        let base = tmpdir("rollback");
        let dir = base.join("created");
        let missing = base.join("no-such-source.txt");
        let report = run(vec![
            Action::CreateDir { path: dir.clone() },
            Action::CopyFile {
                from: missing.clone(),
                to: base.join("x.txt"),
            },
        ]);
        assert!(!report.outcome.ok());
        assert!(matches!(
            report.outcome,
            Outcome::Failed { rolled_back: true, .. }
        ));
        assert!(!dir.exists(), "失败后应回滚删除已创建目录");
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn cancel_rolls_back() {
        let base = tmpdir("cancel");
        let dir = base.join("created");
        let (tx, _rx) = crossbeam_channel::unbounded();
        let cancel = Arc::new(AtomicBool::new(true));
        let report = execute(
            vec![Action::CreateDir { path: dir.clone() }],
            tx,
            cancel,
        );
        assert!(matches!(report.outcome, Outcome::Cancelled { .. }));
        assert!(!dir.exists());
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn write_file_creates_parents() {
        let base = tmpdir("write");
        let target = base.join("a/b/c.txt");
        let report = run(vec![Action::WriteFile {
            path: target.clone(),
            content: "hi".into(),
        }]);
        assert!(report.outcome.ok());
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "hi");
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn remove_path_handles_missing() {
        let report = run(vec![Action::RemovePath {
            path: PathBuf::from("C:/definitely/not/exists-xyz"),
        }]);
        assert!(report.outcome.ok());
    }
}
