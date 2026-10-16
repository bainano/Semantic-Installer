//! 解析器：递归下降，把 token 流解析为 [`crate::ast::Node`] 树并做语义校验。
//!
//! - 未知标签 → 硬错误（带行列号）；
//! - 未知属性 / 缺省属性 → 警告（保持向前兼容）。

use crate::ast::{Node, Span};
use crate::lexer::{tokenize, Token, TokenKind};

pub const ROOT_TAG: &str = "window";

/// 引擎内置的语义标签字典。
pub const KNOWN_TAGS: &[&str] = &[
    "window", "page", "layer", "row", "column", "text", "button", "input", "checkbox", "radio",
    "select", "progress", "image", "link", "divider", "spacer", "label",
];

#[derive(Debug, Clone, PartialEq)]
pub struct ParseError {
    pub message: String,
    pub span: Span,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "语法错误（第 {} 行，第 {} 列）：{}",
            self.span.line, self.span.col, self.message
        )
    }
}

impl std::error::Error for ParseError {}

#[derive(Debug, Clone, PartialEq)]
pub struct ParseResult {
    pub root: Node,
    pub warnings: Vec<String>,
}

/// 解析完整语义标签文档（期望唯一的 `<window>` 根节点）。
pub fn parse(input: &str) -> Result<ParseResult, ParseError> {
    let tokens = tokenize(input).map_err(|e| ParseError {
        message: e.message.clone(),
        span: e.span,
    })?;
    let mut p = Parser {
        tokens,
        pos: 0,
        warnings: Vec::new(),
    };
    let root = p.parse_document()?;
    validate(&root, &mut p.warnings);
    Ok(ParseResult {
        root,
        warnings: p.warnings,
    })
}

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
    warnings: Vec<String>,
}

impl Parser {
    fn peek(&self) -> &Token {
        &self.tokens[self.pos]
    }

    fn advance(&mut self) -> Token {
        let t = self.tokens[self.pos].clone();
        if self.pos < self.tokens.len() - 1 {
            self.pos += 1;
        }
        t
    }

    fn error<T>(&self, message: impl Into<String>, span: Span) -> Result<T, ParseError> {
        Err(ParseError {
            message: message.into(),
            span,
        })
    }

    fn parse_document(&mut self) -> Result<Node, ParseError> {
        // 跳过文档前导文本/空白
        while let TokenKind::Text(_) = self.peek().kind {
            self.advance();
        }
        let root = self.parse_element()?;
        if !root.is(ROOT_TAG) {
            return self.error(
                format!("根节点必须是 <{}>，实际为 <{}>", ROOT_TAG, root.tag),
                root.span,
            );
        }
        // 根后只允许空白
        while let TokenKind::Text(t) = &self.peek().kind {
            if !t.trim().is_empty() {
                return self.error("根节点之后存在多余内容", self.peek().span);
            }
            self.advance();
        }
        if !matches!(self.peek().kind, TokenKind::Eof) {
            return self.error("根节点之后存在多余内容", self.peek().span);
        }
        Ok(root)
    }

    fn parse_element(&mut self) -> Result<Node, ParseError> {
        let open = match self.advance().kind {
            TokenKind::OpenTag(name) => {
                let span = self.tokens[self.pos - 1].span;
                (name, span)
            }
            other => return self.error(format!("期望开始标签，遇到 {other:?}"), self.peek().span),
        };
        let (name, start_span) = open;
        if !KNOWN_TAGS.contains(&name.as_str()) {
            return self.error(format!("未知标签 <{name}>"), start_span);
        }
        let mut node = Node::new(name, start_span);

        // 属性
        loop {
            match &self.peek().kind {
                TokenKind::CloseBracket => {
                    self.advance();
                    break;
                }
                TokenKind::SelfCloseBracket => {
                    self.advance();
                    return Ok(node);
                }
                TokenKind::AttrName(attr) => {
                    let attr = attr.clone();
                    let attr_span = self.peek().span;
                    self.advance();
                    let value = match &self.peek().kind {
                        TokenKind::AttrValue(v) => {
                            let v = v.clone();
                            self.advance();
                            Some(v)
                        }
                        _ => None,
                    };
                    if node.attrs.iter().any(|a| a.name == attr) {
                        self.warnings.push(format!(
                            "标签 <{}>（第 {} 行）重复属性 \"{}\"，已忽略后出现者",
                            node.tag, start_span.line, attr
                        ));
                    } else {
                        node.attrs.push(crate::ast::Attribute::new(attr, value));
                    }
                    let _ = attr_span;
                }
                other => {
                    return self.error(
                        format!("标签 <{}> 内出现非法内容 {other:?}", node.tag),
                        self.peek().span,
                    )
                }
            }
        }

        // 子内容
        let mut text_buf = String::new();
        loop {
            match &self.peek().kind {
                TokenKind::Text(t) => {
                    text_buf.push_str(t);
                    self.advance();
                }
                TokenKind::OpenTag(_) => {
                    let child = self.parse_element()?;
                    node.children.push(child);
                }
                TokenKind::EndTag(tag) => {
                    let tag = tag.clone();
                    let end_span = self.peek().span;
                    self.advance();
                    if tag != node.tag {
                        return self.error(
                            format!("结束标签 </{tag}> 与开始标签 <{}> 不匹配", node.tag),
                            end_span,
                        );
                    }
                    break;
                }
                TokenKind::Eof => {
                    return self.error(format!("标签 <{}> 未闭合", node.tag), start_span);
                }
                other => {
                    return self.error(
                        format!("标签 <{}> 内出现非法内容 {other:?}", node.tag),
                        self.peek().span,
                    )
                }
            }
        }

        let trimmed = text_buf.trim();
        if !trimmed.is_empty() {
            // 保留换行（支持多行文本），行内空白折叠
            node.text = Some(normalize_text(trimmed));
        }
        Ok(node)
    }
}

/// 按行折叠空白并保留换行：多行文本的宽度由布局引擎按"最长行"计算。
fn normalize_text(s: &str) -> String {
    s.lines()
        .map(collapse_ws)
        .collect::<Vec<String>>()
        .join("\n")
}

fn collapse_ws(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_ws = false;
    for c in s.chars() {
        if c.is_whitespace() {
            if !prev_ws {
                out.push(' ');
            }
            prev_ws = true;
        } else {
            out.push(c);
            prev_ws = false;
        }
    }
    out.trim().to_owned()
}

/// 结构校验：page 必须带 id；window 尺寸缺省时给出警告。
fn validate(root: &Node, warnings: &mut Vec<String>) {
    let mut pages = 0usize;
    for node in walk(root) {
        match node.tag.as_str() {
            "window" => {
                if node.attr("width").is_none() {
                    warnings.push("window 缺少 width 属性，将使用默认值 800".into());
                }
                if node.attr("height").is_none() {
                    warnings.push("window 缺少 height 属性，将使用默认值 600".into());
                }
            }
            "page" => {
                pages += 1;
                if node.attr("id").is_none() {
                    warnings.push(format!(
                        "page（第 {} 行）缺少 id 属性，该页面将无法被流程导航引用",
                        node.span.line
                    ));
                }
            }
            "image"
                if node.attr("src").is_none() => {
                    warnings.push(format!(
                        "image（第 {} 行）缺少 src 属性，将渲染为空白",
                        node.span.line
                    ));
                }
            _ => {}
        }
    }
    if pages == 0 {
        warnings.push("文档中没有任何 <page>，流程将无可显示页面".into());
    }
}

/// 深度优先遍历节点树（含自身）。
pub fn walk(node: &Node) -> Vec<&Node> {
    let mut out = Vec::new();
    fn go<'a>(n: &'a Node, out: &mut Vec<&'a Node>) {
        out.push(n);
        for c in &n.children {
            go(c, out);
        }
    }
    go(node, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_simple_document() {
        let src = r#"
<window title="Demo" width="900" height="620">
  <page id="welcome">
    <row>
      <text size="26">Hello</text>
      <button action="next" primary="true">下一步</button>
    </row>
  </page>
</window>
"#;
        let res = parse(src).expect("解析成功");
        assert_eq!(res.root.tag, "window");
        assert_eq!(res.root.attr("width"), Some("900"));
        assert_eq!(res.root.children.len(), 1);
        let page = &res.root.children[0];
        assert_eq!(page.attr("id"), Some("welcome"));
        let row = &page.children[0];
        assert_eq!(row.tag, "row");
        assert_eq!(row.children.len(), 2);
        assert_eq!(row.children[0].text.as_deref(), Some("Hello"));
        assert_eq!(row.children[1].attr("action"), Some("next"));
        assert!(row.children[1].attr_bool("primary") == Some(true));
    }

    #[test]
    fn self_closing_and_bare_attrs() {
        let src = r#"<window width="100" height="100"><page id="p"><column><spacer/><input id="dir" required="true"/><checkbox id="ok" checked/></column></page></window>"#;
        let res = parse(src).expect("解析成功");
        let column = &res.root.children[0].children[0];
        assert_eq!(column.tag, "column");
        assert_eq!(column.children.len(), 3);
        assert!(column.children[0].is("spacer"));
        assert_eq!(column.children[1].attr_bool("required"), Some(true));
        assert_eq!(column.children[2].attr_bool("checked"), Some(true));
    }

    #[test]
    fn comments_are_ignored() {
        let src = "<window width=\"1\" height=\"1\"><!-- 注释 --><page id=\"a\"><text>ok</text></page></window>";
        let res = parse(src).expect("解析成功");
        assert_eq!(res.root.children.len(), 1);
    }

    #[test]
    fn entities_are_decoded() {
        let src = r#"<window width="1" height="1"><page id="a"><text>a &lt; b &amp; c &quot;d&quot;</text></page></window>"#;
        let res = parse(src).expect("解析成功");
        let text = &res.root.children[0].children[0];
        assert_eq!(text.text.as_deref(), Some("a < b & c \"d\""));
    }

    #[test]
    fn mismatched_close_is_error() {
        let src = "<window width=\"1\" height=\"1\"><page id=\"a\"></row></page></window>";
        let err = parse(src).unwrap_err();
        assert!(err.message.contains("不匹配"));
    }

    #[test]
    fn unknown_tag_is_error() {
        let src = "<window width=\"1\" height=\"1\"><page id=\"a\"><unknown/></page></window>";
        let err = parse(src).unwrap_err();
        assert!(err.message.contains("未知标签"));
    }

    #[test]
    fn root_must_be_window() {
        let err = parse("<page id=\"a\"></page>").unwrap_err();
        assert!(err.message.contains("根节点"));
    }

    #[test]
    fn error_has_line_col() {
        let src = "<window width=\"1\" height=\"1\">\n<page id=\"a\"><unknown/></page></window>";
        let err = parse(src).unwrap_err();
        assert_eq!(err.span.line, 2);
        assert!(err.span.col >= 1);
    }

    #[test]
    fn duplicate_attr_warns() {
        let src = r#"<window width="1" height="1"><page id="a" id="b"><text>x</text></page></window>"#;
        let res = parse(src).expect("解析成功");
        assert!(res.warnings.iter().any(|w| w.contains("重复属性")));
        assert_eq!(res.root.children[0].attr("id"), Some("a"));
    }
}
