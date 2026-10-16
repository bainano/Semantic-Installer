//! 语义组件模型与运行时 UI 状态（输入值 / 勾选 / 选中项 / 点击事件）。

use std::collections::HashMap;

use crate::layout::PageModel;

/// 一次按钮 / 链接点击事件，由适配层渲染时收集、渲染后统一分发。
#[derive(Debug, Clone, PartialEq)]
pub struct Click {
    /// 触发节点的 id（无 id 时用 `tag:index` 兜底）。
    pub widget_id: String,
    /// `action` 属性：next / back / install / uninstall / cancel / finish / custom:*。
    pub action: Option<String>,
    /// `goto` 属性：显式跳转的页面 id。
    pub goto: Option<String>,
    /// `url` 属性（link 标签）。
    pub url: Option<String>,
}

/// 运行时 UI 状态：与静态节点树分离，跨页面保留。
#[derive(Debug, Clone, Default)]
pub struct UiState {
    pub inputs: HashMap<String, String>,
    pub checks: HashMap<String, bool>,
    pub selects: HashMap<String, String>,
    /// 单选组：group id -> 已选值。
    pub radios: HashMap<String, String>,
    clicks: Vec<Click>,
}

impl UiState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn input(&self, id: &str) -> &str {
        self.inputs.get(id).map(String::as_str).unwrap_or("")
    }

    pub fn set_input(&mut self, id: impl Into<String>, v: impl Into<String>) {
        self.inputs.insert(id.into(), v.into());
    }

    pub fn checked(&self, id: &str) -> bool {
        self.checks.get(id).copied().unwrap_or(false)
    }

    pub fn set_checked(&mut self, id: impl Into<String>, v: bool) {
        self.checks.insert(id.into(), v);
    }

    pub fn selected(&self, id: &str) -> Option<&str> {
        self.selects.get(id).map(String::as_str)
    }

    pub fn set_selected(&mut self, id: impl Into<String>, v: impl Into<String>) {
        self.selects.insert(id.into(), v.into());
    }

    pub fn radio(&self, group: &str) -> Option<&str> {
        self.radios.get(group).map(String::as_str)
    }

    pub fn set_radio(&mut self, group: impl Into<String>, value: impl Into<String>) {
        self.radios.insert(group.into(), value.into());
    }

    pub fn push_click(&mut self, c: Click) {
        self.clicks.push(c);
    }

    pub fn drain_clicks(&mut self) -> Vec<Click> {
        std::mem::take(&mut self.clicks)
    }
}

/// 页面有效性：所有 `required` 输入非空、`required` 复选框已勾选。
/// 适配层据此自动禁用 `next` / `install` 等前进型按钮。
pub fn page_valid(model: &PageModel, state: &UiState) -> bool {
    for node in &model.nodes {
        match node.tag.as_str() {
            "input" => {
                if node.flag("required", false)
                    && let Some(id) = node.attr("id")
                        && state.input(id).trim().is_empty() {
                            return false;
                        }
            }
            "checkbox" => {
                if node.flag("required", false)
                    && let Some(id) = node.attr("id")
                        && !state.checked(id) {
                            return false;
                        }
            }
            _ => {}
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::Rect;
    use crate::parser::parse;

    fn model(src: &str) -> PageModel {
        let res = parse(src).expect("解析成功");
        let page = res.root.children.iter().find(|n| n.is("page")).unwrap();
        crate::layout::layout_page(
            page,
            Rect::new(0.0, 0.0, 800.0, 600.0),
            &crate::theme::Theme::light(),
            &crate::layout::test_measure::Stub,
        )
        .unwrap()
    }

    #[test]
    fn required_input_blocks_validity() {
        let m = model(
            r#"<window width="1" height="1"><page id="p"><input id="dir" required="true"/></page></window>"#,
        );
        let mut s = UiState::new();
        assert!(!page_valid(&m, &s));
        s.set_input("dir", "C:\\x");
        assert!(page_valid(&m, &s));
    }

    #[test]
    fn required_checkbox_blocks_validity() {
        let m = model(
            r#"<window width="1" height="1"><page id="p"><checkbox id="ok" required="true">接受</checkbox></page></window>"#,
        );
        let mut s = UiState::new();
        assert!(!page_valid(&m, &s));
        s.set_checked("ok", true);
        assert!(page_valid(&m, &s));
    }

    #[test]
    fn clicks_drain() {
        let mut s = UiState::new();
        s.push_click(Click {
            widget_id: "b".into(),
            action: Some("next".into()),
            goto: None,
            url: None,
        });
        assert_eq!(s.drain_clicks().len(), 1);
        assert!(s.drain_clicks().is_empty());
    }
}
