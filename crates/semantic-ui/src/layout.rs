//! 固定画布布局引擎。
//!
//! 设计原则（对应"安装程序尺寸固定"的产品取舍）：
//! - 每个页面在固定画布（`window` 宽高）上做**确定性**布局；
//! - 节点主轴尺寸支持 `Auto`（内容自适应）/ `Fixed(px)` / `Fill`（均分剩余）/ `Weight(n)`（按权重分配）；
//! - 内容超出画布产生溢出警告（不做滚动 / 自适应），由适配层与文档明确约束。

use crate::ast::Node;
use crate::theme::Theme;

/// 布局矩形（与 egui 解耦的纯数据）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }
    pub fn right(&self) -> f32 {
        self.x + self.w
    }
    pub fn bottom(&self) -> f32 {
        self.y + self.h
    }
    /// 四周等距内缩。
    pub fn inset(&self, p: f32) -> Self {
        Self {
            x: self.x + p,
            y: self.y + p,
            w: (self.w - 2.0 * p).max(0.0),
            h: (self.h - 2.0 * p).max(0.0),
        }
    }
}

/// 节点在主轴上的尺寸模式。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Size {
    /// 内容自适应
    Auto,
    /// 固定像素
    Fixed(f32),
    /// 占满剩余空间（多个 Fill 均分）
    Fill,
    /// 按权重分配剩余空间
    Weight(f32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    Start,
    Center,
    End,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontWeight {
    Normal,
    Bold,
}

/// 文本测量抽象：由 GUI 适配层提供真实字体度量；测试用桩实现。
pub trait Measurer {
    fn text_size(&self, text: &str, size: f32, weight: FontWeight) -> (f32, f32);
}

/// 测试与示例共用的文本测量桩（按字符数估算宽度）。
#[doc(hidden)]
pub mod test_measure {
    use super::{FontWeight, Measurer};

    pub struct Stub;

    impl Measurer for Stub {
        fn text_size(&self, text: &str, size: f32, _w: FontWeight) -> (f32, f32) {
            // 与真实测量一致：宽度 = 最长行宽，高度 = 行数 × 行高
            let mut max_w = 0.0f32;
            let mut lines = 0usize;
            for line in text.split('\n') {
                max_w = max_w.max(line.chars().count() as f32 * size * 0.62);
                lines += 1;
            }
            (max_w, lines as f32 * size * 1.4)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Axis {
    H,
    V,
}

impl Axis {
    fn main_attr(self) -> &'static str {
        match self {
            Axis::H => "width",
            Axis::V => "height",
        }
    }
    fn cross_attr(self) -> &'static str {
        match self {
            Axis::H => "height",
            Axis::V => "width",
        }
    }
}

#[derive(Debug, Default)]
pub struct LayoutResult {
    /// 与扁平化前序节点一一对应的矩形。
    pub rects: Vec<Rect>,
    /// 溢出 / 缺省属性等警告。
    pub warnings: Vec<String>,
}

/// 单个图层的布局模型：与页面同画布、独立布局、默认透明背景，
/// z 序由 `PageModel::layers` 中声明顺序决定（后声明者在上）。
#[derive(Debug)]
pub struct LayerModel {
    pub id: Option<String>,
    pub nodes: Vec<Node>,
    pub rects: Vec<Rect>,
}

/// 单个页面的布局模型：扁平化节点 + 对应矩形。
/// 当页面包含 `<layer>` 子节点时，`layers` 非空（渲染按层叠加）；
/// 否则 `layers` 为空（渲染沿用 `nodes`/`rects` 单层旧路径）。
#[derive(Debug)]
pub struct PageModel {
    pub id: String,
    pub title: Option<String>,
    pub nodes: Vec<Node>,
    pub rects: Vec<Rect>,
    pub warnings: Vec<String>,
    pub canvas: Rect,
    pub layers: Vec<LayerModel>,
}

pub fn parse_align(s: Option<&str>) -> Align {
    match s {
        Some("center") => Align::Center,
        Some("end") | Some("right") => Align::End,
        _ => Align::Start,
    }
}

pub fn parse_weight(s: Option<&str>) -> FontWeight {
    match s {
        Some("bold") | Some("bolder") => FontWeight::Bold,
        _ => FontWeight::Normal,
    }
}

/// 解析尺寸属性："fill" / "auto" / 数字。
pub fn parse_size(s: Option<&str>) -> Size {
    match s {
        Some("fill") => Size::Fill,
        Some("auto") => Size::Auto,
        Some(v) => v
            .trim()
            .parse::<f64>()
            .map(|n| Size::Fixed(n as f32))
            .unwrap_or(Size::Auto),
        None => Size::Auto,
    }
}

fn is_container(node: &Node) -> bool {
    node.is("row") || node.is("column") || node.is("layer") || node.is("page")
}

/// 深度优先扁平化（克隆节点，index 0 为 page 自身）。
pub fn flatten(node: &Node) -> Vec<Node> {
    let mut out = Vec::new();
    fn go(n: &Node, out: &mut Vec<Node>) {
        out.push(n.clone());
        for c in &n.children {
            go(c, out);
        }
    }
    go(node, &mut out);
    out
}

/// 在固定画布上为页面做完整布局。
pub fn layout_page(
    page: &Node,
    canvas: Rect,
    theme: &Theme,
    m: &dyn Measurer,
) -> Result<PageModel, String> {
    let id = page
        .attr("id")
        .ok_or_else(|| format!("page（第 {} 行）缺少 id 属性", page.span.line))?
        .to_owned();
    let mut res = LayoutResult::default();
    // 图层模式：page 直接子节点为 <layer> 时，每个图层在整幅画布上独立布局；
    // 此时跳过整页普通布局，避免对图层容器重复计算产生溢出噪音。
    let has_layers = page.children.iter().any(|c| c.is("layer"));
    let mut layers = Vec::new();
    if has_layers {
        for layer in &page.children {
            if !layer.is("layer") {
                res.warnings.push(format!(
                    "图层模式：<page> 直接子节点 <{}>（第 {} 行）不在任何图层内，将被忽略。",
                    layer.tag,
                    layer.span.line
                ));
                continue;
            }
            if layer.children.iter().any(|c| c.is("layer")) {
                res.warnings.push(format!(
                    "<layer>（第 {} 行）不允许嵌套图层，嵌套内容将按普通容器处理。",
                    layer.span.line
                ));
            }
            let mut lres = LayoutResult::default();
            layout_node(layer, canvas, theme, m, &mut lres);
            res.warnings.extend(lres.warnings);
            layers.push(LayerModel {
                id: layer.attr("id").map(str::to_owned),
                nodes: flatten(layer),
                rects: lres.rects,
            });
        }
    } else {
        layout_node(page, canvas, theme, m, &mut res);
    }

    Ok(PageModel {
        id,
        title: page.attr("title").map(str::to_owned),
        nodes: flatten(page),
        rects: res.rects,
        warnings: res.warnings,
        canvas,
        layers,
    })
}

fn font_size(node: &Node, theme: &Theme) -> f32 {
    node.attr_float("size", theme.font_size).max(4.0)
}

fn text_weight(node: &Node) -> FontWeight {
    parse_weight(node.attr("weight"))
}

/// 叶子节点固有尺寸 `(w, h)`。
fn intrinsic_leaf(node: &Node, theme: &Theme, m: &dyn Measurer) -> (f32, f32) {
    match node.tag.as_str() {
        "text" | "label" | "link" => {
            let (w, h) = m.text_size(&node.text_of(""), font_size(node, theme), text_weight(node));
            (w, h)
        }
        "button" => {
            let (tw, th) = m.text_size(&node.text_of(""), font_size(node, theme), FontWeight::Normal);
            ((tw + 32.0).max(88.0), (th + 14.0).max(36.0))
        }
        "input" | "select" => {
            let w = node.attr_float("width", if node.is("select") { 200.0 } else { 260.0 });
            let h = (font_size(node, theme) * 1.9).clamp(28.0, 36.0);
            (w, h)
        }
        "checkbox" | "radio" => {
            let (tw, th) = m.text_size(&node.text_of(""), font_size(node, theme), FontWeight::Normal);
            (18.0 + 8.0 + tw, th.max(22.0))
        }
        "progress" => (0.0, 8.0),
        "divider" => (0.0, 1.0),
        "image" => (
            node.attr_float("width", 0.0),
            node.attr_float("height", 64.0),
        ),
        _ => (0.0, 0.0),
    }
}

/// 节点固有尺寸 `(w, h)`；容器递归求和。
fn intrinsic(node: &Node, theme: &Theme, m: &dyn Measurer) -> (f32, f32) {
    if !is_container(node) {
        return intrinsic_leaf(node, theme, m);
    }
    let axis = if node.is("row") { Axis::H } else { Axis::V };
    let gap = node.attr_float("gap", theme.spacing);
    let pad = node.attr_float("padding", 0.0);
    let mut main_sum = pad * 2.0;
    let mut cross_max = pad * 2.0;
    for (n, c) in node.children.iter().enumerate() {
        let (cw, ch) = intrinsic(c, theme, m);
        let (main, cross) = match axis {
            Axis::H => (cw, ch),
            Axis::V => (ch, cw),
        };
        // Fill/Weight 子节点在固有计算中视为 0（由剩余空间分配）
        let main = if matches!(child_main_size(c, axis), Size::Fill | Size::Weight(_)) {
            0.0
        } else {
            main
        };
        if n > 0 {
            main_sum += gap;
        }
        main_sum += main;
        cross_max = cross_max.max(cross);
    }
    match axis {
        Axis::H => (main_sum, cross_max),
        Axis::V => (cross_max, main_sum),
    }
}

/// 子节点主轴声明尺寸（默认按标签语义）。
fn child_main_size(child: &Node, axis: Axis) -> Size {
    // 先解析显式主轴尺寸属性（spacer 也可声明固定 width/height）
    let attr = axis.main_attr();
    if let Some(v) = child.attr(attr) {
        return parse_size(Some(v));
    }
    if child.is("spacer") {
        return Size::Fill;
    }
    if let Some(w) = child.attr_f64("weight")
        && w > 0.0 {
            return Size::Weight(w as f32);
        }
    Size::Auto
}

/// 子节点跨轴尺寸解析。
fn child_cross_size(
    child: &Node,
    container_cross: f32,
    axis: Axis,
    theme: &Theme,
    m: &dyn Measurer,
) -> f32 {
    let attr = axis.cross_attr();
    let cross_of = |n: &Node| {
        let (w, h) = intrinsic(n, theme, m);
        match axis {
            Axis::H => h,
            Axis::V => w,
        }
    };
    if let Some(v) = child.attr(attr) {
        return match parse_size(Some(v)) {
            Size::Fill => container_cross,
            Size::Fixed(n) => n.min(container_cross).max(0.0),
            _ => cross_of(child).min(container_cross),
        };
    }
    if is_container(child) {
        return container_cross; // 容器跨轴默认撑满
    }
    match child.tag.as_str() {
        // 列中横向撑满
        "progress" | "divider" if axis == Axis::V => container_cross,
        "image" if axis == Axis::V => container_cross,
        _ => cross_of(child).min(container_cross),
    }
}

fn layout_node(node: &Node, rect: Rect, theme: &Theme, m: &dyn Measurer, res: &mut LayoutResult) {
    res.rects.push(rect);
    if !is_container(node) {
        return;
    }
    let axis = if node.is("row") { Axis::H } else { Axis::V };
    let gap = node.attr_float("gap", theme.spacing);
    let pad = node.attr_float("padding", 0.0);
    let align = parse_align(node.attr("align"));
    let inner = rect.inset(pad);
    let (inner_main, inner_cross) = match axis {
        Axis::H => (inner.w, inner.h),
        Axis::V => (inner.h, inner.w),
    };

    // 1. 解析每个子节点主轴尺寸
    let mut mains: Vec<f32> = Vec::with_capacity(node.children.len());
    let mut fill_count = 0usize;
    let mut weight_sum = 0.0f32;
    for c in &node.children {
        let main_of = |n: &Node| {
            let (w, h) = intrinsic(n, theme, m);
            match axis {
                Axis::H => w,
                Axis::V => h,
            }
        };
        match child_main_size(c, axis) {
            Size::Auto => mains.push(main_of(c)),
            Size::Fixed(n) => mains.push(n.max(0.0)),
            Size::Fill => {
                fill_count += 1;
                mains.push(0.0);
            }
            Size::Weight(w) => {
                weight_sum += w.max(0.0);
                mains.push(0.0);
            }
        }
    }
    let n = node.children.len();
    let fixed_total: f32 = mains.iter().sum::<f32>() + gap * (n.saturating_sub(1) as f32);
    let remaining = inner_main - fixed_total;
    // 浮点容差：|剩余| < 0.5px 视为恰好铺满，不报溢出
    if remaining < -0.5 {
        res.warnings.push(format!(
            "布局溢出：<{}>（第 {} 行）内容超出 {}px，超出部分将被裁剪。请调整属性或缩小画布。",
            node.tag,
            node.span.line,
            (-remaining).round()
        ));
    }
    // 2. 分配剩余空间
    let distribute = remaining.max(0.0);
    let mut allocated = 0.0f32;
    if fill_count > 0 || weight_sum > 0.0 {
        let fill_share = if fill_count > 0 {
            distribute / fill_count as f32
        } else {
            0.0
        };
        for (i, c) in node.children.iter().enumerate() {
            if matches!(child_main_size(c, axis), Size::Fill | Size::Weight(_)) {
                let share = match child_main_size(c, axis) {
                    Size::Fill => fill_share,
                    Size::Weight(w) => distribute * (w.max(0.0)) / weight_sum.max(1e-6),
                    _ => 0.0,
                };
                mains[i] = share;
                allocated += share;
            }
        }
    }

    // 3. 计算起始偏移（align 作用于"剩余空间分配后的实际剩余"，
    //    避免与 Fill/Weight 子节点（如 spacer）重复占用空间）
    let leftover = (remaining - allocated).max(0.0);
    let start_offset = if leftover > 0.0 {
        match align {
            Align::Start => 0.0,
            Align::Center => leftover / 2.0,
            Align::End => leftover,
        }
    } else {
        0.0
    };

    // 4. 定位并递归布局
    let mut cursor = start_offset;
    for (i, c) in node.children.iter().enumerate() {
        let main = mains[i];
        let cross = child_cross_size(c, inner_cross, axis, theme, m);
        let child_rect = match axis {
            Axis::H => Rect::new(inner.x + cursor, inner.y, main, cross),
            Axis::V => Rect::new(inner.x, inner.y + cursor, cross, main),
        };
        layout_node(c, child_rect, theme, m, res);
        cursor += main + gap;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse;

    /// 测试用测量桩：按字符数估算文本宽度（宽度 = 最长行，高度 = 行数 × 行高）。
    struct StubMeasurer;

    impl Measurer for StubMeasurer {
        fn text_size(&self, text: &str, size: f32, _w: FontWeight) -> (f32, f32) {
            let mut max_w = 0.0f32;
            let mut lines = 0usize;
            for line in text.split('\n') {
                max_w = max_w.max(line.chars().count() as f32 * size * 0.62);
                lines += 1;
            }
            (max_w, lines as f32 * size * 1.4)
        }
    }

    fn layout(src: &str) -> PageModel {
        let res = parse(src).expect("解析成功");
        let page = res.root.children.iter().find(|n| n.is("page")).expect("有 page");
        let theme = Theme::light();
        layout_page(page, Rect::new(0.0, 0.0, 900.0, 620.0), &theme, &StubMeasurer)
            .expect("布局成功")
    }

    #[test]
    fn row_children_are_laid_left_to_right() {
        let model = layout(
            r#"<window width="900" height="620"><page id="p"><row><text>x</text><text>y</text></row></page></window>"#,
        );
        // 0: page, 1: row, 2: text x, 3: text y
        let row = model.rects[1];
        let tx = model.rects[2];
        let ty = model.rects[3];
        assert!(tx.x < ty.x, "x 应在 y 左侧");
        assert_eq!(tx.y, ty.y);
        assert!(row.w >= tx.w + ty.w);
    }

    #[test]
    fn spacer_pushes_content_to_end() {
        let model = layout(
            r#"<window width="900" height="620"><page id="p"><row align="end"><spacer/><text>a</text><text>b</text></row></page></window>"#,
        );
        let row = model.rects[1];
        // 扁平化顺序：0=page, 1=row, 2=spacer, 3=text a, 4=text b
        let spacer = model.rects[2];
        let a = model.rects[3];
        let b = model.rects[4];
        // 文本在行尾
        assert!(
            (b.right() - row.right()).abs() < 0.5,
            "b 应贴住行右缘：b={b:?} row={row:?}"
        );
        assert!(a.x > spacer.x, "a 应被 spacer 推到右侧");
    }

    #[test]
    fn fixed_width_column_and_bg_rect() {
        let model = layout(
            r#"<window width="900" height="620"><page id="p"><row><column width="240"><text>s</text></column></row></page></window>"#,
        );
        let column = model.rects[2];
        assert!((column.w - 240.0).abs() < 0.5);
    }

    #[test]
    fn overflow_warns() {
        // 列高 20px 装不下字号 20 的文本（固有高 28px）→ 主轴溢出
        let model = layout(
            r#"<window width="900" height="620"><page id="p"><column height="20"><text size="20">文本</text></column></page></window>"#,
        );
        assert!(!model.warnings.is_empty(), "应有溢出警告");
    }

    #[test]
    fn weight_distributes_remaining() {
        let model = layout(
            r#"<window width="900" height="620"><page id="p"><row><text>a</text><text weight="2">b</text><text weight="1">c</text></row></page></window>"#,
        );
        let a = model.rects[2];
        let b = model.rects[3];
        let c = model.rects[4];
        let row_w = model.rects[1].w;
        // b 占剩余 2/3，c 占 1/3
        let total_gap = 8.0 * 2.0;
        let aw = a.w;
        let rem = (row_w - total_gap - aw).max(0.0);
        assert!((b.w - rem * 2.0 / 3.0).abs() < 2.0, "b 宽度应约为剩余 2/3");
        assert!((c.w - rem * 1.0 / 3.0).abs() < 2.0, "c 宽度应约为剩余 1/3");
    }

    #[test]
    fn page_fills_canvas() {
        let model = layout(
            r#"<window width="900" height="620"><page id="p"><text>hi</text></page></window>"#,
        );
        let page = model.rects[0];
        assert_eq!(page.w, 900.0);
        assert_eq!(page.h, 620.0);
    }

    /// 图层前景层：spacer 声明固定宽度时应让出对应空间（column 不被推到右缘）。
    #[test]
    fn layer_fg_spacer_fixed_width_keeps_column_left() {
        let model = layout(
            r#"<window width="900" height="620"><page id="p">
              <layer id="bg"><image src="bg.png" width="260" height="620"/></layer>
              <layer id="fg"><row><spacer width="260"/><column padding="40"><text size="30">Demo 安装程序</text></column></row></layer>
            </page></window>"#,
        );
        let fg = &model.layers[1];
        // 扁平顺序：0=layer, 1=row, 2=spacer, 3=column, 4=text
        let spacer = fg.rects[2];
        let column = fg.rects[3];
        let text = fg.rects[4];
        assert!(
            (spacer.w - 260.0).abs() < 0.5,
            "spacer 固定宽度 260 应生效：{spacer:?}"
        );
        assert!(
            column.x > spacer.w && column.x < spacer.w + 16.0,
            "column 应紧贴 spacer 之后：{column:?}"
        );
        assert!(
            column.w > 100.0 && text.x >= column.x && text.right() <= column.right() + 0.5,
            "text 应在 column 内完整排布：{text:?} {column:?}"
        );
    }

    #[test]
    fn layer_models_stack_on_full_canvas() {
        let model = layout(
            r#"<window width="900" height="620"><page id="p">
              <layer id="bg"><image src="bg.png" width="900" height="620"/></layer>
              <layer id="fg"><column padding="36"><text>hi</text></column></layer>
            </page></window>"#,
        );
        assert_eq!(model.layers.len(), 2, "应生成 2 个图层");
        let bg = &model.layers[0];
        let fg = &model.layers[1];
        assert_eq!(bg.id.as_deref(), Some("bg"));
        assert_eq!(fg.id.as_deref(), Some("fg"));
        // 每层节点 0 = layer 自身，矩形 = 整幅画布（z 序叠加、独立布局）
        assert_eq!(bg.rects[0], Rect::new(0.0, 0.0, 900.0, 620.0));
        assert_eq!(fg.rects[0], Rect::new(0.0, 0.0, 900.0, 620.0));
        assert_eq!(bg.nodes.len(), 2, "bg 层：layer + image");
        // 前景层文字独立布局，不受背景层内容影响
        assert_eq!(fg.nodes[2].tag, "text");
        assert_eq!(fg.nodes[2].text_of(""), "hi");
        // 两层子节点矩形互不引用（各自独立计数）
        assert!(bg.rects[1].w > 0.0 && fg.rects[2].w > 0.0);
    }

    #[test]
    fn page_without_layers_keeps_legacy_path() {
        let model = layout(
            r#"<window width="900" height="620"><page id="p"><text>hi</text></page></window>"#,
        );
        assert!(model.layers.is_empty(), "无 <layer> 时保持单层旧路径");
    }

    #[test]
    fn mixed_page_children_warn_when_layers_present() {
        let model = layout(
            r#"<window width="900" height="620"><page id="p">
              <layer id="bg"><image src="bg.png" width="900" height="620"/></layer>
              <text>游离节点</text>
            </page></window>"#,
        );
        assert_eq!(model.layers.len(), 1);
        assert!(
            model.warnings.iter().any(|w| w.contains("游离节点") || w.contains("将被忽略")),
            "图层模式下游离子节点应产生警告：{:?}",
            model.warnings
        );
    }
}
