//! Demo 安装器（薄二进制：解析参数 → 加载清单与标记 → 运行共享壳）。
//!
//! 用法：
//! ```text
//! installer --manifest install.toml --markup app.ui [--theme light|dark]
//!           [--lang zh-CN|en-US]                  # 覆盖语言（缺省：<window locale> > 系统 > zh-CN）
//!           [--no-titlebar]                       # 非系统标题栏模式（客户区自绘标题栏条）
//!           [--uninstaller path\to\uninstaller.exe]
//!           [--record uninstall.json]          # 等价于 --mode uninstall
//!           [--dry-run install|uninstall]      # 无 GUI 全流程模拟
//!           [--log install.log]
//! ```

use std::path::PathBuf;
use std::process::ExitCode;

use flow_engine::{dry_run_install, dry_run_uninstall, find_record, FlowSpec};
use ui_adapter::{ErrorApp, InstallerApp, Mode, ShellOptions};

struct Args {
    manifest: PathBuf,
    markup: PathBuf,
    theme: Option<String>,
    lang: Option<String>,
    no_titlebar: bool,
    uninstaller: Option<PathBuf>,
    record: Option<PathBuf>,
    app_id: Option<String>,
    dry_run: Option<String>,
    log: Option<PathBuf>,
}

fn parse_args() -> Args {
    let mut args = Args {
        manifest: PathBuf::from("install.toml"),
        markup: PathBuf::from("app.ui"),
        theme: None,
        lang: None,
        no_titlebar: false,
        uninstaller: None,
        record: None,
        app_id: None,
        dry_run: None,
        log: None,
    };
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        let mut val = |name: &str| -> Option<String> {
            match it.next() {
                Some(v) => Some(v),
                None => {
                    eprintln!("参数 {name} 缺少取值");
                    std::process::exit(2);
                }
            }
        };
        match a.as_str() {
            "--manifest" => args.manifest = PathBuf::from(val("--manifest").unwrap()),
            "--markup" => args.markup = PathBuf::from(val("--markup").unwrap()),
            "--theme" => args.theme = Some(val("--theme").unwrap()),
            "--lang" => args.lang = Some(val("--lang").unwrap()),
            "--no-titlebar" => args.no_titlebar = true,
            "--uninstaller" => args.uninstaller = Some(PathBuf::from(val("--uninstaller").unwrap())),
            "--record" => args.record = Some(PathBuf::from(val("--record").unwrap())),
            "--app-id" => args.app_id = Some(val("--app-id").unwrap()),
            "--dry-run" => args.dry_run = Some(val("--dry-run").unwrap()),
            "--log" => args.log = Some(PathBuf::from(val("--log").unwrap())),
            "--help" | "-h" => {
                println!(
                    "用法：installer [--manifest PATH] [--markup PATH] [--theme light|dark]\n  \
                     [--lang zh-CN|en-US] [--no-titlebar] [--uninstaller PATH] [--record uninstall.json]\n  \
                     [--dry-run install|uninstall] [--log PATH]"
                );
                std::process::exit(0);
            }
            other => {
                eprintln!("未知参数：{other}");
                std::process::exit(2);
            }
        }
    }
    args
}

fn init_log(path: Option<&PathBuf>) {
    let mut builder = env_logger::Builder::new();
    builder.filter_level(log::LevelFilter::Info);
    if let Some(p) = path
        && let Ok(f) = std::fs::OpenOptions::new().create(true).append(true).open(p) {
            builder.target(env_logger::Target::Pipe(Box::new(f)));
        }
    let _ = builder.try_init();
}

fn run_gui(mode: Mode, args: &Args, flow: &FlowSpec, record: Option<flow_engine::InstallRecord>) -> ExitCode {
    // 解析语义标记
    let markup_src = match std::fs::read_to_string(&args.markup) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("读取标记 {} 失败：{e}", args.markup.display());
            return ExitCode::FAILURE;
        }
    };
    let parsed = match semantic_ui::parse(&markup_src) {
        Ok(p) => p,
        Err(e) => {
            eprintln!(
                "标记解析失败（第 {} 行，第 {} 列）：{}",
                e.span.line, e.span.col, e.message
            );
            return ExitCode::FAILURE;
        }
    };
    for w in &parsed.warnings {
        eprintln!("[标记警告] {w}");
    }
    let root_dir = args
        .markup
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));

    let (w, h) = ui_adapter::window_size_of(flow, &parsed.root);
    let title = format!("{} 安装程序", flow.app.name);
    let opts = ShellOptions {
        mode,
        flow: flow.clone(),
        markup_root: parsed.root,
        root_dir,
        theme_override: args.theme.clone(),
        uninstaller_path: args.uninstaller.clone(),
        record,
        lang: args.lang.clone(),
        no_titlebar: args.no_titlebar,
    };
    let app = InstallerApp::new(opts);

    let mut viewport = eframe::egui::ViewportBuilder::default()
        .with_inner_size([w, h])
        .with_resizable(false)
        .with_maximized(false)
        .with_maximize_button(false)
        .with_title(title);
    if args.no_titlebar {
        // 非系统标题栏模式：去掉原生标题栏与边框，标题栏条由引擎在客户区自绘
        viewport = viewport.with_decorations(false);
    }
    let native_options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    match eframe::run_native(
        "semantic-installer",
        native_options,
        Box::new(move |_cc| Ok(Box::new(app))),
    ) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("GUI 启动失败：{e}");
            ExitCode::FAILURE
        }
    }
}

fn run_error_app(title: &str, lines: Vec<String>) -> ExitCode {
    let app = ErrorApp {
        title: title.to_owned(),
        lines,
    };
    let viewport = eframe::egui::ViewportBuilder::default()
        .with_inner_size([720.0, 360.0])
        .with_resizable(false)
        .with_maximize_button(false)
        .with_title(title.to_owned());
    let native_options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    match eframe::run_native(
        "semantic-installer-error",
        native_options,
        Box::new(move |_cc| Ok(Box::new(app))),
    ) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("GUI 启动失败：{e}");
            ExitCode::FAILURE
        }
    }
}

fn main() -> ExitCode {
    let args = parse_args();
    init_log(args.log.as_ref());

    // 无 GUI 全流程模拟
    if let Some(mode) = &args.dry_run {
        match mode.as_str() {
            "install" => {
                let flow = match FlowSpec::from_toml_file(&args.manifest) {
                    Ok(f) => f,
                    Err(e) => {
                        eprintln!("{e}");
                        return ExitCode::FAILURE;
                    }
                };
                let root = args.manifest.parent().unwrap_or_else(|| std::path::Path::new("."));
                let inputs = flow_engine::InstallInputs {
                    install_dir_override: None,
                    create_shortcut: true,
                    uninstaller_path: args.uninstaller.clone(),
                    locale: args.lang.clone(),
                };
                match dry_run_install(&flow, root, &inputs) {
                    Ok(lines) => {
                        for l in lines {
                            println!("{l}");
                        }
                        ExitCode::SUCCESS
                    }
                    Err(e) => {
                        eprintln!("{e}");
                        ExitCode::FAILURE
                    }
                }
            }
            "uninstall" => {
                let exe_dir = std::env::current_exe()
                    .ok()
                    .and_then(|p| p.parent().map(|d| d.to_path_buf()))
                    .unwrap_or_default();
                match find_record(args.record.as_deref(), &exe_dir, args.app_id.as_deref()) {
                    Ok((_, record)) => match dry_run_uninstall(&record) {
                        Ok(lines) => {
                            for l in lines {
                                println!("{l}");
                            }
                            ExitCode::SUCCESS
                        }
                        Err(e) => {
                            eprintln!("{e}");
                            ExitCode::FAILURE
                        }
                    },
                    Err(e) => {
                        eprintln!("{e}");
                        ExitCode::FAILURE
                    }
                }
            }
            other => {
                eprintln!("--dry-run 仅支持 install|uninstall，收到：{other}");
                ExitCode::FAILURE
            }
        }
    } else {
        // GUI 模式
        let flow = match FlowSpec::from_toml_file(&args.manifest) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("{e}");
                return run_error_app("清单加载失败", vec![e, "请检查 install.toml 路径与格式。".into()]);
            }
        };

        // 卸载模式（installer --record uninstall.json）
        if let Some(record_path) = &args.record {
            let exe_dir = std::env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(|d| d.to_path_buf()))
                .unwrap_or_default();
            match find_record(Some(record_path), &exe_dir, args.app_id.as_deref()) {
                Ok((_, record)) => run_gui(Mode::Uninstall, &args, &flow, Some(record)),
                Err(e) => {
                    eprintln!("{e}");
                    run_error_app(
                        "无法加载卸载记录",
                        vec![e, "请确认 uninstall.json 存在且格式正确。".into()],
                    )
                }
            }
        } else {
            run_gui(Mode::Install, &args, &flow, None)
        }
    }
}
