//! 系统 CJK 字体探测与注入，保证中文界面正常显示。

use std::path::PathBuf;
use std::sync::Arc;

use eframe::egui;

const CANDIDATES: &[&str] = &[
    // Windows
    "C:\\Windows\\Fonts\\msyh.ttc",
    "C:\\Windows\\Fonts\\msyh.ttf",
    "C:\\Windows\\Fonts\\simhei.ttf",
    "C:\\Windows\\Fonts\\Deng.ttf",
    "C:\\Windows\\Fonts\\simsun.ttc",
    // macOS
    "/System/Library/Fonts/PingFang.ttc",
    "/System/Library/Fonts/Hiragino Sans GB.ttc",
    // Linux
    "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
    "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc",
    "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
];

fn probe_cjk_font() -> Option<PathBuf> {
    CANDIDATES.iter().map(PathBuf::from).find(|p| p.exists())
}

/// 把首个可用的系统 CJK 字体注入 egui 字体表（比例 + 等宽族）。
pub fn install_cjk(ctx: &egui::Context) {
    let Some(path) = probe_cjk_font() else {
        log::warn!("未找到可用的系统 CJK 字体，中文可能无法渲染。");
        return;
    };
    let bytes = match std::fs::read(&path) {
        Ok(b) => b,
        Err(e) => {
            log::warn!("读取 CJK 字体 {} 失败：{e}", path.display());
            return;
        }
    };
    let mut fonts = egui::FontDefinitions::default();
    fonts
        .font_data
        .insert("cjk".to_owned(), Arc::new(egui::FontData::from_owned(bytes)));
    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        fonts.families.entry(family).or_default().push("cjk".to_owned());
    }
    ctx.set_fonts(fonts);
    log::info!("已加载 CJK 字体：{}", path.display());
}
