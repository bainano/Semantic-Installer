# 示例工程

仓库内置两个可直接运行的示例工程，覆盖两种典型使用路径。

## demo —— 完整示例

路径：`examples/demo`

完整演示语义化安装程序的全部能力：

- **多语言**：`lang/zh-CN.toml`、`lang/en-US.toml` 语言资源
- **语言页前置**：`<select id="locale">` 作为流程第一步，运行时切换语言
- **图层**：背景图层 + 前景控件图层
- **动画**：标题 slide-up、说明 fade-in、进度平滑过渡
- **安装选项页**：安装类型选择、安装位置输入

```powershell
cd examples\demo
..\..\target\release\installer.exe --manifest install.toml --markup app.ui `
  --uninstaller ..\..\target\release\uninstaller.exe
```

### 六步流程截图

| ① 语言选择 | ② 欢迎页 |
| --- | --- |
| ![语言选择页](/screenshots/demo-locale.png) | ![欢迎页](/screenshots/demo-welcome.png) |

| ③ 许可协议 | ④ 安装选项 |
| --- | --- |
| ![许可协议页](/screenshots/demo-license.png) | ![安装选项页](/screenshots/demo-options.png) |

| ⑤ 安装进度 | ⑥ 安装完成 |
| --- | --- |
| ![安装进度页](/screenshots/demo-progress.png) | ![完成页](/screenshots/demo-finish.png) |

> 许可协议页的「我同意，继续」按钮在未勾选必选复选框时自动置灰（`required="true"`）；安装进度页在流程引擎后台线程执行动作期间平滑推进。

## plain —— 极简自定义示例

路径：`examples/plain`

无语言页（语言选择前置非强制），演示高度自定义路径：

- **JPG 背景图**：`resources/bg-santorini.jpg`（2100×1400）
- **半透明图层**：`<layer id="shade" bg="#ffffffb0"/>` 压浅背景提升文字对比度
- **透明标题栏**：`--no-titlebar --titlebar-transparent` 透出背景图
- **干净绘制**：标题栏文字 / 图标无阴影

```powershell
cd examples\plain
..\..\target\release\installer.exe --manifest install.toml --markup app.ui `
  --no-titlebar --titlebar-transparent
```

## 如何基于示例二次开发

1. 复制 `examples/plain`（或 `examples/demo`）到你的工程目录；
2. 修改 `app.ui`（界面标记）与 `install.toml`（流程清单）；
3. 替换 `resources/` 下的背景图等资源；
4. 构建并运行。

完整流程见 [二次开发](/secondary-development)。
