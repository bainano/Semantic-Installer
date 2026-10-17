# 快速开始

本文带你从克隆仓库到跑起一个自定义安装界面，全程约 5 分钟。

## 环境要求

- **Windows 10/11**（当前引擎仅面向 Windows 平台）
- **Rust ≥ 1.92**（本仓库在 rustc 1.97 上验证）
- **Cargo** 可访问 crates 镜像（国内可使用 rsproxy）

## 1. 克隆并构建

```powershell
git clone https://github.com/bainano/Semantic-Installer.git
cd Semantic-Installer

# 全工作区构建（含安装器 / 卸载器 / 示例）
cargo build --release
```

构建产物位于 `target\release\installer.exe` 与 `target\release\uninstaller.exe`。

## 2. 运行内置示例

内置两个示例工程：

| 示例 | 路径 | 特点 |
| --- | --- | --- |
| demo | `examples/demo` | 完整示例：多语言、语言页前置、图层、动画、安装选项页 |
| plain | `examples/plain` | 极简自定义：无语言页、JPG 背景图 + 半透明图层 + 透明标题栏 |

以 demo 为例：

```powershell
cd examples\demo
..\..\target\release\installer.exe --manifest install.toml --markup app.ui `
  --uninstaller ..\..\target\release\uninstaller.exe
```

### 实际运行效果

demo 示例的真实运行截图（窗口固定 900×620，无系统标题栏，自绘简化标题栏）：

| 语言选择（第一步） | 欢迎页（图层背景） |
| --- | --- |
| ![语言选择页](/screenshots/demo-locale.png) | ![欢迎页](/screenshots/demo-welcome.png) |

| 许可协议（必选校验） | 安装选项 |
| --- | --- |
| ![许可协议页](/screenshots/demo-license.png) | ![安装选项页](/screenshots/demo-options.png) |

| 安装进度 | 安装完成 |
| --- | --- |
| ![安装进度页](/screenshots/demo-progress.png) | ![完成页](/screenshots/demo-finish.png) |

全部界面由 `examples/demo/app.ui` 一份语义标记文件定义，见 [示例工程](/examples)。

## 3. 无界面快速验证

```powershell
# 安装流程 dry-run（不真正执行动作）
..\..\target\release\installer.exe --manifest install.toml --dry-run install

# 卸载工具 dry-run（记录卸载清单）
..\..\target\release\uninstaller.exe --record <uninstall.json> --dry-run
```

## 4. 跑测试与静态检查

```powershell
cargo test --workspace          # 56 项单元测试
cargo clippy --all-targets -- -D warnings   # 零警告
```

## 5. 自定义你的第一个界面

复制 `examples/plain` 目录，编辑 `app.ui` 与 `install.toml`：

```xml
<window width="900" height="620">
  <page id="welcome" title="欢迎">
    <layer id="bg">
      <image src="resources/bg.jpg" width="900" height="620"/>
    </layer>
    <layer id="fg">
      <column gap="12" padding="48">
        <text size="34" weight="bold">你好，世界</text>
        <button action="next" primary="true">开始安装</button>
      </column>
    </layer>
  </page>
</window>
```

```toml
[window]
width = 900
height = 620
title = "我的安装程序"

steps = ["welcome"]
```

然后：

```powershell
cargo build --release
target\release\installer.exe --manifest install.toml --markup app.ui
```

详细的语义标签说明见 [标记参考](/markup-reference)，二次开发工作流见 [二次开发](/secondary-development)。

## 命令行参数速查

| 参数 | 说明 |
| --- | --- |
| `--manifest <toml>` | 安装清单（步骤、动作集） |
| `--markup <ui>` | 界面标记（语义标签） |
| `--theme <name>` | 主题：light / dark |
| `--lang <locale>` | 语言覆盖（zh-CN / en-US） |
| `--no-titlebar` | 非系统标题栏模式（自绘标题栏） |
| `--titlebar-transparent` | 透明标题栏（隐式启用 `--no-titlebar`） |
| `--uninstaller <exe>` | 卸载工具路径（注册卸载入口用） |
| `--record <json>` | 卸载器记录卸载清单 |
| `--dry-run` | 无界面快速验证（不真正执行动作） |
| `--log <file>` | 日志输出文件 |
