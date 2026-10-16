//! Windows 平台实现：注册表（winreg）、.lnk 快捷方式（windows crate COM）、
//! "添加/删除程序" 卸载登记。
//!
//! 包含两处经审计的 unsafe：`CoInitializeEx` 与 IShellLink COM 调用（系统 API
//! 本身是 unsafe 的），封装在安全函数内，不参与任何内存自管理逻辑。

use std::path::Path;

use windows::core::{Interface, PCWSTR};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, IPersistFile,
};
use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};

use crate::spec::AppSpec;

fn hstring(s: &str) -> windows::core::HSTRING {
    windows::core::HSTRING::from(s)
}

fn pcwstr(h: &windows::core::HSTRING) -> PCWSTR {
    PCWSTR(h.as_ptr())
}

/// 创建 .lnk 快捷方式（COM IShellLink，Windows 专属）。
pub fn create_shortcut(
    path: &Path,
    target: &Path,
    args: Option<&str>,
    icon: Option<&Path>,
) -> Result<(), String> {
    // SAFETY：COM 初始化与接口调用，指针生命周期限定于本函数；这是本项目仅有的 unsafe。
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)
            .map_err(|e| format!("创建快捷方式（CoCreateInstance）失败：{e}"))?;

        let target_h = hstring(&target.to_string_lossy());
        link.SetPath(pcwstr(&target_h))
            .map_err(|e| format!("设置快捷方式目标失败：{e}"))?;

        if let Some(a) = args {
            let a_h = hstring(a);
            link.SetArguments(pcwstr(&a_h))
                .map_err(|e| format!("设置快捷方式参数失败：{e}"))?;
        }

        if let Some(ic) = icon {
            let ic_h = hstring(&ic.to_string_lossy());
            link.SetIconLocation(pcwstr(&ic_h), 0)
                .map_err(|e| format!("设置快捷方式图标失败：{e}"))?;
        }

        let persist: IPersistFile = link
            .cast()
            .map_err(|e| format!("获取 IPersistFile 失败：{e}"))?;
        let lnk_h = hstring(&path.to_string_lossy());
        persist
            .Save(pcwstr(&lnk_h), true)
            .map_err(|e| format!("保存快捷方式失败：{e}"))?;
    }
    Ok(())
}

pub fn remove_shortcut(path: &Path) -> Result<(), String> {
    std::fs::remove_file(path).map_err(|e| format!("删除快捷方式 {} 失败：{e}", path.display()))
}

/// 解析 "HKCU\\Software\\..." / "HKLM\\Software\\..." 形式的注册表路径。
fn split_hive(path: &str) -> Result<(winreg::HKEY, String), String> {
    let (hive, rest) = if let Some(r) = path.strip_prefix("HKCU\\") {
        (winreg::enums::HKEY_CURRENT_USER, r)
    } else if let Some(r) = path.strip_prefix("HKLM\\") {
        (winreg::enums::HKEY_LOCAL_MACHINE, r)
    } else if let Some(r) = path.strip_prefix("HKCR\\") {
        (winreg::enums::HKEY_CLASSES_ROOT, r)
    } else {
        return Err(format!("注册表路径必须以 HKCU\\ / HKLM\\ / HKCR\\ 开头：{path}"));
    };
    Ok((hive, rest.to_owned()))
}

fn open_or_create(hive: winreg::HKEY, sub: &str) -> Result<winreg::RegKey, String> {
    let root = winreg::RegKey::predef(hive);
    root.create_subkey(sub)
        .map(|(k, _)| k)
        .map_err(|e| format!("打开注册表键 {sub} 失败：{e}"))
}

pub fn write_registry_value(path: &str, name: &str, value: &str) -> Result<(), String> {
    let (hive, sub) = split_hive(path)?;
    let key = open_or_create(hive, &sub)?;
    key.set_value(name, &value)
        .map_err(|e| format!("写入注册表值 {path}\\{name} 失败：{e}"))
}

pub fn delete_registry_value(path: &str, name: &str) -> Result<(), String> {
    let (hive, sub) = split_hive(path)?;
    let root = winreg::RegKey::predef(hive);
    let key = root
        .open_subkey(&sub)
        .map_err(|e| format!("打开注册表键 {sub} 失败：{e}"))?;
    match key.delete_value(name) {
        Ok(()) => Ok(()),
        Err(e) => {
            // 值不存在视为成功（幂等）
            if e.kind() == std::io::ErrorKind::NotFound {
                Ok(())
            } else {
                Err(format!("删除注册表值 {path}\\{name} 失败：{e}"))
            }
        }
    }
}

pub fn delete_registry_key(path: &str) -> Result<(), String> {
    let (hive, sub) = split_hive(path)?;
    let (parent, leaf) = match sub.rfind('\\') {
        Some(i) => (&sub[..i], &sub[i + 1..]),
        None => ("", sub.as_str()),
    };
    let root = winreg::RegKey::predef(hive);
    let parent_key = if parent.is_empty() {
        root
    } else {
        root.open_subkey(parent)
            .map_err(|e| format!("打开注册表键 {parent} 失败：{e}"))?
    };
    match parent_key.delete_subkey_all(leaf) {
        Ok(()) => Ok(()),
        Err(e) => {
            if e.kind() == std::io::ErrorKind::NotFound {
                Ok(())
            } else {
                Err(format!("删除注册表键 {path} 失败：{e}"))
            }
        }
    }
}

/// 注册"添加/删除程序"卸载入口（HKCU，无需管理员权限）。
pub fn register_uninstall(app: &AppSpec, uninstall_exe: &Path, install_dir: &Path) -> Result<(), String> {
    let path = format!(
        "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\{}",
        app.id
    );
    let key = {
        let (hive, sub) = split_hive(&path)?;
        open_or_create(hive, &sub)?
    };
    key.set_value("DisplayName", &app.name)
        .map_err(|e| format!("写入 DisplayName 失败：{e}"))?;
    key.set_value("DisplayVersion", &app.version)
        .map_err(|e| format!("写入 DisplayVersion 失败：{e}"))?;
    if let Some(p) = &app.publisher {
        let _ = key.set_value("Publisher", p);
    }
    let uninstall_str = format!("\"{}\"", uninstall_exe.display());
    key.set_value("UninstallString", &uninstall_str)
        .map_err(|e| format!("写入 UninstallString 失败：{e}"))?;
    key.set_value("InstallLocation", &install_dir.to_string_lossy().into_owned())
        .map_err(|e| format!("写入 InstallLocation 失败：{e}"))?;
    let _ = key.set_value("DisplayIcon", &uninstall_exe.to_string_lossy().into_owned());
    let _ = key.set_value("QuietUninstallString", &uninstall_str);
    let _ = key.set_value("NoModify", &1u32);
    let _ = key.set_value("NoRepair", &1u32);
    Ok(())
}

pub fn unregister_uninstall(app_id: &str) -> Result<(), String> {
    delete_registry_key(&format!(
        "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\{app_id}"
    ))
}
