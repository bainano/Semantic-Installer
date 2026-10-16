# Semantic Installer Engine（语义化安装引擎）

基于 **Rust + egui** 的跨平台安装程序引擎：以「类 HTML 语义标签」声明式定义安装界面，
内置固定画布布局引擎、安装 / 卸载流程引擎与一体化卸载工具。**完全开源（MIT）**。

> 核心取舍：安装程序窗口尺寸**固定不可变**（在清单 / 标记中声明，运行时不可缩放、不可最大化），
> 引擎不做自适应布局。这是为了简化引擎、保证布局确定性，详见 `docs/architecture.md`。

## 特性

- **类 HTML 语义标记**：`<window>` `<page>` `<layer>` `<row>` `<column>` `<text>` `<button>` `<input>`
  `<checkbox>` `<radio>` `<select>` `<progress>` `<image>` `<link>` 等；手写词法 + 递归下降解析器，
  解析错误带行列号。
- **图层（Layer）**：同一固定画布上多层 z 序叠加（背景铺底 + 前景控件），支持 `bg` 背景色与整幅画布独立布局。
- **多语言（i18n）**：`lang/{locale}.toml` 语言资源 + `--lang` 参数 > 窗口 `locale` > 系统语言 > 默认的优先级链；
  `<select id="locale">` 运行时一键切换；安装后把所选语言写入应用配置 / 注册表，供安装产物联动。
- **简单样式动画**：`animate` 属性声明式动画（fade-in / slide / spin / pulse / scale-in，可 `infinite` / `delay`），
  纯渲染期时间插值、不侵入布局；进度条平滑过渡。
- **固定画布布局引擎**：解析期即可确定每个控件的矩形，布局确定性、可单测；内容溢出有解析期告警。
- **安装流程引擎**：清单（TOML）驱动 → 动作集（复制 / 写文件 / 快捷方式 / 注册表 / 运行进程）→
  后台线程执行 → 进度上报 → **失败 / 取消逆序回滚** → 全程 install.log。
- **一体化卸载器**：与安装器共用同一 UI 壳与流程引擎（统一底层架构），仅以模式区分；
  Windows 下自动登记「添加 / 删除程序」卸载入口。
- **可靠性**：应用代码 0 unsafe（仅 Windows COM 互操作两处经审计 unsafe）、UI 路径无 unwrap/expect、
  panic hook 记日志后优雅退出、后台线程仅经 channel 通信。
- **中文字体**：自动探测系统 CJK 字体（微软雅黑 / PingFang / NotoSansCJK）注入渲染。

## 快速上手（Windows，需 Rust ≥ 1.92）

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

## 工程结构

```
installer-engine/
├── crates/
│   ├── semantic-ui/    语义标记核心：词法 / 语法 / AST / 布局 / 组件 / 主题（零 GUI）
│   ├── flow-engine/    安装 / 卸载流程引擎：清单 / 动作 / 执行器 / 回滚 / 平台实现（零 GUI）
│   └── ui-adapter/     egui 适配层：固定窗口壳 / 节点渲染 / 字体 / 主题 / 事件（唯一 GUI 层）
├── apps/
│   ├── installer/      安装程序入口（薄二进制）
│   └── uninstaller/    一体化卸载工具入口（内置默认界面，支持 --markup 覆盖）
└── examples/demo/      完整示例：app.ui + install.toml + 资源 + 负载
```

## 文档

| 文档 | 内容 |
| --- | --- |
| [docs/architecture.md](docs/architecture.md) | 总体架构、分层原则、固定尺寸取舍、可靠性设计 |
| [docs/markup-reference.md](docs/markup-reference.md) | 语义标签全字典、属性说明、完整示例 |
| [docs/secondary-development.md](docs/secondary-development.md) | 二次开发指南：复制示例 → 改配置 → 构建 |
| [docs/testing.md](docs/testing.md) | 测试与验证方法（单元测试 / dry-run / 冒烟） |

## 许可

[MIT](LICENSE)
