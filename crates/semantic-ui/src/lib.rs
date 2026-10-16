//! `semantic-ui`：类 HTML 语义标签化 UI 渲染核心（零 GUI 依赖）。
//!
//! 分层：
//! - [`lexer`]：把标记语言文本切成 token 流；
//! - [`parser`]：递归下降解析为 [`ast::Node`] 树；
//! - [`layout`]：在固定画布上为节点树计算确定性矩形布局；
//! - [`widget`]：语义组件模型与运行时 UI 状态；
//! - [`theme`]：颜色 / 字号等设计令牌。

pub mod ast;
pub mod layout;
pub mod lexer;
pub mod parser;
pub mod theme;
pub mod widget;

pub use ast::{Attribute, Node, Span};
pub use layout::{Align, FontWeight, Measurer, PageModel, Rect, Size};
pub use lexer::{tokenize, LexError, Token, TokenKind};
pub use parser::{parse, ParseError, ParseResult};
pub use theme::{parse_color, Theme};
pub use widget::{Click, UiState};
