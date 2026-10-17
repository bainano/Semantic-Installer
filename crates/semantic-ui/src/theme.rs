//! 设计令牌：颜色 / 字号 / 圆角 / 间距。UI 适配层将令牌映射为具体 GUI 视觉。

/// 内置设计主题。
#[derive(Debug, Clone)]
pub struct Theme {
    pub name: String,
    pub bg: u32,
    pub panel: u32,
    pub text: u32,
    pub text_muted: u32,
    pub primary: u32,
    pub accent: u32,
    pub danger: u32,
    pub border: u32,
    pub radius: f32,
    pub font_size: f32,
    pub heading_size: f32,
    pub spacing: f32,
}

impl Theme {
    pub fn light() -> Self {
        Self {
            name: "light".into(),
            bg: 0xFFF5F7FA,
            panel: 0xFFFFFFFF,
            text: 0xFF1F2329,
            text_muted: 0xFF6B7280,
            primary: 0xFF1F6FEB,
            accent: 0xFF2563EB,
            danger: 0xFFE5484D,
            border: 0xFFE5E7EB,
            radius: 8.0,
            font_size: 14.0,
            heading_size: 24.0,
            spacing: 8.0,
        }
    }

    pub fn dark() -> Self {
        Self {
            name: "dark".into(),
            bg: 0xFF111827,
            panel: 0xFF1F2937,
            text: 0xFFF3F4F6,
            text_muted: 0xFF9CA3AF,
            primary: 0xFF3B82F6,
            accent: 0xFF60A5FA,
            danger: 0xFFF87171,
            border: 0xFF374151,
            radius: 8.0,
            font_size: 14.0,
            heading_size: 24.0,
            spacing: 8.0,
        }
    }

    pub fn named(name: &str) -> Self {
        match name {
            "dark" => Self::dark(),
            _ => Self::light(),
        }
    }

    pub fn with_name(mut self, name: &str) -> Self {
        self.name = name.to_owned();
        self
    }
}

/// 解析 `#rgb` / `#rrggbb` / `#rrggbbaa` 颜色为 `0xAARRGGBB`（高 8 位为 alpha，`#rrggbb` 时 alpha=FF）。
/// 半透明色（如 `#00000080`）可用于图层背景遮罩、文字颜色等任意颜色属性。
pub fn parse_color(s: &str) -> Option<u32> {
    let s = s.trim();
    let hex = s.strip_prefix('#')?;
    let digits = |h: &str| u32::from_str_radix(h, 16).ok();
    match hex.len() {
        3 => {
            let r = digits(&hex[0..1])?;
            let g = digits(&hex[1..2])?;
            let b = digits(&hex[2..3])?;
            Some(0xFF00_0000 | (r << 20) | (g << 12) | (b << 4))
        }
        6 => {
            let r = digits(&hex[0..2])?;
            let g = digits(&hex[2..4])?;
            let b = digits(&hex[4..6])?;
            Some(0xFF00_0000 | (r << 16) | (g << 8) | b)
        }
        8 => {
            let r = digits(&hex[0..2])?;
            let g = digits(&hex[2..4])?;
            let b = digits(&hex[4..6])?;
            let a = digits(&hex[6..8])?;
            Some((a << 24) | (r << 16) | (g << 8) | b)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_parsing() {
        assert_eq!(parse_color("#1f6feb"), Some(0xFF1F6FEB));
        assert_eq!(parse_color("#fff"), Some(0xFFF0F0F0));
        assert_eq!(parse_color("#FF0000"), Some(0xFFFF0000));
        assert_eq!(parse_color("#00000080"), Some(0x80000000));
        assert_eq!(parse_color("#ff0000ff"), Some(0xFFFF0000));
        assert_eq!(parse_color("#12345678"), Some(0x78123456));
        assert_eq!(parse_color("1f6feb"), None);
        assert_eq!(parse_color("#12"), None);
        assert_eq!(parse_color("#12345"), None);
    }

    #[test]
    fn themes_have_defaults() {
        let l = Theme::light();
        assert_eq!(l.name, "light");
        assert!(l.primary != l.bg);
        let d = Theme::dark();
        assert_eq!(d.name, "dark");
        assert_eq!(Theme::named("dark").name, "dark");
        assert_eq!(Theme::named("x").name, "light");
    }
}
