//! 多语言（i18n）资源：`lang/{locale}.toml` 加载与标记文本解析。
//!
//! - 资源文件与清单同为 TOML；支持扁平键（`"welcome.title" = "…"`）与嵌套节（`[welcome] title = "…"`），
//!   统一归一为 `welcome.title` 形式的键；
//! - 标记节点带 `i18n="key"` 且资源命中时，布局 / 渲染使用资源文本；
//!   未命中时保持标记默认文本（优雅回退，无需为每个语言补全资源）。

use std::collections::HashMap;
use std::path::Path;

use crate::ast::Node;

/// 语言资源：键 → 文本。
#[derive(Debug, Clone, Default)]
pub struct I18n {
    pub locale: String,
    entries: HashMap<String, String>,
}

impl I18n {
    /// 加载 `root/lang/{locale}.toml`；文件缺失 / 解析失败时得到空资源（回退默认文本）。
    pub fn load(root: &Path, locale: &str) -> Self {
        let mut i18n = Self {
            locale: locale.to_owned(),
            entries: HashMap::new(),
        };
        let path = root.join("lang").join(format!("{locale}.toml"));
        let src = match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(e) => {
                log::warn!(
                    "语言资源 {} 读取失败（{e}），使用标记默认文本",
                    path.display()
                );
                return i18n;
            }
        };
        match src.parse::<toml::Value>() {
            Ok(v) => collect_keys(&v, "", &mut i18n.entries),
            Err(e) => log::warn!(
                "语言资源 {} 解析失败（{e}），使用标记默认文本",
                path.display()
            ),
        }
        i18n
    }

    /// 按键取文本。
    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries.get(key).map(String::as_str)
    }

    /// 已加载的键数量（测试 / 诊断用）。
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// 资源是否为空（无任何键）。
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

fn collect_keys(v: &toml::Value, prefix: &str, out: &mut HashMap<String, String>) {
    match v {
        toml::Value::Table(t) => {
            for (k, val) in t {
                let key = if prefix.is_empty() {
                    k.clone()
                } else {
                    format!("{prefix}.{k}")
                };
                collect_keys(val, &key, out);
            }
        }
        toml::Value::String(s) => {
            out.insert(prefix.to_owned(), s.clone());
        }
        _ => {}
    }
}

/// 用 i18n 资源解析带文本的节点（text / label / button / link / checkbox / radio）：
/// 节点带 `i18n="key"` 且资源命中时替换文本；未命中保持标记默认文本（回退）。
pub fn resolve(node: &mut Node, i18n: &I18n) {
    let has_text = matches!(
        node.tag.as_str(),
        "text" | "label" | "button" | "link" | "checkbox" | "radio"
    );
    if has_text
        && let Some(key) = node.attr("i18n")
        && let Some(text) = i18n.get(key)
    {
        node.text = Some(text.to_owned());
    }
    for child in &mut node.children {
        resolve(child, i18n);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write_lang(root: &Path, locale: &str, src: &str) {
        let dir = root.join("lang");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(format!("{locale}.toml")), src).unwrap();
    }

    #[test]
    fn load_flat_and_nested_keys() {
        let root = std::env::temp_dir().join(format!("i18n-test-{}", std::process::id()));
        write_lang(
            &root,
            "zh-CN",
            r#"
"welcome.title" = "欢迎"
[button]
next = "下一步"
"#,
        );
        let i18n = I18n::load(&root, "zh-CN");
        assert_eq!(i18n.locale, "zh-CN");
        assert_eq!(i18n.get("welcome.title"), Some("欢迎"));
        assert_eq!(i18n.get("button.next"), Some("下一步"));
        assert_eq!(i18n.len(), 2);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn missing_file_falls_back_to_defaults() {
        let root = std::env::temp_dir().join(format!("i18n-none-{}", std::process::id()));
        let i18n = I18n::load(&root, "en-US");
        assert_eq!(i18n.len(), 0);
        assert!(i18n.get("x").is_none());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn resolve_replaces_text_and_keeps_fallback() {
        let root = std::env::temp_dir().join(format!("i18n-res-{}", std::process::id()));
        write_lang(&root, "en-US", r#"welcome = "Welcome""#);
        let i18n = I18n::load(&root, "en-US");

        let mut node = crate::parser::parse(
            r#"<window width="1" height="1"><page id="p"><text i18n="welcome">欢迎</text><text i18n="missing">默认</text></page></window>"#,
        )
        .unwrap()
        .root;
        resolve(&mut node, &i18n);

        let texts: Vec<String> = node
            .children
            .iter()
            .flat_map(|p| p.children.iter())
            .map(|t| t.text.clone().unwrap_or_default())
            .collect();
        assert_eq!(texts, vec!["Welcome", "默认"], "命中替换、未命中回退");
        let _ = fs::remove_dir_all(&root);
    }
}
