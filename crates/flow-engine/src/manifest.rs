//! 安装记录（uninstall.json）的读写与定位。

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::platform::local_app_data;
use crate::spec::{AppSpec, RegistryValueSpec, ShortcutSpec, UninstallSpec};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallRecord {
    pub app: AppSpec,
    pub version: String,
    pub install_dir: String,
    pub installed_at: String,
    pub files: Vec<String>,
    pub dirs: Vec<String>,
    pub shortcuts: Vec<ShortcutSpec>,
    pub registry: Vec<RegistryValueSpec>,
    pub uninstall_key: bool,
    pub uninstall: UninstallSpec,
    pub uninstaller_path: Option<String>,
    /// 安装时选择的语言（来自 id="locale" 控件），供联动与后续查询。
    #[serde(default)]
    pub locale: Option<String>,
    /// 完整清单（供卸载器重建流程与窗口配置）。
    pub flow: crate::spec::FlowSpec,
}

impl InstallRecord {
    /// 当前时间的可读字符串（本地时间）。
    pub fn now() -> String {
        use std::time::{SystemTime, UNIX_EPOCH};
        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        // 直接格式化为本地时间字符串
        let days = secs / 86400;
        let (y, m, d) = civil_from_days(days as i64);
        let rem = secs % 86400;
        let (h, mi, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);
        format!("{y:04}-{m:02}-{d:02} {h:02}:{mi:02}:{s:02}")
    }
}

/// 天数转公历日期（Howard Hinnant 算法）。
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// 写入安装记录：主副本（安装目录内）+ 用户级副本。
pub fn write_record(record: &InstallRecord, install_dir: &Path, app_id: &str) -> Result<(), String> {
    let json = serde_json::to_string_pretty(record).map_err(|e| format!("序列化记录失败：{e}"))?;
    let primary = install_dir.join("uninstall.json");
    if let Some(parent) = primary.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("创建目录失败：{e}"))?;
    }
    std::fs::write(&primary, &json)
        .map_err(|e| format!("写入 {} 失败：{e}", primary.display()))?;
    let user_copy = local_app_data().join(app_id).join("uninstall.json");
    if let Some(parent) = user_copy.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("创建目录失败：{e}"))?;
    }
    std::fs::write(&user_copy, &json)
        .map_err(|e| format!("写入 {} 失败：{e}", user_copy.display()))?;
    Ok(())
}

pub fn load_record(path: &Path) -> Result<InstallRecord, String> {
    let s = std::fs::read_to_string(path)
        .map_err(|e| format!("读取记录 {} 失败：{e}", path.display()))?;
    serde_json::from_str(&s).map_err(|e| format!("解析记录 {} 失败：{e}", path.display()))
}

/// 依次查找：显式路径 → 卸载器同目录 → 用户级副本。
pub fn find_record(explicit: Option<&Path>, exe_dir: &Path, app_id_hint: Option<&str>) -> Result<(PathBuf, InstallRecord), String> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(p) = explicit {
        candidates.push(p.to_path_buf());
    }
    candidates.push(exe_dir.join("uninstall.json"));
    if let Some(id) = app_id_hint {
        candidates.push(local_app_data().join(id).join("uninstall.json"));
    }
    for c in candidates {
        if c.is_file()
            && let Ok(record) = load_record(&c) {
                return Ok((c, record));
            }
    }
    Err("未找到安装记录 uninstall.json（请确认应用已安装，或使用 --manifest 指定记录路径）".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec::FlowSpec;

    #[test]
    fn record_roundtrip() {
        let record = InstallRecord {
            app: AppSpec {
                name: "T".into(),
                id: "com.t".into(),
                version: "1".into(),
                publisher: None,
                homepage: None,
            },
            version: "1".into(),
            install_dir: "C:/x".into(),
            installed_at: InstallRecord::now(),
            files: vec!["C:/x/a.txt".into()],
            dirs: vec!["C:/x".into()],
            shortcuts: vec![],
            registry: vec![],
            uninstall_key: false,
            uninstall: UninstallSpec::default(),
            uninstaller_path: None,
            locale: Some("zh-CN".into()),
            flow: FlowSpec::default(),
        };
        let json = serde_json::to_string(&record).unwrap();
        let back: InstallRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(back.app.id, "com.t");
        assert_eq!(back.files, vec!["C:/x/a.txt"]);
    }

    #[test]
    fn now_formats() {
        let s = InstallRecord::now();
        assert_eq!(s.len(), 19, "格式应为 YYYY-MM-DD HH:MM:SS：{s}");
        assert!(s.contains('-') && s.contains(':'));
    }
}
