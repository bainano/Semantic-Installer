# 语义标记语言参考（markup-reference.md）

一份 `.ui` 文件 = 一个完整安装界面。语法与 HTML 类似：元素、`key="value"` 属性、文本内容、
`<!-- 注释 -->`。解析错误带行列号；未知标签硬错误。

## 1. 根节点 `<window>`

| 属性 | 默认 | 说明 |
| --- | --- | --- |
| `title` | — | 窗口标题 |
| `width` / `height` | 800 / 600 | **固定画布尺寸**（运行时不可缩放、不可最大化） |
| `theme` | `light` | `light` / `dark` |

```xml
<window title="我的安装程序" width="900" height="620" theme="light">
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

## 3. 布局容器 `<row>` / `<column>`

`row` 水平排列子项，`column` 垂直排列。子项尺寸规则：

| 属性 | 说明 |
| --- | --- |
| `width` / `height` | 固定像素尺寸 |
| `weight="n"` | 按比例分配剩余空间 |
| `fill`（缺省） | 占满剩余空间 |
| `gap` | 子项间距（像素） |
| `padding` | 内边距（像素，容器自身） |
| `align` | `start` / `center` / `end`（作用于分配后的剩余空间） |
| `bg` | 背景色（`#rgb` / `#rrggbb`） |

```xml
<row gap="8" padding="20" align="end">
  <column width="240" bg="#1f6feb">…</column>
  <column gap="16">…</column>
</row>
```

## 4. 组件标签

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
| `<spacer/>` | 弹性占位（占据剩余空间，与 `weight` 协同） |
| `<label>` | 次要文本 |

## 5. 完整示例（installer）

```xml
<window title="Demo 安装程序" width="900" height="620" theme="light">
  <page id="welcome" title="欢迎">
    <row>
      <column width="260" bg="#1f6feb">
        <image src="resources/banner.png" width="260" height="620"/>
      </column>
      <column gap="14" padding="44">
        <text size="30" weight="bold">Demo 安装程序</text>
        <spacer/>
        <text size="15">欢迎使用 Demo。点击“下一步”继续安装。</text>
        <spacer/>
        <row gap="8" align="end">
          <button action="next" primary="true">下一步</button>
          <button action="cancel">取消</button>
        </row>
      </column>
    </row>
  </page>

  <page id="license" title="许可协议">
    <column gap="12" padding="44">
      <text size="24" weight="bold">最终用户许可协议</text>
      <divider/>
      <text size="14" color="#444">MIT License…</text>
      <spacer/>
      <checkbox id="accept-license" required="true">我已阅读并同意</checkbox>
      <row gap="8" align="end">
        <button action="back">上一步</button>
        <button action="next" primary="true">我同意，继续</button>
      </row>
    </column>
  </page>

  <page id="options" title="安装选项">
    <column gap="14" padding="44">
      <text size="24" weight="bold">安装选项</text>
      <input id="install-dir" placeholder="$LOCALAPPDATA/Demo App" browse="true"/>
      <checkbox id="create-shortcut" checked="true">创建桌面快捷方式</checkbox>
      <spacer/>
      <row gap="8" align="end">
        <button action="back">上一步</button>
        <button action="install" primary="true">安装</button>
        <button action="cancel">取消</button>
      </row>
    </column>
  </page>

  <page id="progress" title="正在安装">
    <column gap="16" padding="44">
      <text size="24" weight="bold">正在安装…</text>
      <progress id="progress"/>
    </column>
  </page>

  <page id="finish" title="安装完成">
    <column gap="14" padding="44">
      <text size="24" weight="bold">安装完成</text>
      <spacer/>
      <row gap="8" align="end">
        <button action="finish" primary="true">完成</button>
      </row>
    </column>
  </page>
</window>
```

## 6. 引擎与标记的约定键

| 约定 | 说明 |
| --- | --- |
| 页面 id | 与 `install.toml` `[steps].pages` 对应（缺省时用标记声明顺序） |
| `<input id="install-dir">` | 用户输入的安装目录覆盖清单默认值 |
| `<checkbox id="create-shortcut">` | 控制快捷方式创建 |
| `<progress id="progress">` | 进度条填充与文案由引擎绑定 |
| 固定画布 | 内容必须适配画布尺寸；超出会触发解析期告警（不做滚动 / 自适应） |
