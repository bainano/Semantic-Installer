//! 平台能力层：路径占位符解析 + 注册表 / 快捷方式 / 卸载登记（按平台门控）。
//!
//! 可靠性约定：本项目应用代码保持 0 unsafe；仅 `windows.rs` 中的 COM 互操作
//! （IShellLink）包含**经审计的两处小 unsafe 块**，封装在安全函数签名内，并在
//! 文档（docs/architecture.md）中说明。

use std::path::{Path, PathBuf};

use crate::actions::ShortcutDir;

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::{
    create_shortcut, delete_registry_key, delete_registry_value, register_uninstall,
    remove_shortcut, unregister_uninstall, write_registry_value,
};
#[cfg(not(windows))]
mod posix;
#[cfg(not(windows))]
pub use posix::{
    create_shortcut, delete_registry_key, delete_registry_value, register_uninstall,
    remove_shortcut, unregister_uninstall, write_registry_value,
};

/// 路径解析上下文（模板基准 + 平台目录）。
#[derive(Debug, Clone)]
pub struct ResolveCtx {
    /// 资源根目录（markup / manifest / 相对 source 的基准）。
    pub root: PathBuf,
    pub install_dir: PathBuf,
    pub local_app_data: PathBuf,
    pub program_files: Option<PathBuf>,
    pub home: PathBuf,
    pub temp: PathBuf,
    pub exe_dir: PathBuf,
}

impl ResolveCtx {
    /// 以 `install_dir = "."` 占位建立基础上下文（用于解析 install_dir 模板本身）。
    pub fn base(root: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
            install_dir: PathBuf::from("."),
            local_app_data: local_app_data(),
            program_files: program_files(),
            home: home(),
            temp: temp(),
            exe_dir: exe_dir(),
        }
    }

    pub fn with_install_dir(mut self, dir: PathBuf) -> Self {
        self.install_dir = dir;
        self
    }

    pub fn for_install(root: &Path, install_dir: &Path) -> Self {
        Self::base(root).with_install_dir(install_dir.to_path_buf())
    }

    /// 卸载上下文：安装目录即资源基准。
    pub fn for_uninstall(install_dir: &Path) -> Self {
        Self::for_install(install_dir, install_dir)
    }
}

pub fn local_app_data() -> PathBuf {
    dirs::data_local_dir().unwrap_or_else(home)
}

pub fn program_files() -> Option<PathBuf> {
    std::env::var("ProgramFiles")
        .ok()
        .map(PathBuf::from)
        .filter(|p| p.as_os_str().to_string_lossy().len() > 3)
}

pub fn home() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("."))
}

pub fn temp() -> PathBuf {
    std::env::temp_dir()
}

pub fn exe_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."))
}

/// 解析路径模板中的占位符：`$LOCALAPPDATA / $PROGRAMFILES / $INSTALL_DIR / $HOME / $TEMP / $EXEDIR`。
pub fn resolve_template(tpl: &str, ctx: &ResolveCtx) -> Result<PathBuf, String> {
    let mut out = String::new();
    let mut rest = tpl;
    while let Some(i) = rest.find('$') {
        out.push_str(&rest[..i]);
        let tail = &rest[i..];
        let (name, consumed) = if tail.starts_with("$LOCALAPPDATA") {
            ("$LOCALAPPDATA", 13)
        } else if tail.starts_with("$PROGRAMFILES") {
            ("$PROGRAMFILES", 13)
        } else if tail.starts_with("$INSTALL_DIR") {
            ("$INSTALL_DIR", 12)
        } else if tail.starts_with("$EXEDIR") {
            ("$EXEDIR", 7)
        } else if tail.starts_with("$HOME") {
            ("$HOME", 5)
        } else if tail.starts_with("$TEMP") {
            ("$TEMP", 5)
        } else {
            return Err(format!("未知路径占位符（在 \"{tpl}\" 中）"));
        };
        let value = match name {
            "$LOCALAPPDATA" => ctx.local_app_data.to_string_lossy().into_owned(),
            "$PROGRAMFILES" => ctx
                .program_files
                .as_ref()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_default(),
            "$INSTALL_DIR" => ctx.install_dir.to_string_lossy().into_owned(),
            "$HOME" => ctx.home.to_string_lossy().into_owned(),
            "$TEMP" => ctx.temp.to_string_lossy().into_owned(),
            "$EXEDIR" => ctx.exe_dir.to_string_lossy().into_owned(),
            _ => unreachable!("占位符已匹配"),
        };
        out.push_str(&value);
        rest = &tail[consumed..];
    }
    out.push_str(rest);
    Ok(PathBuf::from(out))
}

/// 解析 install_dir 模板（不允许引用自身）。
pub fn resolve_install_dir(tpl: &str, root: &Path) -> Result<PathBuf, String> {
    if tpl.contains("$INSTALL_DIR") {
        return Err("install_dir 模板不能引用自身占位符 $INSTALL_DIR".into());
    }
    let ctx = ResolveCtx::base(root);
    resolve_template(tpl, &ctx)
}

/// 相对路径以 root 为基准解析为绝对路径。
pub fn resolve_source(p: &str, ctx: &ResolveCtx) -> PathBuf {
    let pb = PathBuf::from(p);
    if pb.is_absolute() {
        pb
    } else {
        ctx.root.join(pb)
    }
}

/// 快捷方式所在目录（平台相关）。
pub fn shortcut_dir_path(dir: ShortcutDir) -> Option<PathBuf> {
    match dir {
        ShortcutDir::Desktop => dirs::desktop_dir(),
        #[cfg(windows)]
        ShortcutDir::StartMenu => {
            let base = std::env::var_os("APPDATA").map(PathBuf::from)?;
            Some(
                base.join("Microsoft")
                    .join("Windows")
                    .join("Start Menu")
                    .join("Programs"),
            )
        }
        #[cfg(not(windows))]
        ShortcutDir::StartMenu => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn template_resolution() {
        let ctx = ResolveCtx::base(Path::new("C:/root"));
        assert_eq!(
            resolve_template("$HOME/x", &ctx).unwrap(),
            home().join("x")
        );
        assert_eq!(
            resolve_template("$EXEDIR/a", &ctx).unwrap(),
            exe_dir().join("a")
        );
        assert_eq!(resolve_template("$INSTALL_DIR", &ctx).unwrap(), PathBuf::from("."));
        assert!(resolve_template("$NOPE/x", &ctx).is_err());
    }

    #[test]
    fn install_dir_self_reference_rejected() {
        assert!(resolve_install_dir("$INSTALL_DIR/x", Path::new(".")).is_err());
        assert!(resolve_install_dir("$HOME/x", Path::new(".")).is_ok());
    }

    #[test]
    fn source_resolution() {
        let ctx = ResolveCtx::base(Path::new("C:/root"));
        assert_eq!(resolve_source("a/b", &ctx), PathBuf::from("C:/root/a/b"));
        assert_eq!(resolve_source("C:/abs", &ctx), PathBuf::from("C:/abs"));
    }

    #[test]
    fn app_spec_type_checks() {
        // 编译期占位：确保 AppSpec 可构造
        use crate::spec::AppSpec;
        let _ = AppSpec {
            name: "x".into(),
            id: "x".into(),
            version: "1".into(),
            publisher: None,
            homepage: None,
        };
    }
}
