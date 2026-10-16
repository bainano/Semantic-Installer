//! 非 Windows 平台实现：Linux 桌面项 / macOS .command 启动器；
//! 注册表相关操作为幂等 no-op（记日志）。

use std::path::Path;

use crate::spec::AppSpec;

fn shell_escape(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "\\\""))
}

/// 创建快捷方式：Linux → `.desktop` 桌面项；macOS → `.command` 启动器。
pub fn create_shortcut(
    path: &Path,
    target: &Path,
    args: Option<&str>,
    icon: Option<&Path>,
) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        let mut content = String::new();
        content.push_str("[Desktop Entry]\n");
        content.push_str("Type=Application\n");
        content.push_str("Terminal=false\n");
        let name = path
            .file_stem()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "App".into());
        content.push_str(&format!("Name={name}\n"));
        let mut exec = shell_escape(&target.to_string_lossy());
        if let Some(a) = args {
            exec.push(' ');
            exec.push_str(a);
        }
        content.push_str(&format!("Exec={exec}\n"));
        if let Some(ic) = icon {
            content.push_str(&format!("Icon={}\n", ic.display()));
        }
        std::fs::write(path, content)
            .map_err(|e| format!("写入快捷方式 {} 失败：{e}", path.display()))?;
        set_executable(path)?;
        Ok(())
    }
    #[cfg(target_os = "macos")]
    {
        let mut content = String::from("#!/bin/bash\n");
        content.push_str("exec ");
        content.push_str(&shell_escape(&target.to_string_lossy()));
        if let Some(a) = args {
            content.push(' ');
            content.push_str(a);
        }
        content.push('\n');
        std::fs::write(path, content)
            .map_err(|e| format!("写入快捷方式 {} 失败：{e}", path.display()))?;
        set_executable(path)?;
        Ok(())
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        log::info!("当前平台不支持创建快捷方式：{}", path.display());
        Ok(())
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn set_executable(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    let mut perm = std::fs::metadata(path)
        .map_err(|e| format!("读取 {} 权限失败：{e}", path.display()))?
        .permissions();
    perm.set_mode(0o755);
    std::fs::set_permissions(path, perm)
        .map_err(|e| format!("设置 {} 可执行权限失败：{e}", path.display()))
}

pub fn remove_shortcut(path: &Path) -> Result<(), String> {
    std::fs::remove_file(path).map_err(|e| format!("删除快捷方式 {} 失败：{e}", path.display()))
}

pub fn write_registry_value(path: &str, name: &str, value: &str) -> Result<(), String> {
    log::info!("注册表操作仅在 Windows 平台生效（跳过 {path}\\{name} = {value}）");
    Ok(())
}

pub fn delete_registry_value(path: &str, name: &str) -> Result<(), String> {
    log::info!("注册表操作仅在 Windows 平台生效（跳过删除 {path}\\{name}）");
    Ok(())
}

pub fn delete_registry_key(path: &str) -> Result<(), String> {
    log::info!("注册表操作仅在 Windows 平台生效（跳过删除 {path}）");
    Ok(())
}

pub fn register_uninstall(app: &AppSpec, _exe: &Path, _dir: &Path) -> Result<(), String> {
    log::info!("卸载登记仅在 Windows 平台生效（跳过 {}）", app.name);
    Ok(())
}

pub fn unregister_uninstall(_app_id: &str) -> Result<(), String> {
    log::info!("卸载登记仅在 Windows 平台生效（跳过注销）");
    Ok(())
}
