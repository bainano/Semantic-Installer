//! 语义设计令牌 → egui Visuals 映射。

use eframe::egui;

/// 0xRRGGBB → Color32。
pub fn color(hex: u32) -> egui::Color32 {
    egui::Color32::from_rgb(
        ((hex >> 16) & 0xFF) as u8,
        ((hex >> 8) & 0xFF) as u8,
        (hex & 0xFF) as u8,
    )
}

/// 混入白色（用于悬停高亮）。
pub fn lighten(c: egui::Color32, t: f32) -> egui::Color32 {
    egui::Color32::from_rgb(
        (c.r() as f32 + (255.0 - c.r() as f32) * t) as u8,
        (c.g() as f32 + (255.0 - c.g() as f32) * t) as u8,
        (c.b() as f32 + (255.0 - c.b() as f32) * t) as u8,
    )
}

/// 把语义主题应用到 egui 上下文。
pub fn apply(ctx: &egui::Context, theme: &semantic_ui::Theme) {
    let dark = theme.name == "dark";
    let mut visuals = if dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };

    visuals.panel_fill = color(theme.panel);
    visuals.window_fill = color(theme.panel);
    visuals.faint_bg_color = color(theme.bg);
    visuals.extreme_bg_color = color(theme.bg);
    visuals.override_text_color = Some(color(theme.text));

    let radius = egui::CornerRadius::same(theme.radius.clamp(0.0, 24.0) as u8);
    let border_stroke = egui::Stroke::new(1.0, color(theme.border));

    for w in [
        &mut visuals.widgets.noninteractive,
        &mut visuals.widgets.inactive,
        &mut visuals.widgets.hovered,
        &mut visuals.widgets.active,
        &mut visuals.widgets.open,
    ] {
        w.corner_radius = radius;
        w.bg_stroke = border_stroke;
    }

    visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, color(theme.text));
    visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.0, color(theme.text));
    visuals.widgets.active.fg_stroke = egui::Stroke::new(1.0, color(theme.text));
    visuals.widgets.inactive.bg_fill = color(theme.panel);
    visuals.widgets.hovered.bg_fill = lighten(color(theme.primary), 0.82);
    visuals.widgets.active.bg_fill = color(theme.accent);
    visuals.widgets.inactive.weak_bg_fill = color(theme.border);
    visuals.widgets.hovered.weak_bg_fill = lighten(color(theme.primary), 0.9);
    visuals.widgets.active.weak_bg_fill = color(theme.accent);
    visuals.selection.bg_fill = color(theme.accent);
    visuals.text_cursor.stroke = egui::Stroke::new(2.0, color(theme.accent));

    let egui_theme = if dark {
        egui::Theme::Dark
    } else {
        egui::Theme::Light
    };
    ctx.set_visuals_of(egui_theme, visuals);

    let mut style = (*ctx.style_of(egui_theme)).clone();
    style.spacing.item_spacing = egui::vec2(theme.spacing, theme.spacing);
    style.spacing.button_padding = egui::vec2(10.0, 6.0);
    ctx.set_style_of(egui_theme, style);
}
