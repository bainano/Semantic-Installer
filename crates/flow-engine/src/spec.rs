//! 安装 / 卸载清单（install.toml）的 serde 模型。
//!
//! 说明：`[window]` 段为**可选覆盖**——窗口尺寸的权威来源是语义标记 `<window width height>`；
//! `resizable` 字段仅为声明性约束，引擎在运行时始终固定窗口尺寸。

use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AppSpec {
    pub name: String,
    pub id: String,
    pub version: String,
    #[serde(default)]
    pub publisher: Option<String>,
    #[serde(default)]
    pub homepage: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowSpec {
    #[serde(default = "default_width")]
    pub width: u32,
    #[serde(default = "default_height")]
    pub height: u32,
    /// 固定尺寸：引擎不做运行时缩放，此字段仅为声明性约束（保持 false）。
    #[serde(default)]
    pub resizable: bool,
    #[serde(default)]
    pub theme: Option<String>,
}

fn default_width() -> u32 {
    800
}
fn default_height() -> u32 {
    600
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StepsSpec {
    /// 流程页面顺序（id 列表）；为空时使用标记语言中的页面声明顺序。
    #[serde(default)]
    pub pages: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FlowSpec {
    pub app: AppSpec,
    #[serde(default)]
    pub window: Option<WindowSpec>,
    #[serde(default)]
    pub steps: StepsSpec,
    #[serde(default)]
    pub install: Option<InstallSpec>,
    #[serde(default)]
    pub uninstall: Option<UninstallSpec>,
}

impl FlowSpec {
    pub fn from_toml_str(s: &str) -> Result<Self, String> {
        toml::from_str(s).map_err(|e| format!("清单解析失败：{e}"))
    }

    pub fn from_toml_file(path: &Path) -> Result<Self, String> {
        let s = std::fs::read_to_string(path)
            .map_err(|e| format!("读取清单 {} 失败：{e}", path.display()))?;
        Self::from_toml_str(&s)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallSpec {
    /// 默认安装目录模板，如 "$LOCALAPPDATA/Demo App"。
    pub install_dir: String,
    #[serde(default)]
    pub items: Vec<ItemSpec>,
    #[serde(default)]
    pub shortcuts: Vec<ShortcutSpec>,
    #[serde(default)]
    pub registry: RegistrySpec,
    /// 把与安装器同目录的卸载器复制进安装目录（一体化卸载）。
    #[serde(default)]
    pub copy_uninstaller: bool,
    #[serde(default)]
    pub run_after: Option<RunSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum ItemSpec {
    CopyDir {
        source: String,
        dest: String,
    },
    CopyFile {
        source: String,
        dest: String,
    },
    CreateDir {
        path: String,
    },
    WriteFile {
        path: String,
        content: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShortcutSpec {
    pub name: String,
    /// 目标模板（如 "$INSTALL_DIR/app.exe"）。
    pub target: String,
    #[serde(default)]
    pub args: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
    /// desktop | start-menu（缺省 desktop）。
    #[serde(default)]
    pub dir: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RegistrySpec {
    /// 在"添加/删除程序"注册卸载入口（仅 Windows）。
    #[serde(default)]
    pub uninstall_key: bool,
    #[serde(default)]
    pub values: Vec<RegistryValueSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryValueSpec {
    /// 注册表路径，如 "HKCU\\Software\\Demo App"。
    pub path: String,
    pub name: String,
    /// 值支持路径占位符（如 $INSTALL_DIR）。
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunSpec {
    pub cmd: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub wait: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UninstallSpec {
    /// 卸载时保留的路径模板（如用户数据目录）。注意：不应位于 install_dir 内部。
    #[serde(default)]
    pub keep_user_data: Vec<String>,
    #[serde(default)]
    pub remove_shortcuts: bool,
    #[serde(default)]
    pub remove_registry: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_full_manifest() {
        let src = r#"
[app]
name = "Demo App"
id = "com.example.demo-app"
version = "1.0.0"
publisher = "Example Co."

[window]
width = 900
height = 620
resizable = false

[steps]
pages = ["welcome", "license", "options", "progress", "finish"]

[install]
install_dir = "$LOCALAPPDATA/Demo App"
copy_uninstaller = true

[[install.items]]
type = "copy-dir"
source = "resources/files"
dest = "$INSTALL_DIR/files"

[[install.items]]
type = "write-file"
path = "$INSTALL_DIR/readme.txt"
content = "hello"

[[install.shortcuts]]
name = "Demo App 说明"
target = "$INSTALL_DIR/readme.txt"
dir = "desktop"

[install.registry]
uninstall_key = true

[[install.registry.values]]
path = "HKCU\\Software\\Demo App"
name = "InstallDir"
value = "$INSTALL_DIR"

[uninstall]
keep_user_data = []
remove_shortcuts = true
remove_registry = true
"#;
        let spec = FlowSpec::from_toml_str(src).expect("解析成功");
        assert_eq!(spec.app.name, "Demo App");
        assert_eq!(spec.steps.pages.len(), 5);
        let install = spec.install.as_ref().expect("有 install");
        assert_eq!(install.items.len(), 2);
        assert_eq!(install.shortcuts.len(), 1);
        assert!(install.registry.uninstall_key);
        let uninstall = spec.uninstall.as_ref().expect("有 uninstall");
        assert!(uninstall.remove_shortcuts);
    }

    #[test]
    fn minimal_manifest_has_defaults() {
        let src = r#"
[app]
name = "X"
id = "x"
version = "0.1"
[install]
install_dir = "$HOME/x"
"#;
        let spec = FlowSpec::from_toml_str(src).expect("解析成功");
        assert!(spec.window.is_none());
        assert!(spec.install.as_ref().unwrap().items.is_empty());
        assert!(!spec.install.as_ref().unwrap().registry.uninstall_key);
    }

    #[test]
    fn invalid_toml_errors() {
        let err = FlowSpec::from_toml_str("[app\n").unwrap_err();
        assert!(!err.is_empty());
    }
}
