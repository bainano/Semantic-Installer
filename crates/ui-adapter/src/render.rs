//! 语义节点树 → egui 组件渲染。
//!
//! 布局矩形由固定画布布局引擎预先算好（`PageModel`），渲染阶段只做：
//! 画布定位、控件绘制、交互响应与事件收集。图标一律使用矢量绘制（Painter），
//! 不使用 emoji / 表情字符。
//! 动画为渲染期纯时间函数：fade / slide / pulse 对全部节点生效（透明度 / 位移），
//! spin / scale-in 对带形状的节点（容器背景、按钮、图片、进度条、分隔线）生效，
//! 纯文本与 egui 原生控件（input / checkbox / select）不旋转缩放。

use std::path::Path;

use eframe::egui;
use semantic_ui::animation::{parse_animate, AnimationKind, AnimationSpec};
use semantic_ui::ast::Node;
use semantic_ui::layout::{PageModel, Rect};
use semantic_ui::widget::{Click, UiState};
use semantic_ui::{FontWeight, Theme};

use crate::app::{phase_failed, phase_working, Phase};
use crate::assets::AssetCache;
use crate::theme::{self, color};

/// 节点动画的当前插值状态。
#[derive(Debug, Clone, Copy)]
struct AnimState {
    opacity: f32,
    offset: egui::Vec2,
    angle: f32,
    scale: f32,
}

fn ease_out(k: f64) -> f64 {
    1.0 - (1.0 - k) * (1.0 - k)
}

/// 按"页面进入后的秒数"计算节点动画状态；动画未开始 / 已结束返回 None（保持原样）。
fn compute_anim(spec: &AnimationSpec, since_s: f64) -> Option<AnimState> {
    let t_ms = since_s * 1000.0 - spec.delay_ms as f64;
    if t_ms < 0.0 {
        return None;
    }
    let dur = spec.duration_ms.max(1) as f64;
    let k = if spec.infinite {
        (t_ms % dur) / dur
    } else {
        (t_ms / dur).min(1.0)
    };
    if !spec.infinite && t_ms >= dur {
        return None;
    }
    let mut st = AnimState {
        opacity: 1.0,
        offset: egui::Vec2::ZERO,
        angle: 0.0,
        scale: 1.0,
    };
    let kk = ease_out(k);
    match spec.kind {
        AnimationKind::FadeIn => st.opacity = kk as f32,
        AnimationKind::SlideUp => {
            st.offset = egui::vec2(0.0, 24.0 * (1.0 - kk) as f32);
            st.opacity = kk as f32;
        }
        AnimationKind::SlideDown => {
            st.offset = egui::vec2(0.0, -24.0 * (1.0 - kk) as f32);
            st.opacity = kk as f32;
        }
        AnimationKind::SlideLeft => {
            st.offset = egui::vec2(24.0 * (1.0 - kk) as f32, 0.0);
            st.opacity = kk as f32;
        }
        AnimationKind::SlideRight => {
            st.offset = egui::vec2(-24.0 * (1.0 - kk) as f32, 0.0);
            st.opacity = kk as f32;
        }
        AnimationKind::Pulse => {
            let phase = k * std::f64::consts::TAU;
            st.opacity = (0.55 + 0.45 * ((phase.sin() + 1.0) / 2.0)) as f32;
        }
        AnimationKind::Spin => st.angle = (k * std::f64::consts::TAU) as f32,
        AnimationKind::ScaleIn => {
            st.scale = (0.8 + 0.2 * kk) as f32;
            st.opacity = kk as f32;
        }
    }
    Some(st)
}

/// 绘制矩形（支持旋转角；angle=0 时等价于普通填充）。
fn fill_rect(
    painter: &egui::Painter,
    rect: egui::Rect,
    radius: egui::CornerRadius,
    fill: egui::Color32,
    angle: f32,
) {
    if angle != 0.0 {
        let s = egui::epaint::RectShape::filled(rect, radius, fill).with_angle(angle);
        painter.add(egui::Shape::Rect(s));
    } else {
        painter.rect_filled(rect, radius, fill);
    }
}

fn stroke_rect(
    painter: &egui::Painter,
    rect: egui::Rect,
    radius: egui::CornerRadius,
    stroke: egui::Stroke,
    angle: f32,
) {
    if angle != 0.0 {
        let s = egui::epaint::RectShape::stroke(rect, radius, stroke, egui::StrokeKind::Inside)
            .with_angle(angle);
        painter.add(egui::Shape::Rect(s));
    } else {
        painter.rect_stroke(rect, radius, stroke, egui::StrokeKind::Inside);
    }
}

/// 渲染当前页面并收集点击事件（Ui 中心化：直接绘制在给定 Ui 上）。
/// `anim_t` 为页面进入后的秒数（动画时间基准）。
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
    anim_t: f64,
    emit: &mut dyn FnMut(Click),
) {
    // 固定画布背景
    let canvas = ui.max_rect();
    ui.painter()
        .rect_filled(canvas, egui::CornerRadius::ZERO, color(theme.bg));

    let valid = semantic_ui::widget::page_valid(model, state);
    if model.layers.is_empty() {
        // 单层旧路径：页面扁平列表直接渲染
        for (i, node) in model.nodes.iter().enumerate() {
            let rect = model.rects[i];
            render_node(
                ui, node, &rect, i, valid, state, phase, progress, theme, assets, root, anim_t,
                emit,
            );
        }
    } else {
        // 图层模式：按 z 序逐层渲染，层内节点全局唯一序号（跨层 Id 不冲突）
        let mut global = 0usize;
        for layer in &model.layers {
            for (j, node) in layer.nodes.iter().enumerate() {
                let rect = layer.rects[j];
                render_node(
                    ui, node, &rect, global, valid, state, phase, progress, theme, assets, root,
                    anim_t, emit,
                );
                global += 1;
            }
        }
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
/// `anim_t` 为页面进入后的秒数，驱动样式动画的时间插值。
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
    anim_t: f64,
    emit: &mut dyn FnMut(Click),
) {
    let r = rect_of(rect);
    let id = node_id(node, index);

    // ---- 动画状态：位移 / 透明度 / 旋转 / 缩放 ----
    let anim = node
        .attr("animate")
        .and_then(parse_animate)
        .and_then(|s| compute_anim(&s, anim_t));
    let is_shape = matches!(
        node.tag.as_str(),
        "row" | "column" | "layer" | "page" | "button" | "image" | "divider" | "progress"
    );
    let (pr, painter, hit_r) = if let Some(a) = &anim {
        if a.angle == 0.0 && a.scale == 1.0 && a.opacity >= 1.0 && a.offset == egui::Vec2::ZERO {
            // 无实际变换（理论上不会出现），走原路径
            (r, ui.painter().clone(), r)
        } else {
            let mut p = ui.painter().clone();
            if a.opacity < 1.0 {
                p.set_opacity(a.opacity);
            }
            let mut pr = r.translate(a.offset);
            if a.scale != 1.0 {
                pr = egui::Rect::from_center_size(pr.center(), pr.size() * a.scale);
            }
            // 动画进行中：请求持续重绘（一次性动画结束后 compute_anim 返回 None，自然停止）
            ui.ctx().request_repaint();
            (pr, p, pr)
        }
    } else {
        (r, ui.painter().clone(), r)
    };
    let angle = anim
        .as_ref()
        .filter(|a| a.angle != 0.0 && is_shape)
        .map(|a| a.angle)
        .unwrap_or(0.0);

    match node.tag.as_str() {
        "page" | "layer" | "row" | "column" => {
            if let Some(bg) = node.attr_color("bg") {
                fill_rect(&painter, pr, egui::CornerRadius::ZERO, color(bg), angle);
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
                egui::Align2::CENTER_TOP => pr.center_top(),
                egui::Align2::RIGHT_TOP => pr.right_top(),
                _ => pr.left_top(),
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
            let pos = pr.left_top();
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
                [egui::pos2(pr.min.x, pr.bottom() - 2.0), egui::pos2(pr.min.x + tw, pr.bottom() - 2.0)],
                egui::Stroke::new(1.0, c),
            );
            let resp = ui.interact(hit_r, egui::Id::new(id.clone()), egui::Sense::click());
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
            let hovered = ui.rect_contains_pointer(hit_r);
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
            fill_rect(&painter, pr, radius, fill, angle);
            stroke_rect(
                &painter,
                pr,
                radius,
                egui::Stroke::new(1.0, color(theme.border)),
                angle,
            );
            painter.text(
                pr.center(),
                egui::Align2::CENTER_CENTER,
                node.text_of(""),
                egui::FontId::proportional(font_size(node, theme)),
                fg,
            );
            if disabled {
                return;
            }
            let resp = ui.interact(hit_r, egui::Id::new(id.clone()), egui::Sense::click());
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
            let field_rect = egui::Rect::from_min_size(hit_r.min, egui::vec2((hit_r.width() - browse_w - 8.0).max(40.0), hit_r.height()));
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
                    egui::pos2(hit_r.right() - browse_w, hit_r.top()),
                    egui::vec2(browse_w, hit_r.height()),
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
            let resp = ui.put(hit_r, egui::Checkbox::new(&mut checked, label));
            if resp.changed() {
                state.set_checked(&id_str, checked);
            }
        }
        "radio" => {
            let group = node.attr("group").unwrap_or_default().to_owned();
            let value = node.attr("value").unwrap_or_default().to_owned();
            let selected = state.radio(&group) == Some(value.as_str());
            // 自绘单选：圆 + 标签
            let center = egui::pos2(pr.min.x + 9.0, pr.center().y);
            painter.circle_filled(center, 8.0, color(theme.panel));
            painter.circle_stroke(center, 8.0, egui::Stroke::new(1.5, color(theme.border)));
            if selected {
                painter.circle_filled(center, 4.0, color(theme.primary));
            }
            painter.text(
                egui::pos2(pr.min.x + 26.0, pr.center().y),
                egui::Align2::LEFT_CENTER,
                node.text_of(""),
                egui::FontId::proportional(font_size(node, theme)),
                color(theme.text),
            );
            let resp = ui.interact(hit_r, egui::Id::new(node_id(node, index)), egui::Sense::click());
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
            ui.scope_builder(egui::UiBuilder::new().max_rect(hit_r), |ui| {
                let combo = egui::ComboBox::from_id_salt(id_str.clone())
                    .selected_text(sel.clone())
                    .width(hit_r.width());
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
            fill_rect(&painter, pr, egui::CornerRadius::same(4), track, angle);
            let working = phase_working(phase);
            let (done, total) = progress
                .as_ref()
                .map(|p| (p.done as f32, p.total as f32))
                .unwrap_or((0.0, 1.0));
            let target = if working && total > 0.0 {
                (done / total).clamp(0.0, 1.0)
            } else {
                0.0
            };
            // 进度平滑：朝目标值指数趋近（约 120ms 到达），避免跳变
            let frac = smooth_progress(ui, &id, target);
            if frac > 0.0 {
                let fill_w = pr.width() * frac;
                let fill_r = egui::Rect::from_min_size(pr.min, egui::vec2(fill_w, pr.height()));
                fill_rect(&painter, fill_r, egui::CornerRadius::same(4), color(theme.primary), 0.0);
            }
            // 进度文案（引擎消息）
            if working
                && let Some(p) = progress {
                    painter.text(
                        egui::pos2(pr.min.x, pr.bottom() + 6.0),
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
                    if angle != 0.0 {
                        let s = egui::epaint::RectShape::filled(pr, egui::CornerRadius::ZERO, egui::Color32::WHITE)
                            .with_texture(texture.id(), uv)
                            .with_angle(angle);
                        painter.add(egui::Shape::Rect(s));
                    } else {
                        painter.image(texture.id(), pr, uv, egui::Color32::WHITE);
                    }
                }
        }
        "divider" => {
            fill_rect(&painter, pr, egui::CornerRadius::ZERO, color(theme.border), angle);
        }
        "spacer" => {}
        _ => {
            // 未知标签已在解析期拦截；此处兜底
            log::debug!("渲染跳过未知标签 <{}>", node.tag);
        }
    }
}

/// 进度平滑：每帧按时间常数朝目标值指数趋近（约 120ms 到达），避免进度跳变。
#[derive(Clone, Copy, Default)]
struct ProgressSmooth {
    value: f32,
    time: f64,
}

fn smooth_progress(ui: &egui::Ui, node_id: &str, target: f32) -> f32 {
    let now = ui.input(|i| i.time);
    ui.data_mut(|d| {
        let key = egui::Id::new(("smooth-progress", node_id));
        let mut s = d.get_temp::<ProgressSmooth>(key).unwrap_or_default();
        let dt = (now - s.time).clamp(0.0, 0.2);
        let alpha = ((dt / 0.12).min(1.0)) as f32;
        s.value += (target - s.value) * alpha;
        if (s.value - target).abs() < 0.002 {
            s.value = target;
        }
        s.time = now;
        d.insert_temp(key, s);
        s.value
    })
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
                    0.0,
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
