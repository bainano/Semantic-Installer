//! PNG 图片加载与纹理缓存（banner / 侧图等装饰资源）。

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use eframe::egui;

pub struct AssetCache {
    textures: HashMap<String, egui::TextureHandle>,
}

impl AssetCache {
    pub fn new() -> Self {
        Self {
            textures: HashMap::new(),
        }
    }

    /// 按 src 路径加载并缓存纹理；失败时记日志并返回 None。
    pub fn get(&mut self, ctx: &egui::Context, src: &str, root: &Path) -> Option<egui::TextureHandle> {
        if let Some(t) = self.textures.get(src) {
            return Some(t.clone());
        }
        let path = resolve_src(src, root);
        match load_texture(ctx, &path, src) {
            Ok(t) => {
                self.textures.insert(src.to_owned(), t.clone());
                Some(t)
            }
            Err(e) => {
                log::warn!("加载图片 {} 失败：{e}", path.display());
                None
            }
        }
    }
}

impl Default for AssetCache {
    fn default() -> Self {
        Self::new()
    }
}

fn resolve_src(src: &str, root: &Path) -> PathBuf {
    let pb = PathBuf::from(src);
    if pb.is_absolute() {
        pb
    } else {
        root.join(pb)
    }
}

fn load_texture(ctx: &egui::Context, path: &Path, name: &str) -> Result<egui::TextureHandle, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("读取失败：{e}"))?;
    let img = image::load_from_memory(&bytes)
        .map_err(|e| format!("解码失败：{e}"))?
        .to_rgba8();
    let (w, h) = img.dimensions();
    let color = egui::ColorImage::from_rgba_unmultiplied(
        [w as usize, h as usize],
        img.as_raw(),
    );
    Ok(ctx.load_texture(
        name.to_owned(),
        color,
        egui::TextureOptions::LINEAR,
    ))
}
