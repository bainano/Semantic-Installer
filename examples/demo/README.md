# Demo 示例工程

本目录是语义化安装引擎（Semantic Installer Engine）的完整演示：
- `app.ui` —— 安装器界面（一份类 HTML 语义标记文件，定义全部页面）
- `uninstall.ui` —— 卸载器界面（也可省略，卸载器内置默认界面）
- `install.toml` —— 安装 / 卸载清单（安装动作、快捷方式、注册表、卸载策略）
- `resources/` —— 界面图片（banner.png）与安装负载（files/）

## 运行（Windows，需 Rust ≥ 1.92）

```powershell
# 1. 构建（工程根目录）
cd ..\..
cargo build --release

# 2. 运行安装器（把卸载器一并复制进安装目录，实现一体化卸载）
cd examples\demo
..\..\target\release\installer.exe --manifest install.toml --markup app.ui `
  --uninstaller ..\..\target\release\uninstaller.exe

# 3. 卸载（安装完成后，也可通过“控制面板 → 程序和功能”入口）
..\..\target\release\uninstaller.exe --markup uninstall.ui
```

## 无界面快速验证（CI / 自动化）

```powershell
..\..\target\release\installer.exe --manifest install.toml --dry-run install
..\..\target\release\installer.exe --manifest install.toml --dry-run uninstall
```

## 自定义你的安装程序

1. 复制本目录为 `myapp/`；
2. 修改 `app.ui` 中的页面、文案与控件；
3. 修改 `install.toml` 中的应用信息、安装动作与卸载策略；
4. 替换 `resources/files/` 为你的安装负载；
5. 重新构建并运行。

详见 `docs/secondary-development.md`。
