# 二次开发指南（secondary-development.md）

本文面向想用本引擎快速产出**自定义安装程序**的开发者：不需要改引擎代码，
只需要一份标记文件 + 一份清单 + 负载目录。

## 1. 最快路径：复制 demo

```powershell
# 复制示例工程
Copy-Item -Recurse examples\demo myapp
cd myapp

# 删掉示例 banner 与负载，换成自己的
Remove-Item resources\banner.png, resources\files\*
```

## 2. 修改界面（app.ui）

按 [markup-reference.md](markup-reference.md) 修改页面、文案与控件：

- `<window width height theme>` 声明**固定画布尺寸**（运行时不可变）；
- `<page id>` 与清单 `[steps].pages` 一一对应；
- 前进按钮的自动禁用由 `required` 属性驱动；
- 复杂视觉可直接使用**图层**：`<page>` 下并列多个 `<layer>`（z 序叠加，见 markup-reference §3）；
- 界面文字可用 **i18n**：`i18n="键"` 属性 + `lang/zh-CN.toml` 资源，运行时可切换（见 markup-reference §5）；
- 需要动效时给任意节点加 `animate="fade-in 400ms"` 等样式动画（见 markup-reference §6）。

## 3. 修改安装清单（install.toml）

```toml
[app]
name = "我的应用"              # 产品名（窗口标题、快捷方式、注册表显示名）
id = "com.example.myapp"       # 卸载登记与记录定位使用
version = "1.2.3"
publisher = "我的公司"
homepage = "https://example.com"

[window]
width = 900
height = 620
resizable = false              # 固定尺寸约束
theme = "light"

[steps]
pages = ["welcome", "license", "options", "progress", "finish"]

[install]
install_dir = "$LOCALAPPDATA/我的应用"   # 可用 $LOCALAPPDATA $PROGRAMFILES $HOME $TEMP
copy_uninstaller = true

[[install.items]]
type = "copy-dir"
source = "resources/files"     # 相对清单所在目录
dest = "$INSTALL_DIR"

[[install.items]]
type = "write-file"
path = "$INSTALL_DIR/version.txt"
content = "1.2.3\n"

[[install.shortcuts]]
name = "我的应用"
target = "$INSTALL_DIR/MyApp.exe"
dir = "desktop"                # desktop | start-menu

[install.registry]
uninstall_key = true           # 登记“添加/删除程序”
[[install.registry.values]]
path = "HKCU\\Software\\我的应用"
name = "InstallDir"
value = "$INSTALL_DIR"

[install.run_after]
cmd = "$INSTALL_DIR/MyApp.exe"   # 可选：安装完成后启动
wait = false

[uninstall]
keep_user_data = ["$LOCALAPPDATA/我的应用数据"]   # 位于安装目录之外
remove_shortcuts = true
remove_registry = true
```

### 动作类型速查

| type | 字段 | 说明 |
| --- | --- | --- |
| `copy-dir` | source / dest | 递归复制目录 |
| `copy-file` | source / dest | 复制单文件 |
| `create-dir` | path | 创建目录 |
| `write-file` | path / content | 写入文本文件 |
| （内置） | — | 快捷方式 / 注册表 / 卸载器复制 / 运行进程见上 |

## 4. 准备负载与资源

```
myapp/
├── app.ui                 # 界面（唯一）
├── install.toml           # 清单
├── lang/                  # 多语言资源（可选）：zh-CN.toml / en-US.toml …
│   └── zh-CN.toml         #   i18n 键 → 文本（未命中回退标记默认文本）
├── resources/
│   ├── banner.png         # 欢迎页侧图（可选）
│   └── files/             # 安装负载（被复制到 $INSTALL_DIR）
```

## 5. 构建与运行

```powershell
cd <工程根>
cargo build --release

cd myapp
..\..\target\release\installer.exe --manifest install.toml --markup app.ui `
  --uninstaller ..\..\target\release\uninstaller.exe
```

参数一览（installer）：

| 参数 | 说明 |
| --- | --- |
| `--manifest <path>` | 清单路径（默认 `install.toml`） |
| `--markup <path>` | 界面标记（默认 `app.ui`） |
| `--theme light\|dark` | 覆盖主题 |
| `--uninstaller <path>` | 待复制进安装目录的卸载器（配合 `copy_uninstaller=true`） |
| `--lang <locale>` | 覆盖界面语言（如 `zh-CN` / `en-US`；缺省按 参数 > 窗口 `locale` > 系统语言 > `zh-CN`） |
| `--no-titlebar` | 非系统标题栏模式：去掉原生标题栏，客户区自绘标题栏条（拖动 / 最小化 / 关闭） |
| `--titlebar-transparent` | 透明标题栏：隐式启用 `--no-titlebar`，标题栏条背景透明（透出背景图），文字 / 图标带阴影 |
| `--record <uninstall.json>` | 以卸载模式运行（等价于 `--mode uninstall`） |
| `--dry-run install\|uninstall` | 无 GUI 全流程模拟（CI 用） |
| `--log <path>` | 日志文件 |

卸载器参数：`--record` `--app-id` `--markup` `--theme` `--lang` `--no-titlebar` `--titlebar-transparent` `--dry-run`。
卸载器**内置默认界面**——不带 `--markup` 也能独立运行。

## 6. 常见定制点（改引擎）

| 需求 | 位置 |
| --- | --- |
| 新增语义标签 | `crates/semantic-ui/src/parser.rs`（标签字典）+ `layout.rs` + `ui-adapter/src/render.rs` |
| 新增安装动作 | `crates/flow-engine/src/spec.rs`（ItemSpec）+ `actions.rs`（构建）+ `executor.rs`（执行） |
| 新增按钮 action | `crates/ui-adapter/src/events.rs` 分发处 |
| 新增平台能力 | `crates/flow-engine/src/platform/{windows,posix}.rs` |
| C/C++ 核心模块 | 预留 `native-core` feature（默认关闭）；在 `flow-engine` 内接 FFI 即可 |

## 7. 发布形态说明

demo 采用「目录分发」：`resources/` 与可执行文件同目录分发。
需要单文件安装包时，可把负载改为编译期嵌入（`include_bytes!`）或打包进自解压壳，
引擎接口无需变化（`src` 指向哪里，动作就复制哪里）。

## 8. 行为契约（请勿破坏）

- 窗口尺寸**运行时固定**：修改尺寸请改 `<window width height>`（或 `[window]` 覆盖），
  不要尝试在运行时改窗口大小；
- `install-dir` 输入框与 `create-shortcut` 复选框的 id 是引擎约定键；
- 卸载保留路径 `keep_user_data` 应位于安装目录**之外**（卸载会整目录删除）；
- 界面图标请使用矢量绘制（引擎内置样式），不要使用 emoji / 表情字符。
