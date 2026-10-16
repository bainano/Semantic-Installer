//! `ui-adapter`：egui 适配层——唯一触碰 GUI 的层。
//!
//! - [`app`]：固定尺寸窗口壳与安装 / 卸载流程编排；
//! - [`render`]：语义节点树 → egui 组件；
//! - [`theme`]：语义设计令牌 → egui Visuals；
//! - [`fonts`]：系统 CJK 字体探测与注入；
//! - [`assets`]：PNG 图片加载与纹理缓存；
//! - [`events`]：点击事件 → 流程引擎动作分发。

pub mod app;
pub mod assets;
pub mod events;
pub mod fonts;
pub mod render;
pub mod theme;

pub use app::{window_size_of, ErrorApp, InstallerApp, Mode, ShellOptions};
