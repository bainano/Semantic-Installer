# 架构设计（architecture.md）

## 1. 总体架构

```
┌─────────────────────────────────────────────────────────────┐
│ apps/installer │ apps/uninstaller   （薄二进制：加载配置→跑壳） │
└──────────────┬───────────────────────────┬──────────────────┘
               ▼                           ▼
┌──────────────────────────┐   ┌──────────────────────────────┐
│  crates/ui-adapter       │   │  crates/flow-engine           │
│  egui 适配层：固定窗口壳、 │   │  安装流程引擎：状态机、动作集、 │
│  节点树→egui 渲染、主题、  │◄──┤  执行器（后台线程）、回滚、    │
│  字体、事件→流程分发      │   │  安装/卸载清单读写、平台实现    │
└───────────┬──────────────┘   └──────────────────────────────┘
            ▼
┌─────────────────────────────────────────────────────────────┐
│  crates/semantic-ui  语义标签核心：词法/语法解析、AST、       │
│  固定画布布局引擎、组件模型、主题令牌（不依赖 egui，可单测）   │
└─────────────────────────────────────────────────────────────┘
```

**分层原则**：

- `semantic-ui` 与 `flow-engine` **零 GUI 依赖**——纯逻辑，可 headless 单测，并支撑
  `--dry-run` 无 GUI 全流程验证。
- `ui-adapter` 是**唯一**触碰 egui 的层。
- 安装器与卸载器共用同一 `ui-adapter` 壳 + 同一 `flow-engine`，仅以
  `Mode::{Install, Uninstall}` + 配置区分——即需求中的"统一底层架构"。

## 2. 固定尺寸取舍（重要）

需求中的"窗口尺寸灵活配置与动态调整"按最终裁定实现为：**尺寸可配置、运行时固定**。

- 权威尺寸来源：语义标记 `<window width height>`；`install.toml` 的 `[window]` 段为可选覆盖。
- 运行时：`eframe::ViewportBuilder::with_inner_size([w, h]).with_resizable(false).with_maximized(false)`，
  窗口不可拖动缩放、不可最大化。
- 画布尺寸为**逻辑单位**：在高 DPI 显示器上，egui 按系统缩放（native pixels-per-point）把逻辑画布
  放大为物理像素渲染，窗口物理尺寸 = 逻辑 × 系统缩放；布局始终以逻辑画布为权威，所见即所得。
- 布局引擎基于固定画布（fixed canvas）确定性计算：解析期即可算出每个节点的矩形，
  无需测量反馈循环；内容超出画布时产生**解析期告警**。
- 明确不做：滚动、响应式 / 自适应重排。未来版本可在此模型上扩展响应式布局，不推翻现有模型。

## 3. semantic-ui：标记语言与布局

### 3.1 词法 / 语法

手写词法 + 递归下降解析器（零第三方依赖）。语法与 HTML 类似：

```
<window title="Demo" width="900" height="620" theme="light">
  <page id="welcome" title="欢迎">
    <row> <text size="24">你好</text> </row>
  </page>
</window>
```

支持：元素、`key="value"` 属性、文本内容、`<!-- 注释 -->`、自闭合标签、
裸属性（值为 true）、`#rgb / #rrggbb` 颜色、实体解码（`&lt; &gt; &amp; &quot; &#39; &nbsp;`）。
解析错误带行列号；未知标签硬错误；重复属性警告。

### 3.2 布局引擎（固定画布）

- 画布 = `<window>` 声明的宽高。
- `row`（水平）/ `column`（垂直）流式排列，支持：
  - 固定像素尺寸（`width="240"` / `height="36"`）；
  - `weight="n"` 按比例分配剩余空间；
  - `fill`（缺省）占满剩余空间；
  - `spacer` 弹性占位（可声明 `width`/`height` 固定间隙）；
  - `gap`（间距）、`padding`（内边距）、`align`（Start / Center / End，作用于分配后的剩余空间）。
- 容器跨轴默认撑满；文本尺寸由 `Measurer` 抽象提供（GUI 层用真实字体度量，测试用桩）。
- **图层**：`<page>` 直接子节点为 `<layer>` 时，每个图层在**同一整幅画布**上独立布局，
  渲染按声明顺序（z 序）叠加；`bg` 属性可为图层铺背景色。非 layer 直接子节点忽略并告警，禁止嵌套图层。
- **i18n**：`lang/{locale}.toml` 键值资源，布局前按当前语言把 `i18n` 键文本替换进节点
  （未命中回退默认文本）；`select id="locale"` 运行时切换即整页重布局重渲染。
- **动画**：`animate="<kind> <dur>ms [infinite] [delay <n>ms]"` 为**渲染期**时间插值
  （fade/slide/spin/pulse/scale-in），不侵入布局矩形；一次性动画随页面进入播放。
- 输出 `PageModel { id, title, nodes, rects, warnings, canvas, layers }`——每个控件一个确定矩形。

### 3.3 组件模型与校验

`UiState` 保存输入 / 复选 / 单选 / 下拉的运行时值；`page_valid()` 判定页面是否满足
`required` 约束（必填输入非空、必选复选框已勾选）。`next / install / uninstall` 类前进按钮
在页面未通过校验时自动禁用（渲染层直接绘制为禁用态）。

## 4. flow-engine：安装流程引擎

### 4.1 清单模型（install.toml）

```toml
[app]      name / id / version / publisher / homepage
[window]   width / height / resizable(false) / theme   # 可选覆盖
[steps]    pages = ["welcome", ...]                     # 页面顺序
[install]
  install_dir = "$LOCALAPPDATA/Demo App"                # 目录模板
  copy_uninstaller = true                               # 复制卸载器进安装目录
  items: copy-dir | copy-file | create-dir | write-file
  shortcuts: name / target / args / icon / dir(desktop|start-menu)
  registry: uninstall_key + values(path/name/value)
  run_after: cmd / args / wait
[uninstall]
  keep_user_data = [...]       # 卸载保留路径（应位于安装目录之外）
  remove_shortcuts / remove_registry
```

路径占位符：`$LOCALAPPDATA $PROGRAMFILES $INSTALL_DIR $HOME $TEMP $EXEDIR`，
由 `platform::ResolveCtx` 按平台解析。

### 4.2 动作集与执行器

- 动作：`CopyDir / CopyFile / CreateDir / WriteFile / RemovePath / RemoveIfEmpty /
  CreateShortcut / RemoveShortcut / RegWrite / RegDelete / Run / CopyUninstaller`。
- 执行器在**后台线程**逐动作执行，经 `crossbeam-channel` 上报
  `ProgressEvent { done, total, message }`；UI 线程轮询重绘进度页。
- 取消：`Arc<AtomicBool>` 协作式取消。
- **回滚**：任一动作失败或取消时，按创建顺序逆序删除已创建的文件 / 目录 / 快捷方式 /
  注册表值 / 注册表键，保证安装失败不留残留。
- 全程写 `install.log`（动作描述 + 结果 + 耗时）。

### 4.3 安装记录（uninstall.json）

安装成功后写入：安装目录 `uninstall.json` + 用户级副本
`%LOCALAPPDATA%/<app_id>/uninstall.json`，供卸载器定位。记录包含：
app 信息、安装目录、创建的文件 / 目录 / 快捷方式 / 注册表值、卸载策略、
卸载器路径、**完整清单（flow）**。

### 4.4 平台能力（cfg 门控）

| 能力 | Windows | macOS / Linux |
| --- | --- | --- |
| 注册表读写 | `winreg` 实现 | no-op（记录日志） |
| .lnk 快捷方式 | `windows` crate COM（`ShellLink` + `IPersistFile`） | `.desktop`（Linux）/ `.command`（macOS） |
| 卸载登记 | HKCU「添加 / 删除程序」 | no-op |
| 用户级记录副本 | `%LOCALAPPDATA%` | `~/.local/share`（dirs） |

### 4.5 状态机

```
Idle → Validating（构建动作计划）→ Executing（后台）→ Progress（进度轮询）
     → Success（写记录 / 注册卸载入口） | Failed（回滚） | Cancelled（回滚）
```

## 5. ui-adapter：egui 适配层

- **固定窗口壳**：尺寸仅来自配置；`with_resizable(false)` + 禁最大化。
- **非系统标题栏模式（--no-titlebar）**：`with_decorations(false)` 去掉原生标题栏与边框，
  由引擎在客户区顶部自绘 32px 标题栏条（应用名 + 最小化 / 关闭矢量按钮 + 空白区拖动 `StartDrag`）；
  画布尺寸不变，标题栏条覆盖画布顶部（页面默认 padding ≥ 40 不受遮挡）。安装器 / 卸载器均支持。
- **透明标题栏（--titlebar-transparent）**：隐式启用 `--no-titlebar`，进一步省略标题栏条
  的背景块与分隔线——条区域完全透明，可透出 `<layer>` 背景图；应用名文字与最小化 / 关闭图标
  干净绘制（不叠加阴影），可读性交由开发者通过半透明图层（`layer bg="#rrggbbaa"`）自行控制；
  按钮 hover 仍显示背景块（最小化浅灰 / 关闭红色）。
- **渲染**：`PageModel` 节点树 → egui 组件——自绘按钮（圆角 / 主色 / 悬停 / 禁用）、
  自绘单选与进度条、`TextEdit` / `Checkbox` / `ComboBox` / `Texture`（PNG banner）映射；
  图层按 z 序整层渲染；动画用 `Painter` 的透明度 / 位移 / 旋转（`RectShape::with_angle`）/ 缩放实现。
  图标一律矢量绘制（`egui::Painter`），不使用 emoji / 表情字符。
- **字体**：探测系统 CJK 字体注入 `FontDefinitions`（Windows msyh / macOS PingFang /
  Linux NotoSansCJK），缺失时降级并告警。
- **主题**：语义令牌（bg / panel / text / primary / accent / danger / radius / spacing）→
  `egui::Visuals` + `Style`（按 `egui::Theme::Light/Dark` 设置）；`--theme light|dark` 可覆盖。
- **事件**：按钮 `action` → `events::handle_click` 分发：`next / back / cancel / install /
  uninstall / finish / goto / custom`；`link` 用系统默认浏览器打开 URL；页面标题同步窗口标题。
- **关闭策略**：工作期间收到关闭请求 → 先取消后台线程并等待回滚完成，再允许退出。

## 6. 可靠性设计（无泄漏、无崩溃）

1. **0 unsafe**（应用代码）：仅 `flow-engine/platform/windows.rs` 的 COM 互操作含两处
   经审计的 `unsafe`（`ShellLink` 创建与 `QueryInterface`），已逐行注释说明。
2. 所有权 / RAII 保证无泄漏；egui 纹理句柄随 `AssetCache` 生命周期释放。
3. UI 路径**禁止** `unwrap / expect`；标记 / 清单解析错误以友好错误页呈现（含行列号），
   进程不静默崩溃；`main` 全程返回 `ExitCode`。
4. 后台线程与 UI 线程仅经 channel 通信，无跨线程共享可变状态。
5. 全局 panic hook 记日志后优雅退出（eframe 默认行为 + 错误页兜底）。

## 7. 技术栈与版本

- Rust stable ≥ 1.92（eframe 0.35 MSRV）；本工程锁定 `rust-toolchain.toml`。
- `eframe / egui 0.35`（注意 0.35 为 Ui 中心化 API：`App::ui(&mut Ui, &mut Frame)`、
  `ctx.style_of/set_style_of(egui::Theme, …)`、`Rect` 字段私有化等）。
- `serde / toml / serde_json`、`crossbeam-channel`、`rfd`（目录选择）、`image`（PNG）、
  `winreg`、`windows`（仅 Windows target）、`env_logger`、`dirs`。
- 可选 `native-core` feature（C/C++ 原生核心扩展点）**默认关闭**——本引擎全 Rust 实现，
  预留 FFI 位置供性能敏感模块接入，不引入 C 工具链依赖。

## 8. 目录结构

```
crates/semantic-ui/src/   lib lexer parser ast layout widget theme
crates/flow-engine/src/   lib spec actions executor manifest dry_run platform/{mod,windows,posix}
crates/ui-adapter/src/    lib app render theme fonts assets events
apps/installer/src/       main.rs（参数解析 / 清单加载 / 固定窗口运行）
apps/uninstaller/src/     main.rs（记录定位 / 内置默认界面 / 运行）
examples/demo/            app.ui install.toml uninstall.ui resources/ README.md
```
