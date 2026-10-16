//! 语义标签文档 AST：节点、属性与查询辅助方法。

/// 源码位置（1-based 行号 / 列号）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub line: usize,
    pub col: usize,
}

impl Span {
    pub const fn new(line: usize, col: usize) -> Self {
        Self { line, col }
    }
}

/// 属性：`name="value"` 或裸属性（如 `required`，语义上为 true）。
#[derive(Debug, Clone, PartialEq)]
pub struct Attribute {
    pub name: String,
    pub value: Option<String>,
}

impl Attribute {
    pub fn new(name: impl Into<String>, value: Option<String>) -> Self {
        Self {
            name: name.into(),
            value,
        }
    }
}

/// 语义标签节点。文本内容统一存于 [`Node::text`]（多个文本片段拼接、空白归一化）。
#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    pub tag: String,
    pub attrs: Vec<Attribute>,
    pub text: Option<String>,
    pub children: Vec<Node>,
    pub span: Span,
}

impl Node {
    pub fn new(tag: impl Into<String>, span: Span) -> Self {
        Self {
            tag: tag.into(),
            attrs: Vec::new(),
            text: None,
            children: Vec::new(),
            span,
        }
    }

    pub fn is(&self, tag: &str) -> bool {
        self.tag == tag
    }

    /// 查询属性值（仅当属性带显式值时返回）。
    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|a| a.name == name)
            .and_then(|a| a.value.as_deref())
    }

    /// 查询属性值并解析为 `i64`。
    pub fn attr_int(&self, name: &str) -> Option<i64> {
        self.attr(name).and_then(|v| v.trim().parse().ok())
    }

    /// 查询属性值并解析为 `f64`。
    pub fn attr_f64(&self, name: &str) -> Option<f64> {
        self.attr(name).and_then(|v| v.trim().parse().ok())
    }

    /// 布尔属性：裸属性视为 `true`；`true/1/yes` 为 true，其余为 false。
    pub fn attr_bool(&self, name: &str) -> Option<bool> {
        self.attrs.iter().find(|a| a.name == name).map(|a| match a.value.as_deref() {
            Some(v) => matches!(v.trim().to_ascii_lowercase().as_str(), "true" | "1" | "yes"),
            None => true,
        })
    }

    /// 布尔属性便捷版，缺省为 `default`。
    pub fn flag(&self, name: &str, default: bool) -> bool {
        self.attr_bool(name).unwrap_or(default)
    }

    /// 颜色属性：`#rgb` / `#rrggbb`。
    pub fn attr_color(&self, name: &str) -> Option<u32> {
        self.attr(name).and_then(crate::theme::parse_color)
    }

    /// 浮点属性便捷版，缺省为 `default`。
    pub fn attr_float(&self, name: &str, default: f32) -> f32 {
        self.attr_f64(name).map(|v| v as f32).unwrap_or(default)
    }

    /// 指定标签的子节点迭代器。
    pub fn children_of<'a>(&'a self, tag: &'a str) -> impl Iterator<Item = &'a Node> + 'a {
        self.children.iter().filter(move |c| c.is(tag))
    }

    /// 元素文本内容，缺省返回 `default`。
    pub fn text_of(&self, default: &str) -> String {
        self.text.clone().unwrap_or_else(|| default.to_owned())
    }
}
