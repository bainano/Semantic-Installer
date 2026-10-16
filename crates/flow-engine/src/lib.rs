//! `flow-engine`：跨平台安装 / 卸载流程引擎（零 GUI 依赖）。
//!
//! - [`spec`]：`install.toml` 清单的 serde 模型；
//! - [`actions`]：运行时动作集与计划构建（占位符解析）；
//! - [`executor`]：后台执行器（进度上报 / 取消 / 失败逆序回滚）；
//! - [`manifest`]：安装记录 uninstall.json 的读写；
//! - [`platform`]：Windows / macOS / Linux 平台能力（注册表、快捷方式等）；
//! - [`dry_run`]：无 GUI 的 --dry-run 全流程模拟（用于自动化验证）。

pub mod actions;
pub mod dry_run;
pub mod executor;
pub mod manifest;
pub mod platform;
pub mod spec;

pub use actions::{
    build_install_actions, build_uninstall_actions, record_from_report, Action, InstallInputs,
    ShortcutDir,
};
pub use dry_run::{dry_run_install, dry_run_uninstall};
pub use executor::{execute, Created, ExecReport, Outcome, ProgressEvent};
pub use manifest::{find_record, load_record, write_record, InstallRecord};
pub use platform::{
    local_app_data, resolve_install_dir, resolve_source, resolve_template, ResolveCtx,
};
pub use spec::{
    AppSpec, FlowSpec, InstallSpec, ItemSpec, RegistrySpec, RegistryValueSpec, RunSpec,
    ShortcutSpec, StepsSpec, UninstallSpec, WindowSpec,
};
