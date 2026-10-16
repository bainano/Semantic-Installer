//! 点击事件 → 流程引擎动作分发。

use eframe::egui;
use semantic_ui::widget::Click;

use crate::app::{phase_working, InstallerApp, Mode};

fn open_url(url: &str) {
    let result = if cfg!(windows) {
        std::process::Command::new("cmd")
            .args(["/c", "start", "", url])
            .spawn()
            .map(|_| ())
    } else if cfg!(target_os = "macos") {
        std::process::Command::new("open").arg(url).spawn().map(|_| ())
    } else {
        std::process::Command::new("xdg-open").arg(url).spawn().map(|_| ())
    };
    if let Err(e) = result {
        log::warn!("打开链接失败：{e}");
    }
}

/// 分发一次点击。goto 优先于 action 处理；自定义动作记日志（二次开发扩展点）。
pub fn handle_click(app: &mut InstallerApp, ctx: &egui::Context, click: Click) {
    if let Some(url) = click.url
        && !url.is_empty() {
            open_url(&url);
            return;
        }

    let action = click.action.as_deref().unwrap_or("");
    match action {
        "next" => {
            if app.current + 1 < app.order.len() {
                app.current += 1;
            }
        }
        "back" => {
            if app.current > 0 {
                app.current -= 1;
            }
        }
        "cancel" => {
            if phase_working(&app.phase) {
                // 工作期间：请求取消，由工作线程回滚；窗口保持打开直到回滚完成
                if let Some(w) = &app.worker {
                    w.cancel.store(true, std::sync::atomic::Ordering::Relaxed);
                }
            } else {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
        "install" => {
            if app.opts.mode == Mode::Install {
                app.start_install();
            }
        }
        "uninstall" => {
            if app.opts.mode == Mode::Uninstall {
                app.start_uninstall();
            }
        }
        "finish" => {
            if app.opts.mode == Mode::Uninstall {
                app.schedule_self_cleanup();
            }
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        other => {
            // 自定义动作扩展点：默认仅记录日志
            log::info!("自定义动作（未绑定处理器）：{other}");
        }
    }

    if let Some(goto) = &click.goto {
        app.goto(goto);
    }

    // 同步窗口标题与页面标题
    if let Some(model) = app.current_model()
        && let Some(title) = &model.title {
            let base = app.opts.flow.app.name.clone();
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(format!(
                "{base} - {title}"
            )));
        }
}
