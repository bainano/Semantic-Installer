//! 应用壳：固定尺寸窗口 + 页面导航 + 安装 / 卸载流程编排。
//!
//! 固定尺寸约束：窗口尺寸仅来自清单 / 标记配置，运行时禁缩放、禁最大化；
//! 布局引擎按固定画布确定性计算，引擎不做自适应（产品化取舍，见 docs）。

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crossbeam_channel::Receiver;
use eframe::egui;
use semantic_ui::ast::Node;
use semantic_ui::layout::{layout_page, Measurer, PageModel, Rect};
use semantic_ui::widget::{Click, UiState};
use semantic_ui::{FontWeight, Theme};

use crate::assets::AssetCache;
use crate::events;
use crate::{fonts, render, theme};
use semantic_ui::i18n::I18n;

/// 运行模式：安装器 / 卸载器（统一底层架构）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Install,
    Uninstall,
}

/// 应用壳配置。
pub struct ShellOptions {
    pub mode: Mode,
    pub flow: flow_engine::FlowSpec,
    pub markup_root: Node,
    pub root_dir: PathBuf,
    pub theme_override: Option<String>,
    pub uninstaller_path: Option<PathBuf>,
    pub record: Option<flow_engine::InstallRecord>,
    /// 语言覆盖（--lang），优先级最高。
    pub lang: Option<String>,
    /// 非系统标题栏模式（--no-titlebar）：窗口无原生标题栏，客户区内自绘标题栏条。
    pub no_titlebar: bool,
    /// 透明标题栏（--titlebar-transparent）：自绘标题栏条背景透明（透出背景图），
    /// 文字带阴影保证可读；按钮 hover 才出现背景块。
    pub titlebar_transparent: bool,
}

/// 从清单 / 标记解析固定窗口尺寸。
pub fn window_size_of(flow: &flow_engine::FlowSpec, root: &Node) -> (f32, f32) {
    if let Some(w) = &flow.window
        && w.width > 0 && w.height > 0 {
            return (w.width as f32, w.height as f32);
        }
    (
        root.attr_float("width", 800.0),
        root.attr_float("height", 600.0),
    )
}

pub(crate) enum Phase {
    Idle,
    Working,
    Done,
    Failed(String),
}

pub(crate) struct WorkerHandle {
    pub(crate) rx: Receiver<flow_engine::ProgressEvent>,
    pub(crate) cancel: Arc<AtomicBool>,
    pub(crate) join: Option<std::thread::JoinHandle<flow_engine::ExecReport>>,
}

/// egui 测量适配：用真实字体度量文本。
struct CtxMeasurer<'a> {
    ctx: &'a egui::Context,
}

impl Measurer for CtxMeasurer<'_> {
    fn text_size(&self, text: &str, size: f32, _weight: FontWeight) -> (f32, f32) {
        // 多行文本：宽度 = 最长行宽，高度 = 行数 × 行高（与渲染端 painter.text 多行行为一致）
        let font_id = egui::FontId::proportional(size);
        self.ctx.fonts_mut(|f| {
            let mut w = 0.0f32;
            let mut h = 0.0f32;
            let line_h = (size * 1.3).max(8.0);
            for line in text.split('\n') {
                let galley = f.layout_no_wrap(line.to_owned(), font_id.clone(), egui::Color32::WHITE);
                w = w.max(galley.size().x);
                h += line_h.max(galley.size().y);
            }
            (w, h)
        })
    }
}

/// 安装器 / 卸载器共用应用壳。
pub struct InstallerApp {
    pub(crate) opts: ShellOptions,
    pub(crate) pages: HashMap<String, PageModel>,
    pub(crate) order: Vec<String>,
    pub(crate) current: usize,
    pub(crate) state: UiState,
    pub(crate) theme: Theme,
    pub(crate) phase: Phase,
    pub(crate) worker: Option<WorkerHandle>,
    pub(crate) last_progress: Option<flow_engine::ProgressEvent>,
    pub(crate) assets: AssetCache,
    pub(crate) ready: bool,
    pub(crate) window: (f32, f32),
    pub(crate) install_dir: Option<PathBuf>,
    pub(crate) locale: String,
    pub(crate) i18n: I18n,
    /// 页面进入时刻（egui 全局时间秒），动画时间基准。
    page_entered_at: f64,
    /// 上一帧渲染的页面序号（检测页面切换以重置动画基准）。
    last_page_index: usize,
    /// 最近一次写入窗口标题栏的文本（变化时才重发 ViewportCommand）。
    last_title: Option<String>,
    /// 非系统标题栏模式（--no-titlebar）：自绘标题栏条高度。
    pub(crate) no_titlebar: bool,
    /// 透明标题栏模式（--titlebar-transparent）：条背景透明、文字带阴影。
    pub(crate) titlebar_transparent: bool,
    /// 临时诊断：官方截图帧计数。
    shot_frame: u32,
    /// 截图请求队列（页面 id, 序号）——请求后等待 Event::Screenshot 回来再保存。
    shot_requests: std::collections::VecDeque<(String, u32)>,
    /// 截图序号。
    shot_seq: u32,
    /// 自动翻页截图是否已覆盖最后页面（finish），之后停止请求。
    autopage_done: bool,
    /// start_install 后快速截图进度页的帧号（不等待 60 帧周期，避免安装过快错过）。
    shot_progress_at: Option<u32>,
}

/// 系统语言探测（Windows：注册表 `LocaleName`，如 "zh-CN"；其他平台回退 None）。
#[cfg(windows)]
fn detect_system_locale() -> Option<String> {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let key = hkcu.open_subkey(r"Control Panel\International").ok()?;
    let v: String = key.get_value("LocaleName").ok()?;
    (!v.is_empty()).then_some(v)
}

#[cfg(not(windows))]
fn detect_system_locale() -> Option<String> {
    None
}

impl InstallerApp {
    pub fn new(opts: ShellOptions) -> Self {
        let theme_name = opts
            .theme_override
            .clone()
            .or_else(|| opts.flow.window.as_ref().and_then(|w| w.theme.clone()))
            .unwrap_or_else(|| "light".to_owned());
        let theme = Theme::named(&theme_name);
        let window = window_size_of(&opts.flow, &opts.markup_root);

        // 页面顺序：清单 [steps] 优先，否则标记声明顺序
        let order: Vec<String> = if !opts.flow.steps.pages.is_empty() {
            opts.flow.steps.pages.clone()
        } else {
            opts.markup_root
                .children_of("page")
                .filter_map(|p| p.attr("id").map(str::to_owned))
                .collect()
        };

        // 从标记种子化初始 UI 状态（输入默认值 / 勾选 / 单选）
        let mut state = UiState::new();
        for node in semantic_ui::parser::walk(&opts.markup_root) {
            match node.tag.as_str() {
                "input" => {
                    if let Some(id) = node.attr("id")
                        && let Some(v) = node.attr("value") {
                            state.set_input(id, v);
                        }
                }
                "checkbox" => {
                    if let Some(id) = node.attr("id") {
                        state.set_checked(id, node.flag("checked", false));
                    }
                }
                "radio" => {
                    if let (Some(group), Some(value)) = (node.attr("group"), node.attr("value"))
                        && node.flag("checked", false) {
                            state.set_radio(group, value);
                        }
                }
                _ => {}
            }
        }

        // root_dir 需在 opts 移入结构体前取出（语言资源与图标加载基准）
        let root_dir = opts.root_dir.clone();
        let no_titlebar = opts.no_titlebar;
        let titlebar_transparent = opts.titlebar_transparent;

        // 语言：--lang 参数 > <window locale> > 系统语言 > 默认 zh-CN
        let locale = opts
            .lang
            .clone()
            .or_else(|| opts.markup_root.attr("locale").map(str::to_owned))
            .or_else(detect_system_locale)
            .unwrap_or_else(|| "zh-CN".to_owned());
        let i18n = I18n::load(&root_dir, &locale);

        // 语言下拉（id="locale"）默认选中当前生效语言：--lang > 窗口 locale > 系统语言 > zh-CN，
        // 保证界面语言与下拉显示一致（例如英文系统默认界面为英文、下拉亦选中英文）。
        state.set_selected("locale", &locale);

        Self {
            opts,
            pages: HashMap::new(),
            order,
            current: 0,
            state,
            theme,
            phase: Phase::Idle,
            worker: None,
            last_progress: None,
            assets: AssetCache::new(),
            ready: false,
            window,
            install_dir: None,
            locale,
            i18n,
            page_entered_at: 0.0,
            last_page_index: 0,
            last_title: None,
            no_titlebar,
            titlebar_transparent,
            shot_frame: 0,
            shot_requests: std::collections::VecDeque::new(),
            shot_seq: 0,
            autopage_done: false,
            shot_progress_at: None,
        }
    }

    fn prepare(&mut self, ctx: &egui::Context) {
        fonts::install_cjk(ctx);
        let canvas = Rect::new(0.0, 0.0, self.window.0, self.window.1);
        let measurer = CtxMeasurer { ctx };
        let mut any_error = false;
        // 先按当前语言解析标记文本（i18n 键 → 资源文本），再布局
        for i in 0..self.opts.markup_root.children.len() {
            if !self.opts.markup_root.children[i].is("page") {
                continue;
            }
            let page = &mut self.opts.markup_root.children[i];
            semantic_ui::resolve_i18n(page, &self.i18n);
            let id = page.attr("id").unwrap_or_default().to_owned();
            match layout_page(page, canvas, &self.theme, &measurer) {
                Ok(model) => {
                    for w in &model.warnings {
                        log::warn!("[布局] {w}");
                    }
                    self.pages.insert(id, model);
                }
                Err(e) => {
                    log::error!("页面布局失败：{e}");
                    any_error = true;
                }
            }
        }
        if any_error {
            self.phase = Phase::Failed("页面布局失败，请检查语义标记配置。".into());
        }
        self.ready = true;
    }

    pub(crate) fn current_model(&self) -> Option<&PageModel> {
        let id = self.order.get(self.current)?;
        self.pages.get(id)
    }

    /// 当前页 id（顺序数组中的 id 字符串）。
    fn current_page_id(&self) -> Option<&str> {
        self.order.get(self.current).map(String::as_str)
    }

    pub(crate) fn goto(&mut self, id: &str) {
        if let Some(i) = self.order.iter().position(|p| p == id) {
            self.current = i;
        } else {
            log::warn!("尝试跳转到不存在的页面：{id}");
        }
    }

    fn spawn_worker(&mut self, actions: Vec<flow_engine::Action>) {
        let (tx, rx) = crossbeam_channel::unbounded();
        let cancel = Arc::new(AtomicBool::new(false));
        let cancel2 = Arc::clone(&cancel);
        let join = std::thread::Builder::new()
            .name("flow-worker".into())
            .spawn(move || flow_engine::execute(actions, tx, cancel2))
            .expect("创建后台工作线程失败");
        self.worker = Some(WorkerHandle {
            rx,
            cancel,
            join: Some(join),
        });
        self.phase = Phase::Working;
        self.last_progress = None;
    }

    pub(crate) fn start_install(&mut self) {
        if self.opts.flow.install.is_none() {
            self.phase = Phase::Failed("清单缺少 [install] 段。".into());
            return;
        }
        let root = self.opts.root_dir.clone();
        let override_dir = self.state.input("install-dir").trim().to_owned();
        let inputs = flow_engine::InstallInputs {
            install_dir_override: (!override_dir.is_empty()).then_some(override_dir),
            create_shortcut: self.state.checked("create-shortcut"),
            uninstaller_path: self.opts.uninstaller_path.clone(),
            locale: Some(self.locale.clone()),
        };
        match flow_engine::build_install_actions(&self.opts.flow, &root, &inputs) {
            Ok((actions, install_dir)) => {
                log::info!("开始安装到 {}", install_dir.display());
                self.install_dir = Some(install_dir);
                self.spawn_worker(actions);
                self.goto("progress");
            }
            Err(e) => {
                log::error!("构建安装计划失败：{e}");
                self.phase = Phase::Failed(e);
            }
        }
    }

    pub(crate) fn start_uninstall(&mut self) {
        let Some(record) = self.opts.record.clone() else {
            self.phase = Phase::Failed("缺少安装记录。".into());
            return;
        };
        match flow_engine::build_uninstall_actions(&record) {
            Ok(actions) => {
                log::info!(
                    "开始卸载 {}（安装于 {}）",
                    record.app.name,
                    record.install_dir
                );
                self.install_dir = Some(PathBuf::from(&record.install_dir));
                self.spawn_worker(actions);
                self.goto("progress");
            }
            Err(e) => {
                log::error!("构建卸载计划失败：{e}");
                self.phase = Phase::Failed(e);
            }
        }
    }

    fn poll_worker(&mut self, ctx: &egui::Context) {
        let Some(worker) = &self.worker else { return };
        // 消费进度；发送端关闭（Disconnected）= 工作线程已结束
        let mut finished = false;
        loop {
            match worker.rx.try_recv() {
                Ok(ev) => self.last_progress = Some(ev),
                Err(crossbeam_channel::TryRecvError::Empty) => break,
                Err(crossbeam_channel::TryRecvError::Disconnected) => {
                    finished = true;
                    break;
                }
            }
        }
        ctx.request_repaint_after(Duration::from_millis(100));
        if finished {
            let worker = self.worker.take().expect("worker 存在");
            let report = worker
                .join
                .and_then(|j| j.join().ok())
                .expect("工作线程应正常返回");
            self.last_progress = None;
            match report.outcome {
                flow_engine::Outcome::Success => self.finish_flow(report),
                flow_engine::Outcome::Failed { error, .. } => {
                    log::error!("流程执行失败：{error}");
                    self.phase = Phase::Failed(format!("操作失败：{error}"));
                }
                flow_engine::Outcome::Cancelled { .. } => {
                    self.phase = Phase::Failed("操作已取消，已执行回滚。".into());
                }
            }
        }
    }

    fn finish_flow(&mut self, report: flow_engine::ExecReport) {
        match self.opts.mode {
            Mode::Install => {
                let Some(dir) = self.install_dir.clone() else {
                    self.phase = Phase::Failed("缺少安装目录。".into());
                    return;
                };
                let flow = self.opts.flow.clone();
                let uninstall_spec = flow.uninstall.clone().unwrap_or_default();
                let uninstaller_installed = report.created.iter().find_map(|c| {
                    if let flow_engine::Created::File(p) = c {
                        let name = p
                            .file_name()
                            .map(|n| n.to_string_lossy().into_owned())
                            .unwrap_or_default();
                        if name.to_ascii_lowercase().contains("uninstaller") {
                            return Some(p.clone());
                        }
                    }
                    None
                });
                let record = flow_engine::record_from_report(
                    &flow,
                    &dir,
                    &uninstall_spec,
                    &report.created,
                    uninstaller_installed.as_deref(),
                    Some(self.locale.clone()),
                );
                if let Err(e) = flow_engine::write_record(&record, &dir, &flow.app.id) {
                    log::error!("写入安装记录失败：{e}");
                }
                if let Some(up) = &record.uninstaller_path {
                    let up = PathBuf::from(up);
                    if let Err(e) = flow_engine::platform::register_uninstall(&flow.app, &up, &dir) {
                        log::error!("注册卸载入口失败：{e}");
                    }
                }
                let _ = std::fs::write(dir.join("install.log"), report.log.join("\n"));
                self.goto("finish");
                self.phase = Phase::Done;
            }
            Mode::Uninstall => {
                if let Some(dir) = &self.install_dir {
                    let _ = std::fs::write(dir.join("uninstall.log"), report.log.join("\n"));
                }
                if let Some(record) = &self.opts.record
                    && record.uninstall_key {
                        let _ = flow_engine::platform::unregister_uninstall(&record.app.id);
                    }
                self.goto("finish");
                self.phase = Phase::Done;
            }
        }
    }

    /// 卸载完成后延迟清理安装目录（Windows 下卸载器自身被占用，交给 cmd 延时删除）。
    pub(crate) fn schedule_self_cleanup(&self) {
        if self.opts.mode != Mode::Uninstall {
            return;
        }
        let Some(dir) = &self.install_dir else { return };
        if !dir.exists() {
            return;
        }
        #[cfg(windows)]
        {
            let cmd = format!(
                "timeout /t 1 /nobreak >nul & rmdir /s /q \"{}\"",
                dir.display()
            );
            match std::process::Command::new("cmd").arg("/c").arg(&cmd).spawn() {
                Ok(_) => log::info!("已安排延迟清理安装目录 {}", dir.display()),
                Err(e) => log::warn!("安排延迟清理失败：{e}"),
            }
        }
        #[cfg(not(windows))]
        {
            let _ = std::fs::remove_dir_all(dir);
        }
    }
}

impl eframe::App for InstallerApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if !self.ready {
            self.prepare(&ctx);
        }
        theme::apply(&ctx, &self.theme);

        // 关闭请求：工作期间先取消并等待回滚，避免进程退出杀死后台线程
        let close_requested = ctx.input(|i| i.viewport().close_requested());
        if close_requested {
            if matches!(self.phase, Phase::Working) {
                if let Some(w) = &self.worker {
                    w.cancel.store(true, Ordering::Relaxed);
                }
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            } else {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                return;
            }
        }

        self.poll_worker(&ctx);

        // 动画时间基准：页面切换后重置（每次进入页面动画重放）
        let now = ctx.input(|i| i.time);
        if self.current != self.last_page_index {
            self.last_page_index = self.current;
            self.page_entered_at = now;
        }
        let anim_t = now - self.page_entered_at;

        // 验证机制：SEMANTIC_SHOT=1 时请求 egui 官方截图（唯一可靠的窗口渲染验证手段）。
        //  - 默认模式：第 90 帧截一张当前页，输出 SEMANTIC_SHOT_OUT（默认 plain-shot.png）。
        //  - SEMANTIC_SHOT_AUTOPAGE=1 自动翻页模式：每 60 帧截当前页，间隔后按流程自动前进，
        //    覆盖 locale → welcome → license → options → progress → finish 全流程，
        //    输出到 SEMANTIC_SHOT_DIR（默认 shots/），文件名 {seq:02d}-{page}.png。
        //  - SEMANTIC_SHOT_PAGE=xxx：首帧跳转到指定页（progress 自动开始安装）。
        //  - SEMANTIC_SHOT_EXIT=1：截图保存后优雅关闭窗口。
        if std::env::var("SEMANTIC_SHOT").is_ok() {
            // 强制持续重绘：无动画页面（如 license/options）在数帧后事件循环会进入空闲，
            // 永远到不了目标截图帧；以 60fps 请求持续重绘直至截图完成。
            ctx.request_repaint_after(Duration::from_millis(16));
            self.shot_frame += 1;
            let autopage = std::env::var("SEMANTIC_SHOT_AUTOPAGE").is_ok();
            let page_id = self.current_page_id().map(str::to_owned);
            // 指定目标页：SEMANTIC_SHOT_PAGE=xxx 时首帧直接跳转
            if self.shot_frame == 1 {
                if let Ok(target) = std::env::var("SEMANTIC_SHOT_PAGE") {
                    if !target.is_empty() {
                        self.goto(&target);
                    }
                }
            }
            // progress 页截图：延迟到第 16 帧再开始安装，先截到安装前的进度页画面
            //（demo 安装极快，若首帧即 start_install，截图帧时流程已跳到 finish）
            if self.shot_frame == 16
                && std::env::var("SEMANTIC_SHOT_PAGE").as_deref() == Ok("progress")
            {
                self.start_install();
            }
            if autopage && !self.autopage_done {
                let f = self.shot_frame;
                // 每个 60 帧周期：f%60==1 截图当前页，f%60==45 执行流程动作
                if f % 60 == 1 {
                    if let Some(id) = &page_id {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
                        self.shot_requests.push_back((id.clone(), self.shot_seq));
                        self.shot_seq += 1;
                        if id == "finish" {
                            self.autopage_done = true;
                        }
                    }
                } else if f % 60 == 45 {
                    match page_id.as_deref() {
                        Some("locale" | "welcome" | "license") => {
                            if self.current + 1 < self.order.len() {
                                self.current += 1;
                            }
                        }
                        Some("options") => {
                            self.start_install();
                            self.shot_progress_at = Some(self.shot_frame + 3);
                        }
                        _ => {}
                    }
                }
                // progress 页快速截图（进入后 3 帧，不等 60 帧周期）
                if let Some(at) = self.shot_progress_at
                    && f >= at {
                        self.shot_progress_at = None;
                        ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
                        self.shot_requests.push_back(("progress".into(), self.shot_seq));
                        self.shot_seq += 1;
                    }
            } else {
                // 单张模式：截当前页（可用 SEMANTIC_SHOT_PAGE 指定目标页）。
                // progress 页在安装进行中（第 15 帧）截图，避免安装过快直接跳到 finish。
                let shot_at = if page_id.as_deref() == Some("progress") { 15 } else { 90 };
                if self.shot_frame == shot_at {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
                    self.shot_requests.push_back((page_id.unwrap_or_default(), self.shot_seq));
                    self.shot_seq += 1;
                }
            }
            if let Some(img) = ctx.input(|i| {
                i.events.iter().find_map(|e| match e {
                    egui::Event::Screenshot { image, .. } => Some(image.clone()),
                    _ => None,
                })
            }) {
                if let Some((page, seq)) = self.shot_requests.pop_front() {
                    let (w, h) = (img.width() as u32, img.height() as u32);
                    let mut buf = Vec::with_capacity((w * h * 4) as usize);
                    for p in &img.pixels {
                        buf.extend_from_slice(&[p.r(), p.g(), p.b(), p.a()]);
                    }
                    let dir = std::env::var("SEMANTIC_SHOT_DIR").unwrap_or_else(|_| "shots".into());
                    let _ = std::fs::create_dir_all(&dir);
                    let out = std::path::Path::new(&dir).join(format!("{seq:02}-{page}.png"));
                    let _ = image::save_buffer(&out, &buf, w, h, image::ColorType::Rgba8);
                    // SEMANTIC_SHOT_EXIT=1：保存后优雅关闭窗口，避免强杀进程造成资源竞争
                    if std::env::var("SEMANTIC_SHOT_EXIT").is_ok() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                }
            }
        }

        // 运行时语言切换：id="locale" 控件变更 → 重载资源并置未就绪（下帧重布局）
        if let Some(sel) = self.state.selected("locale").map(str::to_owned)
            && sel != self.locale {
                self.locale = sel;
                self.i18n = I18n::load(&self.opts.root_dir, &self.locale);
                self.pages.clear();
                self.ready = false;
            }

        let Some(page_id) = self.order.get(self.current).cloned() else {
            ui.label(format!("当前页面不可用（第 {} 页）", self.current + 1));
            return;
        };
        let Some(model) = self.pages.get(&page_id) else {
            ui.label(format!("页面 {page_id} 未定义，请检查标记配置。"));
            return;
        };

        // 同步窗口标题与页面标题（首次进入与页面切换即生效，i18n 后为所选语言）
        if let Some(title) = &model.title {
            let base = self.opts.flow.app.name.clone();
            let want = format!("{base} - {title}");
            if self.last_title.as_deref() != Some(want.as_str()) {
                ctx.send_viewport_cmd(egui::ViewportCommand::Title(want.clone()));
                self.last_title = Some(want);
            }
        }

        let mut clicks: Vec<Click> = Vec::new();
        render::render_page(
            ui,
            model,
            &mut self.state,
            &self.phase,
            &self.last_progress,
            &self.theme,
            &mut self.assets,
            &self.opts.root_dir,
            anim_t,
            &mut |c| clicks.push(c),
        );
        for click in clicks {
            events::handle_click(self, &ctx, click);
        }

        // 非系统标题栏模式：客户区顶部自绘标题栏条（拖动 / 最小化 / 关闭，矢量图标）
        if self.no_titlebar {
            self.draw_titlebar(ui, &ctx);
        }
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        let c = theme::color(self.theme.bg);
        [
            c.r() as f32 / 255.0,
            c.g() as f32 / 255.0,
            c.b() as f32 / 255.0,
            1.0,
        ]
    }
}

impl InstallerApp {
    /// 自绘标题栏条（仅 --no-titlebar 模式）：覆盖在画布顶部 32px，
    /// 左侧应用名、右侧最小化 / 关闭按钮（矢量绘制，内联 SVG 语义，禁 emoji），
    /// 空白区域按住拖动窗口。
    fn draw_titlebar(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let canvas = ui.max_rect();
        let bar = egui::Rect::from_min_size(canvas.min, egui::vec2(canvas.width(), 32.0));
        let panel = theme::color(self.theme.panel);
        let border = theme::color(self.theme.border);
        let text_c = theme::color(self.theme.text);
        let painter = ui.painter();
        let transparent = self.titlebar_transparent;

        // 条背景 + 底部分隔线（透明模式省略，透出背景图）
        if !transparent {
            painter.rect_filled(bar, egui::CornerRadius::ZERO, panel);
            painter.line_segment(
                [
                    egui::pos2(bar.min.x, bar.max.y - 1.0),
                    egui::pos2(bar.max.x, bar.max.y - 1.0),
                ],
                egui::Stroke::new(1.0, border),
            );
        }

        // 左侧应用名（透明模式不再叠加阴影，保持干净；可读性由开发者用半透明图层自行控制）
        let title = format!("{} 安装程序", self.opts.flow.app.name);
        let font = egui::FontId::proportional(13.0);
        let pos = egui::pos2(bar.min.x + 12.0, bar.min.y + 16.0);
        painter.text(pos, egui::Align2::LEFT_CENTER, title, font, text_c);

        // 右侧按钮区：最小化（横线）+ 关闭（叉），矢量绘制
        let btn_w = 46.0;
        let min_rect =
            egui::Rect::from_min_size(egui::pos2(bar.max.x - btn_w * 2.0, bar.min.y), egui::vec2(btn_w, 32.0));
        let close_rect =
            egui::Rect::from_min_size(egui::pos2(bar.max.x - btn_w, bar.min.y), egui::vec2(btn_w, 32.0));

        let min_resp = ui.interact(min_rect, ui.id().with("titlebar-min"), egui::Sense::click());
        if min_resp.hovered() {
            // 浅色主题：标准标题栏按钮 hover 浅灰块；深色主题：半透明白
            let hover = if self.theme.name == "dark" {
                egui::Color32::from_rgba_unmultiplied(255, 255, 255, 30)
            } else {
                egui::Color32::from_rgb(0xE5, 0xE5, 0xE5)
            };
            painter.rect_filled(min_rect, egui::CornerRadius::ZERO, hover);
        }
        let (cx, cy) = (min_rect.center().x, min_rect.center().y);
        let min_stroke = egui::Stroke::new(1.5, text_c);
        painter.line_segment(
            [egui::pos2(cx - 6.0, cy), egui::pos2(cx + 6.0, cy)],
            min_stroke,
        );
        if min_resp.clicked() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
        }

        let close_resp = ui.interact(close_rect, ui.id().with("titlebar-close"), egui::Sense::click());
        if close_resp.hovered() {
            painter.rect_filled(close_rect, egui::CornerRadius::ZERO, theme::color(0xFFE81123));
        }
        let (cx, cy) = (close_rect.center().x, close_rect.center().y);
        let s = 6.5;
        let c = if close_resp.hovered() {
            egui::Color32::WHITE
        } else {
            text_c
        };
        let x_stroke = egui::Stroke::new(1.5, c);
        painter.line_segment(
            [egui::pos2(cx - s, cy - s), egui::pos2(cx + s, cy + s)],
            x_stroke,
        );
        painter.line_segment(
            [egui::pos2(cx - s, cy + s), egui::pos2(cx + s, cy - s)],
            x_stroke,
        );
        if close_resp.clicked() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }

        // 空白区拖动窗口
        let drag_rect = egui::Rect::from_min_max(bar.min, egui::pos2(min_rect.min.x, bar.max.y));
        let drag_resp = ui.interact(drag_rect, ui.id().with("titlebar-drag"), egui::Sense::drag());
        if drag_resp.dragged() {
            ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
        }
    }
}

/// 配置加载失败时的友好错误页（进程不静默崩溃）。
pub struct ErrorApp {
    pub title: String,
    pub lines: Vec<String>,
}

impl eframe::App for ErrorApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        ui.add_space(16.0);
        ui.heading(&self.title);
        ui.add_space(8.0);
        for line in &self.lines {
            ui.monospace(line);
        }
        ui.add_space(16.0);
        if ui.button("退出").clicked() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}

// 供 render/events 模块使用
pub(crate) fn phase_working(p: &Phase) -> bool {
    matches!(p, Phase::Working)
}

pub(crate) fn phase_failed(p: &Phase) -> Option<&str> {
    match p {
        Phase::Failed(m) => Some(m),
        _ => None,
    }
}
