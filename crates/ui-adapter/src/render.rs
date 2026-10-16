//! 语义节点树 → egui 组件渲染。
//!
//! 布局矩形由固定画布布局引擎预先算好（`PageModel`），渲染阶段只做：
//! 画布定位、控件绘制、交互响应与事件收集。图标一律使用矢量绘制（Painter），
//! 不使用 emoji / 表情字符。

use std::path::Path;

use eframe::egui;
use semantic_ui::ast::Node;
use semantic_ui::layout::{PageModel, Rect};
use semantic_ui::widget::{Click, UiState};
use semantic_ui::{FontWeight, Theme};

use crate::app::{phase_failed, phase_working, Phase};
use crate::assets::AssetCache;
use crate::theme::{self, color};

/// 渲染当前页面并收集点击事件（Ui 中心化：直接绘制在给定 Ui 上）。
/// 参数为渲染管线的显式依赖（状态、主题、进度、资源、事件出口），保持扁平可读。
#[allow(clippy::too_many_arguments)]
pub(crate) fn render_page(
    ui: &mut egui::Ui,
    model: &PageModel,
    state: &mut UiState,
    phase: &Phase,
    progress: &Option<flow_engine::ProgressEvent>,
    theme: &Theme,
    assets: &mut AssetCache,
    root: &Path,
    emit: &mut dyn FnMut(Click),
) {
    // 固定画布背景
    let canvas = ui.max_rect();
    ui.painter()
        .rect_filled(canvas, egui::CornerRadius::ZERO, color(theme.bg));

    let valid = semantic_ui::widget::page_valid(model, state);
    for (i, node) in model.nodes.iter().enumerate() {
        let rect = model.rects[i];
        render_node(
            ui, node, &rect, i, valid, state, phase, progress, theme, assets, root, emit,
        );
    }
    // 失败提示条（覆盖页面顶部）
    if let Some(msg) = phase_failed(phase) {
        let painter = ui.painter();
        let strip = egui::Rect::from_min_size(
            egui::pos2(0.0, 0.0),
            egui::vec2(ui.max_rect().width(), 40.0),
        );
        painter.rect_filled(strip, egui::CornerRadius::ZERO, color(theme.danger));
        painter.text(
            strip.left_center() + egui::vec2(16.0, 0.0),
            egui::Align2::LEFT_CENTER,
            msg,
            egui::FontId::proportional(14.0),
            egui::Color32::WHITE,
        );
    }
}

fn rect_of(r: &Rect) -> egui::Rect {
    egui::Rect::from_min_size(egui::pos2(r.x, r.y), egui::vec2(r.w, r.h))
}

fn node_id(node: &Node, index: usize) -> String {
    // 始终携带序号保证同页内唯一；显式 id 仅作可读前缀（仍以序号结尾，杜绝冲突）
    match node.attr("id") {
        Some(id) => format!("{}:{}:{}", node.tag, index, id),
        None => format!("{}:{}", node.tag, index),
    }
}

fn font_size(node: &Node, theme: &Theme) -> f32 {
    node.attr_float("size", theme.font_size).max(4.0)
}

/// 渲染单个节点到给定矩形。参数为渲染管线的显式依赖，保持扁平可读。
/// `index` 为节点在页面扁平列表中的序号，用于生成唯一交互 Id（同页多按钮不再冲突）。
#[allow(clippy::too_many_arguments)]
fn render_node(
    ui: &mut egui::Ui,
    node: &Node,
    rect: &Rect,
    index: usize,
    valid: bool,
    state: &mut UiState,
    phase: &Phase,
    progress: &Option<flow_engine::ProgressEvent>,
    theme: &Theme,
    assets: &mut AssetCache,
    root: &Path,
    emit: &mut dyn FnMut(Click),
) {
    let r = rect_of(rect);
    let painter = ui.painter();
    let id = node_id(node, index);

    match node.tag.as_str() {
        "page" | "row" | "column" => {
            if let Some(bg) = node.attr_color("bg") {
                painter.rect_filled(r, egui::CornerRadius::ZERO, color(bg));
            }
        }
        "text" | "label" => {
            let muted = node.is("label");
            let c = node
                .attr_color("color")
                .map(color)
                .unwrap_or_else(|| color(if muted { theme.text_muted } else { theme.text }));
            let size = font_size(node, theme);
            let weight = semantic_ui::layout::parse_weight(node.attr("weight"));
            let align = match node.attr("align") {
                Some("center") => egui::Align2::CENTER_TOP,
                Some("end") | Some("right") => egui::Align2::RIGHT_TOP,
                _ => egui::Align2::LEFT_TOP,
            };
            let pos = match align {
                egui::Align2::CENTER_TOP => r.center_top(),
                egui::Align2::RIGHT_TOP => r.right_top(),
                _ => r.left_top(),
            };
            let mut font = egui::FontId::proportional(size);
            if weight == FontWeight::Bold {
                font.size += 1.0; // egui 默认字体族无真实粗体，微增字号模拟加粗
            }
            painter.text(pos, align, node.text_of(""), font, c);
        }
        "link" => {
            let c = color(theme.accent);
            let size = font_size(node, theme);
            let pos = r.left_top();
            painter.text(
                pos,
                egui::Align2::LEFT_TOP,
                node.text_of(""),
                egui::FontId::proportional(size),
                c,
            );
            let (tw, _) = ui.ctx().fonts_mut(|f| {
                let g = f.layout_no_wrap(node.text_of(""), egui::FontId::proportional(size), c);
                (g.size().x, g.size().y)
            });
            painter.line_segment(
                [egui::pos2(r.min.x, r.bottom() - 2.0), egui::pos2(r.min.x + tw, r.bottom() - 2.0)],
                egui::Stroke::new(1.0, c),
            );
            let resp = ui.interact(r, egui::Id::new(id.clone()), egui::Sense::click());
            if resp.clicked() {
                emit(Click {
                    widget_id: id,
                    action: node.attr("action").map(str::to_owned),
                    goto: node.attr("goto").map(str::to_owned),
                    url: node.attr("url").map(str::to_owned),
                });
            }
            resp.on_hover_cursor(egui::CursorIcon::PointingHand);
        }
        "button" => {
            let is_forward = matches!(
                node.attr("action"),
                Some("next") | Some("install") | Some("uninstall")
            );
            let disabled = (is_forward && !valid) || node.attr("enabled") == Some("false");
            let hovered = ui.rect_contains_pointer(r);
            let active = ui.input(|i| i.pointer.any_pressed()) && hovered;
            let primary = node.flag("primary", false);
            let (fill, fg) = if disabled {
                (color(theme.border), color(theme.text_muted))
            } else if primary {
                if active {
                    (color(theme.accent), egui::Color32::WHITE)
                } else if hovered {
                    (theme::lighten(color(theme.primary), 0.12), egui::Color32::WHITE)
                } else {
                    (color(theme.primary), egui::Color32::WHITE)
                }
            } else if active {
                (color(theme.border), color(theme.text))
            } else if hovered {
                (theme::lighten(color(theme.panel), -0.06), color(theme.text))
            } else {
                (color(theme.panel), color(theme.text))
            };
            let radius = egui::CornerRadius::same(theme.radius.clamp(0.0, 24.0) as u8);
            painter.rect_filled(r, radius, fill);
            painter.rect_stroke(
                r,
                radius,
                egui::Stroke::new(1.0, color(theme.border)),
                egui::StrokeKind::Inside,
            );
            painter.text(
                r.center(),
                egui::Align2::CENTER_CENTER,
                node.text_of(""),
                egui::FontId::proportional(font_size(node, theme)),
                fg,
            );
            if disabled {
                return;
            }
            let resp = ui.interact(r, egui::Id::new(id.clone()), egui::Sense::click());
            if resp.clicked() {
                emit(Click {
                    widget_id: id,
                    action: node.attr("action").map(str::to_owned),
                    goto: node.attr("goto").map(str::to_owned),
                    url: None,
                });
            }
        }
        "input" => {
            let browse = node.flag("browse", false);
            let browse_w = if browse { 64.0 } else { 0.0 };
            let field_rect = egui::Rect::from_min_size(r.min, egui::vec2((r.width() - browse_w - 8.0).max(40.0), r.height()));
            let id_str = node.attr("id").unwrap_or_default().to_owned();
            let mut value = state.input(&id_str).to_owned();
            let placeholder = node.attr("placeholder").unwrap_or_default();
            let resp = ui.put(
                field_rect,
                egui::TextEdit::singleline(&mut value)
                    .hint_text(placeholder)
                    .font(egui::FontId::proportional(font_size(node, theme)))
                    .background_color(color(theme.panel))
                    .text_color(color(theme.text)),
            );
            if resp.changed() {
                state.set_input(&id_str, value);
            }
            if browse {
                let btn_rect = egui::Rect::from_min_size(
                    egui::pos2(r.right() - browse_w, r.top()),
                    egui::vec2(browse_w, r.height()),
                );
                let br = ui.put(btn_rect, egui::Button::new("浏览…"));
                if br.clicked()
                    && let Some(dir) = rfd::FileDialog::new().pick_folder() {
                        state.set_input(&id_str, dir.to_string_lossy().into_owned());
                    }
            }
        }
        "checkbox" => {
            let id_str = node.attr("id").unwrap_or_default().to_owned();
            let mut checked = state.checked(&id_str);
            let label = node.text_of("");
            let resp = ui.put(r, egui::Checkbox::new(&mut checked, label));
            if resp.changed() {
                state.set_checked(&id_str, checked);
            }
        }
        "radio" => {
            let group = node.attr("group").unwrap_or_default().to_owned();
            let value = node.attr("value").unwrap_or_default().to_owned();
            let selected = state.radio(&group) == Some(value.as_str());
            // 自绘单选：圆 + 标签
            let center = egui::pos2(r.min.x + 9.0, r.center().y);
            painter.circle_filled(center, 8.0, color(theme.panel));
            painter.circle_stroke(center, 8.0, egui::Stroke::new(1.5, color(theme.border)));
            if selected {
                painter.circle_filled(center, 4.0, color(theme.primary));
            }
            painter.text(
                egui::pos2(r.min.x + 26.0, r.center().y),
                egui::Align2::LEFT_CENTER,
                node.text_of(""),
                egui::FontId::proportional(font_size(node, theme)),
                color(theme.text),
            );
            let resp = ui.interact(r, egui::Id::new(node_id(node, index)), egui::Sense::click());
            if resp.clicked() && !selected {
                state.set_radio(group, value);
            }
        }
        "select" => {
            let id_str = node.attr("id").unwrap_or_default().to_owned();
            let options: Vec<String> = node
                .attr("options")
                .map(|s| {
                    s.split(',')
                        .map(|x| x.trim().to_owned())
                        .filter(|x| !x.is_empty())
                        .collect()
                })
                .unwrap_or_default();
            let mut sel = state
                .selected(&id_str)
                .map(str::to_owned)
                .or_else(|| options.first().cloned())
                .unwrap_or_default();
            let mut changed = false;
            ui.scope_builder(egui::UiBuilder::new().max_rect(r), |ui| {
                let combo = egui::ComboBox::from_id_salt(id_str.clone())
                    .selected_text(sel.clone())
                    .width(r.width());
                let resp = combo.show_ui(ui, |ui| {
                    for opt in &options {
                        ui.selectable_value(&mut sel, opt.clone(), opt.clone());
                    }
                });
                changed = resp.inner.is_some() || resp.response.changed();
            });
            if changed {
                state.set_selected(&id_str, sel);
            }
        }
        "progress" => {
            let track = color(theme.border);
            painter.rect_filled(r, egui::CornerRadius::same(4), track);
            let working = phase_working(phase);
            let (done, total) = progress
                .as_ref()
                .map(|p| (p.done as f32, p.total as f32))
                .unwrap_or((0.0, 1.0));
            let frac = if working && total > 0.0 { (done / total).clamp(0.0, 1.0) } else { 0.0 };
            if frac > 0.0 {
                let fill_w = r.width() * frac;
                let fill_r = egui::Rect::from_min_size(r.min, egui::vec2(fill_w, r.height()));
                painter.rect_filled(fill_r, egui::CornerRadius::same(4), color(theme.primary));
            }
            // 进度文案（引擎消息）
            if working
                && let Some(p) = progress {
                    painter.text(
                        egui::pos2(r.min.x, r.bottom() + 6.0),
                        egui::Align2::LEFT_TOP,
                        format!("{}（{}/{}）", p.message, p.done, p.total),
                        egui::FontId::proportional(12.0),
                        color(theme.text_muted),
                    );
                }
        }
        "image" => {
            if let Some(src) = node.attr("src")
                && let Some(texture) = assets.get(ui.ctx(), src, root) {
                    let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
                    painter.image(texture.id(), r, uv, egui::Color32::WHITE);
                }
        }
        "divider" => {
            painter.rect_filled(r, egui::CornerRadius::ZERO, color(theme.border));
        }
        "spacer" => {}
        _ => {
            // 未知标签已在解析期拦截；此处兜底
            log::debug!("渲染跳过未知标签 <{}>", node.tag);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::{Event, PointerButton, RawInput};
    use semantic_ui::layout::test_measure::Stub;
    use semantic_ui::layout::{layout_page, Rect as SrcRect};
    use semantic_ui::parser::parse;
    use semantic_ui::Theme as SrcTheme;

    /// 构造一个两按钮页面并布局（Stub 测量），返回 PageModel。
    fn two_button_model() -> PageModel {
        let src = r##"<window width="900" height="620">
          <page id="a" title="A">
            <column padding="44">
              <spacer/>
              <row gap="8" align="end">
                <button action="next" primary="true">下一步</button>
                <button action="cancel">取消</button>
              </row>
            </column>
          </page>
        </window>"##;
        let parsed = parse(src).expect("解析成功");
        let page = parsed
            .root
            .children
            .iter()
            .find(|n| n.is("page"))
            .expect("缺少 page");
        let theme = SrcTheme::light();
        layout_page(
            page,
            SrcRect::new(0.0, 0.0, 900.0, 620.0),
            &theme,
            &Stub,
        )
        .expect("布局成功")
    }

    /// 无头驱动两帧：首帧注册控件矩形，次帧在指定位置注入一次主键点击，
    /// 断言点击「下一步」按钮会产出 `action="next"` 的 Click。
    /// 这是针对"同页多按钮 Id 冲突导致点击无效"的回归测试。
    #[test]
    fn click_next_button_emits_next_action() {
        let model = two_button_model();
        let next_idx = model
            .nodes
            .iter()
            .position(|n| n.attr("action") == Some("next"))
            .expect("找不到下一步按钮");
        let rect = model.rects[next_idx];
        let pos = egui::pos2(rect.x + rect.w / 2.0, rect.y + rect.h / 2.0);

        let ctx = egui::Context::default();
        let mut state = UiState::default();
        let mut assets = AssetCache::default();
        let theme = SrcTheme::light();
        let root = Path::new("");

        let mut frame = |events: Vec<Event>, clicks: &mut Vec<Click>| {
            let input = RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(900.0, 620.0),
                )),
                events,
                ..Default::default()
            };
            let _output = ctx.run_ui(input, |ui| {
                render_page(
                    ui,
                    &model,
                    &mut state,
                    &Phase::Idle,
                    &None,
                    &theme,
                    &mut assets,
                    root,
                    &mut |c| clicks.push(c),
                );
            });
        };

        // 帧 1：仅注册交互矩形（egui 的点击判定基于上一帧矩形）
        frame(vec![], &mut Vec::new());
        // 帧 2：点击「下一步」中心
        let mut clicks = Vec::new();
        frame(
            vec![
                Event::PointerMoved(pos),
                Event::PointerButton {
                    pos,
                    button: PointerButton::Primary,
                    pressed: true,
                    modifiers: Default::default(),
                },
                Event::PointerButton {
                    pos,
                    button: PointerButton::Primary,
                    pressed: false,
                    modifiers: Default::default(),
                },
            ],
            &mut clicks,
        );
        assert!(
            clicks.iter().any(|c| c.action.as_deref() == Some("next")),
            "点击「下一步」应产出 next 动作，实际点击事件：{clicks:?}"
        );
    }
}
