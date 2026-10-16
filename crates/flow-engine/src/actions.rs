//! 运行时动作集与计划构建：从清单 + 用户输入生成可执行动作序列。

use std::path::{Path, PathBuf};

use crate::platform::{resolve_install_dir, resolve_source, resolve_template, ResolveCtx};
use crate::spec::{FlowSpec, InstallSpec, ItemSpec, RegistryValueSpec, RunSpec, ShortcutSpec, UninstallSpec};

/// 快捷方式存放位置。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShortcutDir {
    Desktop,
    StartMenu,
}

/// 安装用户输入（来自 UI 选项页）。
#[derive(Debug, Clone, Default)]
pub struct InstallInputs {
    /// 用户指定的安装目录（覆盖清单默认值）。
    pub install_dir_override: Option<String>,
    /// 用户取消创建快捷方式（对应选项页复选框）。
    pub create_shortcut: bool,
    /// 与安装器同目录的卸载器路径（copy_uninstaller 时使用）。
    pub uninstaller_path: Option<PathBuf>,
    /// 用户选择的语言（id="locale" 控件），联动写入应用配置 / 注册表。
    pub locale: Option<String>,
}

/// 单个可执行动作。
#[derive(Debug, Clone)]
pub enum Action {
    CopyDir { from: PathBuf, to: PathBuf },
    CopyFile { from: PathBuf, to: PathBuf },
    CreateDir { path: PathBuf },
    WriteFile { path: PathBuf, content: String },
    /// 删除文件或整棵目录树（卸载用）。
    RemovePath { path: PathBuf },
    /// 仅当目录为空时删除（卸载用，配合自底向上的目录序列）。
    RemoveIfEmpty { path: PathBuf },
    CreateShortcut {
        name: String,
        target: PathBuf,
        args: Option<String>,
        icon: Option<PathBuf>,
        dir: ShortcutDir,
    },
    RemoveShortcut { name: String, dir: ShortcutDir },
    RegWrite { path: String, name: String, value: String },
    /// name 为 None 时删除整个注册表键子树。
    RegDelete { path: String, name: Option<String> },
    Run { cmd: PathBuf, args: Vec<String>, wait: bool },
    CopyUninstaller { from: PathBuf, to: PathBuf },
}

impl Action {
    /// 人类可读的动作描述（进度条消息 / dry-run 输出 / 日志）。
    pub fn describe(&self) -> String {
        match self {
            Action::CopyDir { from, to } => format!("复制目录 {} → {}", from.display(), to.display()),
            Action::CopyFile { from, to } => format!("复制文件 {} → {}", from.display(), to.display()),
            Action::CreateDir { path } => format!("创建目录 {}", path.display()),
            Action::WriteFile { path, .. } => format!("写入文件 {}", path.display()),
            Action::RemovePath { path } => format!("删除 {}", path.display()),
            Action::RemoveIfEmpty { path } => format!("删除空目录 {}", path.display()),
            Action::CreateShortcut { name, dir, .. } => {
                format!("创建快捷方式 \"{name}\"（{}）", shortcut_dir_label(*dir))
            }
            Action::RemoveShortcut { name, dir } => {
                format!("删除快捷方式 \"{name}\"（{}）", shortcut_dir_label(*dir))
            }
            Action::RegWrite { path, name, .. } => format!("写入注册表 {path}\\{name}"),
            Action::RegDelete { path, name } => match name {
                Some(n) => format!("删除注册表值 {path}\\{n}"),
                None => format!("删除注册表键 {path}"),
            },
            Action::Run { cmd, .. } => format!("运行 {}", cmd.display()),
            Action::CopyUninstaller { from, to } => {
                format!("部署卸载器 {} → {}", from.display(), to.display())
            }
        }
    }
}

pub fn shortcut_dir_label(d: ShortcutDir) -> &'static str {
    match d {
        ShortcutDir::Desktop => "桌面",
        ShortcutDir::StartMenu => "开始菜单",
    }
}

impl std::fmt::Display for Action {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.describe())
    }
}

/// 把注册表值中的路径占位符解析为字符串。
fn resolve_registry_value(v: &str, ctx: &ResolveCtx) -> Result<String, String> {
    resolve_template(v, ctx).map(|p| p.to_string_lossy().into_owned())
}

fn shortcut_dir_of(spec: &ShortcutSpec) -> ShortcutDir {
    match spec.dir.as_deref() {
        Some("start-menu") => ShortcutDir::StartMenu,
        _ => ShortcutDir::Desktop,
    }
}

fn append_shortcut_actions(
    actions: &mut Vec<Action>,
    shortcuts: &[ShortcutSpec],
    ctx: &ResolveCtx,
    enabled: bool,
) -> Result<(), String> {
    if !enabled {
        return Ok(());
    }
    for sc in shortcuts {
        let target = resolve_template(&sc.target, ctx)?;
        let icon = sc
            .icon
            .as_ref()
            .map(|i| resolve_source(i, ctx))
            .filter(|p| p.exists());
        actions.push(Action::CreateShortcut {
            name: sc.name.clone(),
            target,
            args: sc.args.clone(),
            icon,
            dir: shortcut_dir_of(sc),
        });
    }
    Ok(())
}

/// 从清单 + 用户输入构建安装动作序列，返回 `(actions, 解析后的安装目录)`。
pub fn build_install_actions(
    spec: &FlowSpec,
    root: &Path,
    inputs: &InstallInputs,
) -> Result<(Vec<Action>, PathBuf), String> {
    let install: &InstallSpec = spec.install.as_ref().ok_or("清单缺少 [install] 段")?;
    let dir_tpl = inputs
        .install_dir_override
        .clone()
        .unwrap_or_else(|| install.install_dir.clone());
    let install_dir = resolve_install_dir(&dir_tpl, root)?;
    let ctx = ResolveCtx::for_install(root, &install_dir);

    let mut actions: Vec<Action> = Vec::new();

    for item in &install.items {
        match item {
            ItemSpec::CopyDir { source, dest } => {
                let from = resolve_source(source, &ctx);
                let to = resolve_template(dest, &ctx)?;
                actions.push(Action::CopyDir { from, to });
            }
            ItemSpec::CopyFile { source, dest } => {
                let from = resolve_source(source, &ctx);
                let to = resolve_template(dest, &ctx)?;
                actions.push(Action::CopyFile { from, to });
            }
            ItemSpec::CreateDir { path } => {
                let p = resolve_template(path, &ctx)?;
                actions.push(Action::CreateDir { path: p });
            }
            ItemSpec::WriteFile { path, content } => {
                let p = resolve_template(path, &ctx)?;
                actions.push(Action::WriteFile {
                    path: p,
                    content: content.clone(),
                });
            }
        }
    }

    append_shortcut_actions(&mut actions, &install.shortcuts, &ctx, inputs.create_shortcut)?;

    for v in &install.registry.values {
        actions.push(Action::RegWrite {
            path: v.path.clone(),
            name: v.name.clone(),
            value: resolve_registry_value(&v.value, &ctx)?,
        });
    }

    if install.copy_uninstaller {
        if let Some(up) = &inputs.uninstaller_path {
            if up.exists() {
                let file_name = up
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "uninstaller".to_owned());
                let to = install_dir.join(file_name);
                actions.push(Action::CopyUninstaller { from: up.clone(), to });
            } else {
                log::warn!("copy_uninstaller=true 但卸载器 {} 不存在，跳过", up.display());
            }
        } else {
            log::warn!("copy_uninstaller=true 但未找到同目录卸载器，跳过");
        }
    }

    if let Some(run) = &install.run_after {
        append_run(&mut actions, run, &ctx)?;
    }

    // 多语言联动：把所选语言写入应用配置 / 注册表（被安装程序启动时读取）
    if let Some(locale) = &inputs.locale
        && let Some(loc) = spec.locale.as_ref()
    {
        if let Some(f) = &loc.write_file {
            let path = install_dir.join(resolve_template(f, &ctx)?);
            actions.push(Action::WriteFile {
                path,
                content: locale.clone(),
            });
        }
        if let Some(r) = &loc.write_registry {
            actions.push(Action::RegWrite {
                path: r.clone(),
                name: "Locale".to_owned(),
                value: locale.clone(),
            });
        }
    }

    Ok((actions, install_dir))
}

fn append_run(actions: &mut Vec<Action>, run: &RunSpec, ctx: &ResolveCtx) -> Result<(), String> {
    let cmd = resolve_source(&run.cmd, ctx);
    actions.push(Action::Run {
        cmd,
        args: run.args.clone(),
        wait: run.wait,
    });
    Ok(())
}

/// 从安装记录构建卸载动作序列（含用户数据保留过滤）。
pub fn build_uninstall_actions(record: &crate::manifest::InstallRecord) -> Result<Vec<Action>, String> {
    let install_dir = PathBuf::from(&record.install_dir);
    let ctx = ResolveCtx::for_uninstall(&install_dir);
    let keep: Vec<PathBuf> = record
        .uninstall
        .keep_user_data
        .iter()
        .map(|t| resolve_template(t, &ctx))
        .collect::<Result<_, _>>()?;
    let kept = |p: &Path| keep.iter().any(|k| p.starts_with(k));

    let mut actions: Vec<Action> = Vec::new();

    for f in &record.files {
        let p = PathBuf::from(f);
        if kept(&p) {
            continue;
        }
        actions.push(Action::RemovePath { path: p });
    }

    // 目录自底向上删除
    let mut dirs: Vec<PathBuf> = record.dirs.iter().map(PathBuf::from).collect();
    dirs.sort_by_key(|d| std::cmp::Reverse(d.components().count()));
    for d in dirs {
        if kept(&d) {
            continue;
        }
        actions.push(Action::RemoveIfEmpty { path: d });
    }

    if record.uninstall.remove_shortcuts {
        for sc in &record.shortcuts {
            actions.push(Action::RemoveShortcut {
                name: sc.name.clone(),
                dir: shortcut_dir_of(sc),
            });
        }
    }

    if record.uninstall.remove_registry {
        for v in &record.registry {
            actions.push(Action::RegDelete {
                path: v.path.clone(),
                name: Some(v.name.clone()),
            });
        }
        if record.uninstall_key {
            let key = format!(
                "Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\{}",
                record.app.id
            );
            actions.push(Action::RegDelete {
                path: format!("HKCU\\{key}"),
                name: None,
            });
        }
    }

    // 用户级 uninstall.json 副本
    let user_record = crate::platform::local_app_data().join(&record.app.id).join("uninstall.json");
    actions.push(Action::RemovePath { path: user_record });

    Ok(actions)
}

/// 由执行结果构建安装记录（安装成功后由调用方落盘）。
pub fn record_from_report(
    spec: &FlowSpec,
    install_dir: &Path,
    uninstall_spec: &UninstallSpec,
    created: &[crate::executor::Created],
    uninstaller_path: Option<&Path>,
    locale: Option<String>,
) -> crate::manifest::InstallRecord {
    use crate::executor::Created;
    use crate::manifest::InstallRecord;

    let mut files = Vec::new();
    let mut dirs = Vec::new();
    let mut shortcuts = Vec::new();
    let mut registry = Vec::new();

    for c in created {
        match c {
            Created::File(p) => files.push(p.to_string_lossy().into_owned()),
            Created::Dir(p) => dirs.push(p.to_string_lossy().into_owned()),
            Created::Shortcut(_) => {}
            Created::RegValue { path, name } => {
                let value = spec
                    .install
                    .as_ref()
                    .and_then(|i| {
                        i.registry
                            .values
                            .iter()
                            .find(|v| v.path == *path && v.name == *name)
                    })
                    .map(|v| v.value.clone())
                    .unwrap_or_default();
                registry.push(RegistryValueSpec {
                    path: path.clone(),
                    name: name.clone(),
                    value,
                });
            }
            Created::RegKey { .. } => {}
        }
    }
    let install = spec.install.as_ref();
    shortcuts.extend(
        install
            .map(|i| i.shortcuts.clone())
            .unwrap_or_default(),
    );
    let uninstall_key = install.map(|i| i.registry.uninstall_key).unwrap_or(false);
    let registry_default = install
        .map(|i| i.registry.values.clone())
        .unwrap_or_default();
    // 记录实际写入的注册表值
    if registry.is_empty() {
        registry = registry_default;
    }

    InstallRecord {
        app: spec.app.clone(),
        version: spec.app.version.clone(),
        install_dir: install_dir.to_string_lossy().into_owned(),
        installed_at: crate::manifest::InstallRecord::now(),
        files,
        dirs,
        shortcuts,
        registry,
        uninstall_key,
        uninstall: uninstall_spec.clone(),
        uninstaller_path: uninstaller_path.map(|p| p.to_string_lossy().into_owned()),
        flow: spec.clone(),
        locale,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec::FlowSpec;

    const MANIFEST: &str = r#"
[app]
name = "T"
id = "com.test.t"
version = "1.0"
[install]
install_dir = "$LOCALAPPDATA/T App"
copy_uninstaller = false
[[install.items]]
type = "copy-dir"
source = "payload"
dest = "$INSTALL_DIR/files"
[[install.shortcuts]]
name = "T 说明"
target = "$INSTALL_DIR/readme.txt"
dir = "desktop"
[install.registry]
uninstall_key = true
[[install.registry.values]]
path = "HKCU\\Software\\T"
name = "InstallDir"
value = "$INSTALL_DIR"
"#;

    #[test]
    fn builds_actions_with_resolution() {
        let spec = FlowSpec::from_toml_str(MANIFEST).unwrap();
        let root = Path::new("C:/fake-root");
        let inputs = InstallInputs {
            create_shortcut: true,
            ..Default::default()
        };
        let (actions, dir) = build_install_actions(&spec, root, &inputs).unwrap();
        assert!(dir.to_string_lossy().contains("T App"));
        assert_eq!(actions.len(), 3); // copy-dir + shortcut + regwrite
        match &actions[0] {
            Action::CopyDir { from, to } => {
                assert_eq!(*from, root.join("payload"));
                assert!(to.to_string_lossy().contains("T App"));
            }
            other => panic!("期望 CopyDir，得到 {other:?}"),
        }
        assert!(matches!(actions[1], Action::CreateShortcut { .. }));
        assert!(matches!(actions[2], Action::RegWrite { .. }));
    }

    #[test]
    fn shortcut_override_disables_shortcut() {
        let spec = FlowSpec::from_toml_str(MANIFEST).unwrap();
        let root = Path::new("C:/fake-root");
        let inputs = InstallInputs {
            create_shortcut: false,
            ..Default::default()
        };
        let (actions, _) = build_install_actions(&spec, root, &inputs).unwrap();
        assert!(!actions.iter().any(|a| matches!(a, Action::CreateShortcut { .. })));
    }

    #[test]
    fn install_dir_cannot_reference_itself() {
        let spec_src = r#"
[app]
name = "X"
id = "x"
version = "1"
[install]
install_dir = "$INSTALL_DIR/sub"
"#;
        let spec = FlowSpec::from_toml_str(spec_src).unwrap();
        let err = build_install_actions(&spec, Path::new("."), &InstallInputs::default()).unwrap_err();
        assert!(err.contains("自身"));
    }

    #[test]
    fn locale_linkage_appends_write_actions() {
        let spec_src = r#"
[app]
name = "X"
id = "x"
version = "1"
[install]
install_dir = "$HOME/x"
[locale]
write_file = "config/locale"
write_registry = "HKCU\\Software\\X"
"#;
        let spec = FlowSpec::from_toml_str(spec_src).unwrap();
        let root = Path::new("C:/fake-root");
        let inputs = InstallInputs {
            locale: Some("en-US".into()),
            ..Default::default()
        };
        let (actions, _) = build_install_actions(&spec, root, &inputs).unwrap();
        assert_eq!(actions.len(), 2, "WriteFile + RegWrite 两个联动动作");
        match &actions[0] {
            Action::WriteFile { path, content } => {
                assert!(path.to_string_lossy().ends_with("config/locale"));
                assert_eq!(content, "en-US");
            }
            other => panic!("期望 WriteFile，得到 {other:?}"),
        }
        match &actions[1] {
            Action::RegWrite { path, name, value } => {
                assert_eq!(path, "HKCU\\Software\\X");
                assert_eq!(name, "Locale");
                assert_eq!(value, "en-US");
            }
            other => panic!("期望 RegWrite，得到 {other:?}"),
        }

        // 未选语言 → 不追加联动动作
        let (actions, _) = build_install_actions(&spec, root, &InstallInputs::default()).unwrap();
        assert!(actions.is_empty());
    }
}
