//! 简单样式动画声明：`animate` 属性解析与模型。
//!
//! 语法（空格分隔的 token）：
//! ```text
//! animate = "<kind> [<duration>ms] [infinite] [delay <n>ms]"
//! kind    = fade-in | slide-up | slide-down | slide-left | slide-right
//!         | spin | pulse | scale-in
//! ```
//! 示例：`animate="fade-in 300ms"`、`animate="slide-up 400ms"`、
//! `animate="spin 1200ms infinite"`、`animate="pulse 800ms infinite"`、
//! `animate="scale-in 200ms delay 100ms"`。
//!
//! 缺省时长：fade/slide/scale-in 300ms，spin 1200ms，pulse 800ms。
//! 动画为渲染期纯时间函数（不侵入布局引擎），由适配层按帧插值应用。

/// 动画种类。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimationKind {
    /// 透明度 0 → 1。
    FadeIn,
    /// 自下而上位移 24px 并淡入。
    SlideUp,
    SlideDown,
    SlideLeft,
    SlideRight,
    /// 绕中心顺时针旋转（一周/循环）。
    Spin,
    /// 透明度呼吸脉冲（0.55 ↔ 1）。
    Pulse,
    /// 由 0.8 倍缩放至原尺寸并淡入。
    ScaleIn,
}

/// 解析后的动画声明。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AnimationSpec {
    pub kind: AnimationKind,
    pub duration_ms: u64,
    pub infinite: bool,
    /// 开始前的等待时间；等待期内节点保持静止可见。
    pub delay_ms: u64,
}

fn default_duration(kind: AnimationKind) -> u64 {
    match kind {
        AnimationKind::FadeIn
        | AnimationKind::SlideUp
        | AnimationKind::SlideDown
        | AnimationKind::SlideLeft
        | AnimationKind::SlideRight
        | AnimationKind::ScaleIn => 300,
        AnimationKind::Spin => 1200,
        AnimationKind::Pulse => 800,
    }
}

/// 解析 `animate` 属性值；非法 / 缺失返回 None（调用方按无动画处理）。
pub fn parse_animate(value: &str) -> Option<AnimationSpec> {
    let tokens: Vec<&str> = value.split_whitespace().collect();
    if tokens.is_empty() {
        return None;
    }
    let kind = match tokens[0] {
        "fade-in" => AnimationKind::FadeIn,
        "slide-up" => AnimationKind::SlideUp,
        "slide-down" => AnimationKind::SlideDown,
        "slide-left" => AnimationKind::SlideLeft,
        "slide-right" => AnimationKind::SlideRight,
        "spin" => AnimationKind::Spin,
        "pulse" => AnimationKind::Pulse,
        "scale-in" => AnimationKind::ScaleIn,
        _ => return None,
    };
    let mut spec = AnimationSpec {
        kind,
        duration_ms: default_duration(kind),
        infinite: false,
        delay_ms: 0,
    };
    let mut i = 1;
    while i < tokens.len() {
        let t = tokens[i];
        if t == "infinite" {
            spec.infinite = true;
            i += 1;
        } else if t == "delay" {
            let ms = tokens.get(i + 1).and_then(|x| parse_ms(x))?;
            spec.delay_ms = ms;
            i += 2;
        } else {
            let ms = parse_ms(t)?;
            spec.duration_ms = ms;
            i += 1;
        }
    }
    Some(spec)
}

fn parse_ms(t: &str) -> Option<u64> {
    t.strip_suffix("ms").and_then(|n| n.parse().ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_basic_forms() {
        let a = parse_animate("fade-in 300ms").unwrap();
        assert_eq!(a.kind, AnimationKind::FadeIn);
        assert_eq!(a.duration_ms, 300);
        assert!(!a.infinite);

        let b = parse_animate("slide-up 400ms").unwrap();
        assert_eq!(b.kind, AnimationKind::SlideUp);
        assert_eq!(b.duration_ms, 400);

        let c = parse_animate("spin 1200ms infinite").unwrap();
        assert_eq!(c.kind, AnimationKind::Spin);
        assert!(c.infinite);

        let d = parse_animate("scale-in 200ms delay 100ms").unwrap();
        assert_eq!(d.kind, AnimationKind::ScaleIn);
        assert_eq!(d.duration_ms, 200);
        assert_eq!(d.delay_ms, 100);
    }

    #[test]
    fn defaults_and_invalid() {
        assert_eq!(parse_animate("pulse").unwrap().duration_ms, 800);
        assert_eq!(parse_animate("spin").unwrap().duration_ms, 1200);
        assert_eq!(parse_animate("fade-in").unwrap().duration_ms, 300);
        assert!(parse_animate("").is_none());
        assert!(parse_animate("bounce 300ms").is_none());
        assert!(parse_animate("fade-in 300ms").is_some());
    }
}
