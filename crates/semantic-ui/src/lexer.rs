//! 词法器：把类 HTML 语义标记切成 token 流，支持注释、引号属性值与实体解码。
//!
//! 状态：文本 ⇄ 标签内部（`<name` 之后进入，属性 / `>` / `/>` 之后退出）。

use crate::ast::Span;

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    /// `<name`
    OpenTag(String),
    /// `</name>`
    EndTag(String),
    /// `>`
    CloseBracket,
    /// `/>`
    SelfCloseBracket,
    /// 属性名（裸属性无后继 AttrValue）
    AttrName(String),
    /// 属性值（已解码引号字符串）
    AttrValue(String),
    /// 元素之间的文本片段
    Text(String),
    Eof,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LexError {
    pub message: String,
    pub span: Span,
}

impl std::fmt::Display for LexError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "词法错误（第 {} 行，第 {} 列）：{}",
            self.span.line, self.span.col, self.message
        )
    }
}

impl std::error::Error for LexError {}

/// 文本实体解码：`&lt; &gt; &amp; &quot; &#39; &nbsp;`
fn decode_entities(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut rest = input;
    while let Some(pos) = rest.find('&') {
        out.push_str(&rest[..pos]);
        let tail = &rest[pos..];
        let (repl, consumed) = match &tail[..tail.len().min(8)] {
            t if t.starts_with("&lt;") => ("<", 4),
            t if t.starts_with("&gt;") => (">", 4),
            t if t.starts_with("&amp;") => ("&", 5),
            t if t.starts_with("&quot;") => ("\"", 6),
            t if t.starts_with("&#39;") => ("'", 5),
            t if t.starts_with("&nbsp;") => (" ", 6),
            _ => ("&", 1),
        };
        out.push_str(repl);
        rest = &tail[consumed..];
    }
    out.push_str(rest);
    out
}

struct Cursor {
    chars: Vec<char>,
    pos: usize,
    line: usize,
    col: usize,
}

impl Cursor {
    fn new(input: &str) -> Self {
        Self {
            chars: input.chars().collect(),
            pos: 0,
            line: 1,
            col: 1,
        }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn bump(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.pos += 1;
        if c == '\n' {
            self.line += 1;
            self.col = 1;
        } else {
            self.col += 1;
        }
        Some(c)
    }

    fn span(&self) -> Span {
        Span::new(self.line, self.col)
    }

    fn eof(&self) -> bool {
        self.pos >= self.chars.len()
    }
}

fn skip_ws(cur: &mut Cursor) {
    while let Some(c) = cur.peek() {
        if c.is_whitespace() {
            cur.bump();
        } else {
            break;
        }
    }
}

fn is_name_char(c: char) -> bool {
    c.is_alphanumeric() || c == '-' || c == '_' || c == ':' || c == '.'
}

fn lex_name(cur: &mut Cursor) -> String {
    let mut name = String::new();
    while let Some(c) = cur.peek() {
        if is_name_char(c) {
            name.push(c);
            cur.bump();
        } else {
            break;
        }
    }
    name
}

/// 把整个文档切成 token 流。
pub fn tokenize(input: &str) -> Result<Vec<Token>, LexError> {
    let mut cur = Cursor::new(input);
    let mut tokens = Vec::new();

    while !cur.eof() {
        let c = cur.peek().expect("非空");
        if c == '<' {
            let start = cur.span();
            cur.bump();
            match cur.peek() {
                Some('!') => {
                    // 注释 <!-- ... -->
                    cur.bump();
                    if cur.peek() != Some('-') {
                        return Err(LexError {
                            message: "期望注释以 <!-- 开头".into(),
                            span: start,
                        });
                    }
                    cur.bump();
                    if cur.peek() != Some('-') {
                        return Err(LexError {
                            message: "期望注释以 <!-- 开头".into(),
                            span: start,
                        });
                    }
                    cur.bump();
                    let mut closed = false;
                    while !cur.eof() {
                        if cur.peek() == Some('-')
                            && cur.chars.get(cur.pos + 1) == Some(&'-')
                            && cur.chars.get(cur.pos + 2) == Some(&'>')
                        {
                            cur.bump();
                            cur.bump();
                            cur.bump();
                            closed = true;
                            break;
                        }
                        cur.bump();
                    }
                    if !closed {
                        return Err(LexError {
                            message: "注释未闭合（缺少 -->）".into(),
                            span: start,
                        });
                    }
                }
                Some('/') => {
                    // 结束标签 </name>
                    cur.bump();
                    let name = lex_name(&mut cur);
                    if name.is_empty() {
                        return Err(LexError {
                            message: "结束标签缺少名称".into(),
                            span: start,
                        });
                    }
                    skip_ws(&mut cur);
                    if cur.peek() != Some('>') {
                        return Err(LexError {
                            message: format!("结束标签 </{name}> 缺少 >"),
                            span: start,
                        });
                    }
                    cur.bump();
                    tokens.push(Token {
                        kind: TokenKind::EndTag(name),
                        span: start,
                    });
                }
                _ => {
                    // 开始标签 <name ...>
                    let name = lex_name(&mut cur);
                    if name.is_empty() {
                        return Err(LexError {
                            message: "开始标签缺少名称".into(),
                            span: start,
                        });
                    }
                    tokens.push(Token {
                        kind: TokenKind::OpenTag(name),
                        span: start,
                    });
                    lex_inside_tag(&mut cur, &mut tokens)?;
                }
            }
        } else {
            // 文本，直到下一个 '<'
            let start = cur.span();
            let mut text = String::new();
            while let Some(c) = cur.peek() {
                if c == '<' {
                    break;
                }
                text.push(c);
                cur.bump();
            }
            if !text.is_empty() {
                tokens.push(Token {
                    kind: TokenKind::Text(decode_entities(&text)),
                    span: start,
                });
            }
        }
    }

    tokens.push(Token {
        kind: TokenKind::Eof,
        span: cur.span(),
    });
    Ok(tokens)
}

/// 标签内部词法：属性名 / `=值` / `>` / `/>`。
fn lex_inside_tag(cur: &mut Cursor, tokens: &mut Vec<Token>) -> Result<(), LexError> {
    loop {
        skip_ws(cur);
        match cur.peek() {
            None => {
                return Err(LexError {
                    message: "标签未闭合（缺少 >）".into(),
                    span: cur.span(),
                })
            }
            Some('>') => {
                cur.bump();
                tokens.push(Token {
                    kind: TokenKind::CloseBracket,
                    span: cur.span(),
                });
                return Ok(());
            }
            Some('/') => {
                cur.bump();
                if cur.peek() == Some('>') {
                    cur.bump();
                    tokens.push(Token {
                        kind: TokenKind::SelfCloseBracket,
                        span: cur.span(),
                    });
                    return Ok(());
                }
                return Err(LexError {
                    message: "期望自闭合标记 />".into(),
                    span: cur.span(),
                });
            }
            Some(c) if is_name_char(c) => {
                let name_span = cur.span();
                let name = lex_name(cur);
                skip_ws(cur);
                if cur.peek() == Some('=') {
                    cur.bump();
                    skip_ws(cur);
                    match cur.peek() {
                        Some(q @ ('"' | '\'')) => {
                            cur.bump();
                            let mut val = String::new();
                            let mut closed = false;
                            while let Some(c) = cur.peek() {
                                if c == q {
                                    cur.bump();
                                    closed = true;
                                    break;
                                }
                                val.push(c);
                                cur.bump();
                            }
                            if !closed {
                                return Err(LexError {
                                    message: format!("属性 {name} 的值引号未闭合"),
                                    span: name_span,
                                });
                            }
                            tokens.push(Token {
                                kind: TokenKind::AttrName(name),
                                span: name_span,
                            });
                            tokens.push(Token {
                                kind: TokenKind::AttrValue(decode_entities(&val)),
                                span: cur.span(),
                            });
                        }
                        _ => {
                            return Err(LexError {
                                message: format!("属性 {name} 的值必须用引号括起"),
                                span: cur.span(),
                            })
                        }
                    }
                } else {
                    tokens.push(Token {
                        kind: TokenKind::AttrName(name),
                        span: name_span,
                    });
                }
            }
            Some(_) => {
                return Err(LexError {
                    message: "标签内出现非法字符".into(),
                    span: cur.span(),
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tag_attributes_are_tokenized() {
        let tokens = tokenize(r#"<window width="900" height="620">"#).unwrap();
        let kinds: Vec<TokenKind> = tokens.iter().map(|t| t.kind.clone()).collect();
        assert_eq!(
            kinds,
            vec![
                TokenKind::OpenTag("window".into()),
                TokenKind::AttrName("width".into()),
                TokenKind::AttrValue("900".into()),
                TokenKind::AttrName("height".into()),
                TokenKind::AttrValue("620".into()),
                TokenKind::CloseBracket,
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn bare_attr_has_no_value() {
        let tokens = tokenize("<button primary/>").unwrap();
        let kinds: Vec<TokenKind> = tokens.iter().map(|t| t.kind.clone()).collect();
        assert_eq!(
            kinds,
            vec![
                TokenKind::OpenTag("button".into()),
                TokenKind::AttrName("primary".into()),
                TokenKind::SelfCloseBracket,
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn single_quoted_values_work() {
        let tokens = tokenize("<text color='#fff'>x</text>").unwrap();
        let kinds: Vec<TokenKind> = tokens.iter().map(|t| t.kind.clone()).collect();
        assert!(kinds.contains(&TokenKind::AttrValue("#fff".into())));
    }

    #[test]
    fn unterminated_tag_errors() {
        let err = tokenize("<window width=\"1\"").unwrap_err();
        assert!(err.message.contains("未闭合"));
    }

    #[test]
    fn text_around_tags() {
        let tokens = tokenize("<text>hello</text>").unwrap();
        let kinds: Vec<TokenKind> = tokens.iter().map(|t| t.kind.clone()).collect();
        assert_eq!(
            kinds,
            vec![
                TokenKind::OpenTag("text".into()),
                TokenKind::CloseBracket,
                TokenKind::Text("hello".into()),
                TokenKind::EndTag("text".into()),
                TokenKind::Eof,
            ]
        );
    }
}
