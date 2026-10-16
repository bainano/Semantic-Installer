# 测试与验证（testing.md）

## 1. 自动化测试

```powershell
# 全部测试（解析器 / 布局 / 流程引擎 / 记录往返）
cargo test

# 静态检查（零警告要求）
cargo clippy --all-targets -- -D warnings
```

当前测试基线（提交时）：

| 目标 | 数量 | 覆盖 |
| --- | --- | --- |
| `semantic-ui` | 25 | 词法 / 语法 / 错误定位 / 实体解码 / 布局数学（固定 / 比例 / 弹性 / 对齐）/ 页面校验 |
| `flow-engine` | 18 | 清单解析 / 占位符解析 / 动作构建 / 执行器（temp 目录真实文件操作）/ 失败逆序回滚 / 取消回滚 / 记录往返 |

### 测试要点

- 布局引擎用 `Stub` 度量器验证固定画布数学：`Auto / Fixed / Fill / Weight(n)`、
  `spacer` 弹性、`align` 作用于 leftover、容器跨轴撑满、溢出告警。
- 执行器在 `std::env::temp_dir()` 下创建真实文件验证复制 / 写入 / 删除与**逆序回滚**
  （先建目录 → 再建子文件，失败后子文件先删、目录后删）。
- `--dry-run` 是二进制级验证：不触碰文件系统，仅构建并打印动作计划，
  同时校验负载源存在性。

## 2. 无界面全流程验证（CI / 快速回归）

```powershell
# 安装计划（含源存在性校验）
installer.exe --manifest examples\demo\install.toml --dry-run install

# 卸载计划（需一份 uninstall.json）
uninstaller.exe --record <path\to\uninstall.json> --dry-run
```

`--dry-run` 会打印：应用信息、安装 / 卸载目录、每个动作的描述（源 → 目标、
快捷方式名称、注册表路径）、预期回滚顺序。

## 3. GUI 冒烟（Windows）

1. `cargo build --release`；
2. 运行安装器（demo 配置），检查：窗口按配置尺寸出现、不可缩放、不可最大化；
   中文渲染正常；欢迎页 banner 显示；「下一步」在许可未勾选时禁用、勾选后可用；
   页面导航正确；安装进度条随动作推进；完成后进入完成页并写入 uninstall.json。
3. 从「控制面板 → 程序和功能」（或直接运行卸载器）卸载：确认页 → 进度 → 完成，
   安装目录被清理；`keep_user_data` 指定的路径保留。
4. 关闭窗口时（安装进行中）应先出现取消回滚再退出，不留残留文件。

> 冒烟可用临时清单覆盖安装目录为 `$TEMP/semantic-demo-install` 以便逆操作。
> 本仓库提交时在 Windows 10/11 实机验证；macOS / Linux 已验证可编译与核心流程
> （`cargo build --target x86_64-apple-darwin / x86_64-unknown-linux-gnu`），
> 对应硬件上的 GUI 表现未实测（文档明示）。

## 4. 稳定性与内存

- 应用代码 0 unsafe（仅 Windows COM 两处经审计 unsafe）；
- UI 路径无 `unwrap / expect`，解析错误以错误页呈现；
- 后台线程与 UI 线程仅经 channel 通信；
- 长时间运行（连续多次安装 / 卸载）无泄漏：所有动态资源均为 RAII 管理
  （纹理句柄随缓存释放、线程 join、文件句柄即时关闭）。

## 5. 验证清单（发布前）

- [ ] `cargo test` 全绿
- [ ] `cargo clippy --all-targets -- -D warnings` 零警告
- [ ] `installer --dry-run install` 通过
- [ ] `uninstaller --record … --dry-run` 通过
- [ ] GUI：窗口固定尺寸 / 中文渲染 / 导航 / 进度 / 完成 / 卸载 / 取消回滚
- [ ] 安装 → 卸载闭环后系统无残留（注册表、快捷方式、目录）
