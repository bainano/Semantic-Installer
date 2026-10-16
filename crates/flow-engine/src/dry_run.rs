//! `--dry-run`：无 GUI 的全流程模拟（安装 / 卸载），用于自动化验证与 CI。

use std::path::Path;

use crate::actions::{build_install_actions, build_uninstall_actions, Action, InstallInputs};
use crate::manifest::InstallRecord;
use crate::spec::FlowSpec;

/// 模拟安装：解析清单、构建动作、校验源存在性，输出人类可读的计划。
pub fn dry_run_install(spec: &FlowSpec, root: &Path, inputs: &InstallInputs) -> Result<Vec<String>, String> {
    let (actions, install_dir) = build_install_actions(spec, root, inputs)?;
    let mut lines = Vec::new();
    lines.push(format!("[DRY-RUN] 应用：{} {}", spec.app.name, spec.app.version));
    lines.push(format!("[DRY-RUN] 安装目录：{}", install_dir.display()));
    lines.push(format!("[DRY-RUN] 共 {} 个动作：", actions.len()));
    for a in &actions {
        match a {
            Action::CopyFile { from, .. }
            | Action::CopyDir { from, .. }
            | Action::CopyUninstaller { from, .. } => {
                let marker = if from.exists() { "  " } else { "!!" };
                lines.push(format!("  {marker} {a}"));
            }
            other => lines.push(format!("     {other}")),
        }
    }
    let missing = missing_sources(&actions);
    if !missing.is_empty() {
        return Err(format!(
            "[DRY-RUN] 存在缺失源：{}",
            missing
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join("；")
        ));
    }
    lines.push("[DRY-RUN] 源文件校验通过。".into());
    Ok(lines)
}

/// 模拟卸载：从安装记录构建动作并输出计划。
pub fn dry_run_uninstall(record: &InstallRecord) -> Result<Vec<String>, String> {
    let actions = build_uninstall_actions(record)?;
    let mut lines = Vec::new();
    lines.push(format!(
        "[DRY-RUN] 卸载：{} {}（安装于 {}）",
        record.app.name, record.version, record.install_dir
    ));
    lines.push(format!("[DRY-RUN] 共 {} 个动作：", actions.len()));
    for a in &actions {
        lines.push(format!("  {a}"));
    }
    if !record.uninstall.keep_user_data.is_empty() {
        lines.push(format!(
            "[DRY-RUN] 保留用户数据：{}",
            record.uninstall.keep_user_data.join("；")
        ));
    }
    Ok(lines)
}

fn missing_sources(actions: &[Action]) -> Vec<std::path::PathBuf> {
    actions
        .iter()
        .filter_map(|a| match a {
            Action::CopyFile { from, .. }
            | Action::CopyDir { from, .. }
            | Action::CopyUninstaller { from, .. } if !from.exists() => Some(from.clone()),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dry_run_reports_missing_source() {
        let spec = FlowSpec::from_toml_str(
            r#"
[app]
name = "X"
id = "x"
version = "1"
[install]
install_dir = "$HOME/x"
[[install.items]]
type = "copy-file"
source = "no-such-file.bin"
dest = "$INSTALL_DIR/a.bin"
"#,
        )
        .unwrap();
        let err = dry_run_install(&spec, Path::new("."), &InstallInputs::default()).unwrap_err();
        assert!(err.contains("缺失源"));
    }
}
