---
layout: home

hero:
  name: Semantic Installer
  text: 类 HTML 语义化安装引擎
  tagline: 基于 Rust + egui 的跨平台安装程序引擎：以「类 HTML 语义标签」声明式定义安装界面，内置固定画布布局引擎、安装 / 卸载流程引擎与一体化卸载工具。完全开源（MIT）。
  image:
    src: /logo.svg
    alt: Semantic Installer
  actions:
    - theme: brand
      text: 快速开始
      link: /quick-start
    - theme: alt
      text: 开发者文档
      link: /architecture

features:
  - title: 类 HTML 语义标签
    details: <window> <page> <layer> <row> <column> <text> <button> <input> <checkbox> <radio> <select> <progress> <image> <link> 等语义化组件，手写词法 + 递归下降解析器，解析错误带行列号。
  - title: 图层（Layer）渲染
    details: 同一固定画布上多层 z 序叠加：背景铺底 + 前景控件。支持 bg 背景色（含 #rrggbbaa 半透明遮罩），天然支持精美背景自定义。
  - title: 多语言 i18n
    details: lang/{locale}.toml 语言资源 + 运行时一键切换；语言选择可前置为流程第一步，安装后写入应用配置供产品联动。
  - title: 简单样式动画
    details: animate 属性声明式动画：fade-in / slide / spin / pulse / scale-in，支持 infinite / delay；纯渲染期时间插值、不侵入布局，进度条平滑过渡。
  - title: 安装流程引擎
    details: 清单（TOML）驱动 → 动作集（复制 / 写文件 / 快捷方式 / 注册表 / 运行进程）→ 后台线程执行 → 进度上报 → 失败 / 取消逆序回滚 → 全程 install.log。
  - title: 一体化卸载工具
    details: 与安装器共享同一 UI 外壳与流程引擎（统一底层架构），仅以模式区分；Windows 下自动登记「添加 / 删除程序」卸载入口。
  - title: 非系统标题栏模式
    details: --no-titlebar 去掉原生标题栏、客户区内自绘标题栏（拖动 / 最小化 / 关闭）；--titlebar-transparent 让标题栏透明透出背景图，文字图标干净绘制。
  - title: 固定画布布局引擎
    details: 窗口尺寸固定不可变（禁缩放 / 最大化），引擎不做自适应布局——解析期即可确定每个控件的矩形，布局确定、可单测，内容溢出有解析期告警。
  - title: 可靠性设计
    details: 应用代码 0 unsafe（仅 Windows COM 互操作两处经审计 unsafe）、UI 路径无 unwrap/expect、panic hook 记录日志后优雅退出、后台线程仅经 channel 通信。
---

<div class="vp-doc" style="padding: 0 1rem 2rem;">

## 解决什么问题

开源生态中长期缺少**美观、现代化**的安装界面解决方案：主流安装程序界面风格古板、自定义程度极低。Semantic Installer 以「类 HTML 语义标签」的声明式方式定义安装界面，配合图层、动画、多语言与透明标题栏能力，让开发者像写网页一样轻松构建**大厂级**的安装体验。

## 技术选型

| 层 | 技术 | 职责 |
| --- | --- | --- |
| 语义化核心 | `semantic-ui`（Rust，零 GUI） | 词法 / 语法 / AST / 布局 / 组件 / 主题 |
| 流程引擎 | `flow-engine`（Rust，零 GUI） | 清单 / 动作 / 执行器 / 回滚 / 平台实现 |
| 界面适配 | `ui-adapter`（Rust + egui 0.35） | 固定窗口壳 / 节点渲染 / 字体 / 主题 / 事件（唯一 GUI 层） |
| 应用入口 | `apps/installer`、`apps/uninstaller` | 安装程序入口 / 一体化卸载工具入口 |

## 快速上手（Windows）

```powershell
cargo build --release

cd examples\demo
..\..\target\release\installer.exe --manifest install.toml --markup app.ui `
  --uninstaller ..\..\target\release\uninstaller.exe
```

无界面快速验证：

```powershell
..\..\target\release\installer.exe --manifest install.toml --dry-run install
..\..\target\release\uninstaller.exe --record <uninstall.json> --dry-run
```

## 实际运行效果

内置 demo 示例的真实运行截图（窗口固定 900×620，自绘简化标题栏）：

| 语言选择 | 欢迎页（图层背景） | 许可协议（必选校验） |
| --- | --- | --- |
| ![语言选择页](/screenshots/demo-locale.png) | ![欢迎页](/screenshots/demo-welcome.png) | ![许可协议页](/screenshots/demo-license.png) |

| 安装选项 | 安装进度 | 安装完成 |
| --- | --- | --- |
| ![安装选项页](/screenshots/demo-options.png) | ![安装进度页](/screenshots/demo-progress.png) | ![完成页](/screenshots/demo-finish.png) |

## 文档

| 文档 | 内容 |
| --- | --- |
| [总体架构](/architecture) | 整体架构、分层原则、固定尺寸取舍、可靠性设计 |
| [标记参考](/markup-reference) | 语义标签全字典、属性说明、完整示例 |
| [二次开发](/secondary-development) | 二次开发指南：复制示例 → 改配置 → 构建 |
| [测试与验证](/testing) | 单元测试 / dry-run / 冒烟验证方法 |

</div>
