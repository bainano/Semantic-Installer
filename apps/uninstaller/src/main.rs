//! Demo 卸载器（薄二进制）：读取安装记录 → 运行共用壳（卸载模式）。
//!
//! 用法：
//! ```text
//! uninstaller [--record uninstall.json] [--app-id com.example.demo]
//!             [--markup uninstall.ui] [--theme light|dark] [--dry-run] [--log PATH]
//! ```
//!
//! 查找顺序：显式 --record → 本程序同目录 uninstall.json → 用户级副本。

use std::path::PathBuf;
use std::process::ExitCode;

use flow_engine::{dry_run_uninstall, find_record, InstallRecord};
use semantic_ui::parse;
use ui_adapter::{ErrorApp, InstallerApp, Mode, ShellOptions};

/// 内置默认卸载界面（确认 → 进度 → 完成），支持 --markup 覆盖。
const DEFAULT_MARKUP: &str = r##"<window title="卸载程序" width="720" height="460" theme="light">
  <page id="confirm" title="确认卸载">
    <column padding="36" gap="14">
      <text size="24" weight="bold">卸载确认</text>
      <text color="#666">卸载将删除以下内容，且无法恢复：</text>
      <text id="summary" color="#333" size="14">- 安装目录中的全部程序文件
- 开始菜单 / 桌面快捷方式
- 注册表卸载登记（Windows）
- 用户数据（由清单 keep_user_data 保护）</text>
      <spacer/>
      <row gap="8" align="end">
        <button action="uninstall" primary="true">卸载</button>
        <button action="cancel">取消</button>
      </row>
    </column>
  </page>
  <page id="progress" title="正在卸载">
    <column padding="36" gap="14">
      <text size="24" weight="bold">正在卸载…</text>
      <progress id="progress"/>
    </column>
  </page>
  <page id="finish" title="卸载完成">
    <column padding="36" gap="14">
      <text size="24" weight="bold">卸载完成</text>
      <text color="#666">感谢使用，再见。</text>
      <spacer/>
      <row gap="8" align="end">
        <button action="finish" primary="true">完成</button>
      </row>
    </column>
  </page>
</window>"##;

struct Args {
    record: Option<PathBuf>,
    app_id: Option<String>,
    markup: Option<PathBuf>,
    theme: Option<String>,
    dry_run: bool,
    log: Option<PathBuf>,
}

fn parse_args() -> Args {
    let mut args = Args {
        record: None,
        app_id: None,
        markup: None,
        theme: None,
        dry_run: false,
        log: None,
    };
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        let mut val = |name: &str| -> String {
            match it.next() {
                Some(v) => v,
                None => {
                    eprintln!("参数 {name} 缺少取值");
                    std::process::exit(2);
                }
            }
        };
        match a.as_str() {
            "--record" => args.record = Some(PathBuf::from(val("--record"))),
            "--app-id" => args.app_id = Some(val("--app-id")),
            "--markup" => args.markup = Some(PathBuf::from(val("--markup"))),
            "--theme" => args.theme = Some(val("--theme")),
            "--dry-run" => args.dry_run = true,
            "--log" => args.log = Some(PathBuf::from(val("--log"))),
            "--help" | "-h" => {
                println!(
                    "用法：uninstaller [--record uninstall.json] [--app-id ID] [--markup uninstall.ui] [--theme light|dark] [--dry-run] [--log PATH]"
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

fn main() -> ExitCode {
    let args = parse_args();
    init_log(args.log.as_ref());

    // 定位安装记录
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_default();
    let (record_path, record): (PathBuf, InstallRecord) =
        match find_record(args.record.as_deref(), &exe_dir, args.app_id.as_deref()) {
            Ok(found) => found,
            Err(e) => {
                eprintln!("{e}");
                return run_error_app("未找到安装记录", vec![e, "请从已安装程序的卸载入口启动本工具。".into()]);
            }
        };
    log::info!("已加载安装记录：{}", record_path.display());

    if args.dry_run {
        return match dry_run_uninstall(&record) {
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
        };
    }

    // 加载界面标记（内置默认或 --markup 覆盖）
    let (markup_src, root_dir) = match &args.markup {
        Some(path) => match std::fs::read_to_string(path) {
            Ok(s) => (
                s,
                path.parent().map(|p| p.to_path_buf()).unwrap_or_default(),
            ),
            Err(e) => {
                eprintln!("读取标记 {} 失败：{e}", path.display());
                return ExitCode::FAILURE;
            }
        },
        None => (DEFAULT_MARKUP.to_owned(), exe_dir.clone()),
    };
    let parsed = match parse(&markup_src) {
        Ok(p) => p,
        Err(e) => {
            eprintln!(
                "标记解析失败（第 {} 行，第 {} 列）：{}",
                e.span.line, e.span.col, e.message
            );
            return ExitCode::FAILURE;
        }
    };

    // 卸载器复用安装器壳：模式 = Uninstall，清单 = 记录中携带的 FlowSpec
    let flow = record.flow.clone();
    let (w, h) = ui_adapter::window_size_of(&flow, &parsed.root);
    let opts = ShellOptions {
        mode: Mode::Uninstall,
        flow,
        markup_root: parsed.root,
        root_dir,
        theme_override: args.theme.clone(),
        uninstaller_path: None,
        record: Some(record),
    };
    let app = InstallerApp::new(opts);

    let viewport = eframe::egui::ViewportBuilder::default()
        .with_inner_size([w, h])
        .with_resizable(false)
        .with_maximized(false)
        .with_maximize_button(false)
        .with_title("卸载程序");
    let native_options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    match eframe::run_native(
        "semantic-uninstaller",
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

fn init_log(path: Option<&PathBuf>) {
    let mut builder = env_logger::Builder::new();
    builder.filter_level(log::LevelFilter::Info);
    if let Some(p) = path
        && let Ok(f) = std::fs::OpenOptions::new().create(true).append(true).open(p)
    {
        builder.target(env_logger::Target::Pipe(Box::new(f)));
    }
    let _ = builder.try_init();
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
        "semantic-uninstaller-error",
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
