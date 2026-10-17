# 语义标记语言参考（markup-reference.md）

一份 `.ui` 文件 = 一个完整安装界面。语法与 HTML 类似：元素、`key="value"` 属性、文本内容、
`<!-- 注释 -->`。解析错误带行列号；未知标签硬错误。

## 1. 根节点 `<window>`

| 属性 | 默认 | 说明 |
| --- | --- | --- |
| `title` | — | 窗口标题 |
| `width` / `height` | 800 / 600 | **固定画布尺寸**（运行时不可缩放、不可最大化） |
| `theme` | `light` | `light` / `dark` |
| `locale` | 系统探测 | 界面语言代码（如 `zh-CN`；缺省时：`--lang` 参数 > 此属性 > 系统语言 > `zh-CN`） |

```xml
<window title="我的安装程序" width="900" height="620" theme="light" locale="zh-CN">
  <!-- 页面定义 -->
</window>
```

## 2. 页面 `<page>`

| 属性 | 说明 |
| --- | --- |
| `id` | 页面标识（与 install.toml `[steps].pages` 对应） |
| `title` | 页面标题（同步到窗口标题） |

```xml
<page id="welcome" title="欢迎">
  <!-- 页面内容 -->
</page>
```

## 3. 图层 `<layer>`（Phase 9）

`<layer>` 是 `<page>` 的直接子容器：多个图层在**同一固定画布**上按声明顺序（即 z 序）叠加，
每层独立布局、默认透明背景。用于「背景铺底 + 前景文字/控件」等场景，且不破坏固定画布约束。

| 属性 | 说明 |
| --- | --- |
| `id` | 图层标识 |
| `bg` | 图层背景色（`#rgb` / `#rrggbb` / `#rrggbbaa`，8 位含 alpha 支持半透明遮罩） |
| `visible` | `true` / `false` 控制显隐（可与动画联动，默认 `true`） |

规则与限制：
- 声明顺序即 z 序：后声明的层绘制在上方；
- 图层模式下，`<page>` 的**非 layer 直接子节点会被忽略并告警**（必须放进某个层）；
- **不允许图层嵌套**：`<layer>` 内出现 `<layer>` 时按普通容器布局并告警；
- 没有 `<layer>` 的页面保持传统单层布局（旧路径完全兼容）；
- **图层自身 `bg` 属性会先铺满整幅画布再渲染层内子节点**，支持 `#rrggbbaa` 半透明色——
  常用做法是背景图之上加一层半透明遮罩（如 `bg="#ffffffb0"`）压浅/压暗背景，保证前景文字可读。

```xml
<page id="welcome" title="欢迎">
  <layer id="bg" bg="#1f6feb">
    <image src="resources/banner.png" width="260" height="620"/>
  </layer>
  <layer id="fg">
    <row>
      <spacer width="260"/>
      <column gap="14" padding="40">
        <text size="30" weight="bold">欢迎</text>
        <spacer/>
        <row gap="8" align="end">
          <button action="next" primary="true">下一步</button>
        </row>
      </column>
    </row>
  </layer>
</page>
```

## 4. 布局容器 `<row>` / `<column>`

`row` 水平排列子项，`column` 垂直排列。子项尺寸规则：

| 属性 | 说明 |
| --- | --- |
| `width` / `height` | 固定像素尺寸 |
| `weight="n"` | 按比例分配剩余空间 |
| `fill`（缺省） | 占满剩余空间 |
| `gap` | 子项间距（像素） |
| `padding` | 内边距（像素，容器自身） |
| `align` | `start` / `center` / `end`（作用于分配后的剩余空间） |
| `bg` | 背景色（`#rgb` / `#rrggbb` / `#rrggbbaa`） |

```xml
<row gap="8" padding="20" align="end">
  <column width="240" bg="#1f6feb">…</column>
  <column gap="16">…</column>
</row>
```

## 5. 多语言 i18n（Phase 10）

语言资源为 `lang/{locale}.toml`（与 `.ui` 同目录），键与节点 `i18n` 属性对应：

```toml
# lang/zh-CN.toml
welcome.title = "欢迎"
button.next = "下一步"
```

- 任意带文本的节点（`text / label / button / link / checkbox / radio`）可写
  `i18n="键"`，资源命中时**布局与渲染**都使用资源文本；未命中时回退节点默认文本；
- `<page title-i18n="键">`：页面标题（`title` 属性）随语言资源翻译，窗口标题栏同步；
- 资源支持扁平键（`"welcome.title" = "…"`）与嵌套节（`[welcome] title = "…"`），统一归一为点分键；
- **语言优先级链**：`--lang` 参数 > `<window locale>` > 系统语言（Windows 注册表） > 默认 `zh-CN`；
  语言下拉（`id="locale"`）默认选中当前生效语言，保证界面语言与下拉一致；
- **运行时切换**：`<select id="locale" options="zh-CN=简体中文,en-US=English"/>`
  （引擎约定键 `id="locale"`；选项支持 `key=label` 显示本地化名称，值仍为语言代码），
  切换即全界面重渲染；
- **安装后联动**：清单 `[locale]` 段把所选语言写入应用配置 / 注册表，
  被安装程序启动时读取即可跟随语言；卸载器自动读取安装记录中的语言显示同语言界面。

```xml
<text i18n="welcome.title">欢迎</text>                       <!-- 回退默认文本 -->
<page id="locale" title="选择语言" title-i18n="page.locale"/> <!-- 页面标题随语言翻译 -->
<select id="locale" options="zh-CN=简体中文,en-US=English"/>   <!-- key=label 显示本地化名称 -->
```

```toml
# install.toml：联动写入安装目录 config/locale 文件 + 注册表值 Locale
[locale]
write_file = "config/locale"
write_registry = "HKCU\\Software\\Demo App"
```

## 6. 简单样式动画（Phase 11）

`animate` 属性声明样式动画（渲染期时间插值，不侵入布局引擎）：

```text
animate = "<kind> [<duration>ms] [infinite] [delay <n>ms]"
kind    = fade-in | slide-up | slide-down | slide-left | slide-right | spin | pulse | scale-in
```

| 种类 | 效果 | 缺省时长 |
| --- | --- | --- |
| `fade-in` | 透明度 0 → 1 | 300ms |
| `slide-up / -down / -left / -right` | 24px 位移 + 淡入 | 300ms |
| `spin` | 绕中心顺时针旋转一周 | 1200ms |
| `pulse` | 透明度呼吸脉冲（0.55 ↔ 1） | 800ms |
| `scale-in` | 0.8 倍缩放至原尺寸 + 淡入 | 300ms |

- 一次性动画在页面进入时播放（每次进入页面重新播放）；`infinite` 循环播放；
- 动画为**纯时间函数**：播放中每帧重绘，结束后恢复原布局矩形；
- 适用范围：fade / slide / pulse 对全部节点生效；
  **spin / scale-in 仅对带形状的节点生效**（容器背景、按钮、图片、进度条、分隔线），
  纯文本与 egui 原生控件（input / checkbox / select）不旋转缩放；
- 进度条本身已内置平滑插值（约 120ms 指数趋近），进度变化不跳变。

```xml
<text animate="fade-in 400ms">标题</text>
<text animate="slide-up 500ms">正文</text>
<row animate="fade-in 700ms">…</row>
<text animate="pulse 1500ms infinite">请勿关闭窗口</text>
<row animate="scale-in 300ms"><button action="finish">完成</button></row>
```

## 7. 组件标签

### `<text>` / `<label>`

| 属性 | 说明 |
| --- | --- |
| `size` | 字号（px，默认主题字号） |
| `weight` | `bold` |
| `color` | 文字颜色 |
| `align` | `left` / `center` / `right` |

`label` 是次要文本（默认使用 muted 颜色）。

> **多行文本**：`<text>` / `<label>` 内容中的换行会被保留（行内连续空白折叠为一个空格）。
> 布局引擎按「最长行」计算宽度、按「行数 × 行高」计算高度，因此多行文本可直接书写，无需
> 使用多个 `<text>` 节点。固定画布下整段文本超出画布时会产生布局告警（见 `layout` 告警日志）。

### `<button>`

| 属性 | 说明 |
| --- | --- |
| `action` | **`next` / `back` / `cancel` / `install` / `uninstall` / `finish` / `custom:*`** |
| `goto` | 点击后跳转的页面 id（与 action 叠加） |
| `primary` | `true` 渲染为主按钮（主色填充） |
| `enabled` | `false` 强制禁用 |

前进类按钮（`next / install / uninstall`）在页面 `required` 约束未满足时自动禁用。

```xml
<button action="next" primary="true">下一步</button>
<button action="back">上一步</button>
<button action="cancel">取消</button>
<button action="install" primary="true">安装</button>
<button action="finish" primary="true">完成</button>
```

### `<input>`

| 属性 | 说明 |
| --- | --- |
| `id` | 状态键（引擎读取：安装目录等） |
| `placeholder` | 占位文本 |
| `value` | 初始值 |
| `browse` | `true` 时附「浏览…」按钮（目录选择对话框，rfd） |
| `required` | 非空才允许前进 |

引擎约定：`id="install-dir"` 的输入框内容将覆盖清单中的默认安装目录。

### `<checkbox>` / `<radio>`

| 属性 | 说明 |
| --- | --- |
| `id` | checkbox 状态键 |
| `checked` | 初始勾选 |
| `required` | 必选（未勾选不允许前进） |
| `group` / `value` | radio 分组与值 |

引擎约定：`id="create-shortcut"` 的勾选控制是否创建快捷方式。

### `<select>`

| 属性 | 说明 |
| --- | --- |
| `id` | 状态键 |
| `options` | 逗号分隔的选项列表 |

### `<progress>`

| 属性 | 说明 |
| --- | --- |
| `id` | 状态键（引擎绑定进度条填充与文案） |

### `<image>`

| 属性 | 说明 |
| --- | --- |
| `src` | 图片路径（相对 `.ui` 所在目录，支持绝对路径；PNG） |
| `width` / `height` | 绘制尺寸 |

### 辅助标签

| 标签 | 作用 |
| --- | --- |
| `<link url="…">文本</link>` | 超链接（默认浏览器打开；`action` / `goto` 亦可） |
| `<divider/>` | 分隔线 |
| `<spacer/>` | 弹性占位（占据剩余空间，与 `weight` 协同；可声明 `width`/`height` 固定尺寸占用固定间隙） |
| `<label>` | 次要文本 |

## 8. 完整示例（installer）

```xml
<window title="Demo 安装程序" width="900" height="620" theme="light">
  <page id="welcome" title="欢迎">
    <layer id="bg" bg="#1f6feb">
      <image src="resources/banner.png" width="260" height="620"/>
    </layer>
    <layer id="fg">
      <row>
        <spacer width="260"/>
        <column gap="14" padding="40">
          <text size="30" weight="bold" i18n="welcome.title" animate="fade-in 400ms">Demo 安装程序</text>
          <spacer/>
          <text size="15" i18n="welcome.body" animate="slide-up 500ms">欢迎使用 Demo。点击“下一步”继续安装。</text>
          <spacer/>
          <row gap="8" align="end" animate="fade-in 700ms">
            <button action="next" primary="true" i18n="button.next">下一步</button>
            <button action="cancel" i18n="button.cancel">取消</button>
          </row>
        </column>
      </row>
    </layer>
  </page>

  <page id="license" title="许可协议">
    <column gap="12" padding="44">
      <text size="24" weight="bold" i18n="license.title">最终用户许可协议</text>
      <divider/>
      <text size="14" color="#444">MIT License…</text>
      <spacer/>
      <checkbox id="accept-license" required="true" i18n="license.accept">我已阅读并同意</checkbox>
      <row gap="8" align="end">
        <button action="back" i18n="button.back">上一步</button>
        <button action="next" primary="true" i18n="button.agree">我同意，继续</button>
      </row>
    </column>
  </page>

  <page id="options" title="安装选项">
    <column gap="14" padding="44">
      <text size="24" weight="bold" i18n="options.title">安装选项</text>
      <input id="install-dir" placeholder="$LOCALAPPDATA/Demo App" browse="true"/>
      <checkbox id="create-shortcut" checked="true" i18n="options.shortcut">创建桌面快捷方式</checkbox>
      <select id="locale" options="zh-CN=简体中文,en-US=English"/>
      <spacer/>
      <row gap="8" align="end">
        <button action="back" i18n="button.back">上一步</button>
        <button action="install" primary="true" i18n="button.install">安装</button>
        <button action="cancel" i18n="button.cancel">取消</button>
      </row>
    </column>
  </page>

  <page id="progress" title="正在安装">
    <column gap="16" padding="44">
      <text size="24" weight="bold" i18n="progress.title">正在安装…</text>
      <progress id="progress"/>
    </column>
  </page>

  <page id="finish" title="安装完成">
    <column gap="14" padding="44">
      <text size="24" weight="bold" i18n="finish.title" animate="fade-in 300ms">安装完成</text>
      <spacer/>
      <row gap="8" align="end" animate="scale-in 300ms">
        <button action="finish" primary="true" i18n="button.finish">完成</button>
      </row>
    </column>
  </page>
</window>
```

## 9. 引擎与标记的约定键

| 约定 | 说明 |
| --- | --- |
| 页面 id | 与 `install.toml` `[steps].pages` 对应（缺省时用标记声明顺序） |
| `<input id="install-dir">` | 用户输入的安装目录覆盖清单默认值 |
| `<checkbox id="create-shortcut">` | 控制快捷方式创建 |
| `<progress id="progress">` | 进度条填充与文案由引擎绑定 |
| `<select id="locale">` | 界面语言切换（引擎约定键；值如 `zh-CN`、`en-US`） |
| `lang/{locale}.toml` | i18n 资源（与 `.ui` 同目录） |
| 固定画布 | 内容必须适配画布尺寸；超出会触发解析期告警（不做滚动 / 自适应） |
