//! 桌面入口保留关闭窗口直到所有受管计算进程已确认退出。

mod commands;
mod requests;
mod shutdown;

use tauri::{Manager, RunEvent, WindowEvent};
use tauri_plugin_dialog::{DialogExt, MessageDialogKind};
use workbench_core::Workbench;
use shutdown::ShutdownGate;

fn request_shutdown(app: &tauri::AppHandle) {
    if !app.state::<ShutdownGate>().begin() { return; }
    let handle = app.clone();
    let core = app.state::<Workbench>().inner().clone();
    tauri::async_runtime::spawn(async move {
        let outcome = tauri::async_runtime::spawn_blocking(move || core.shutdown()).await;
        let success = matches!(outcome, Ok(Ok(())));
        handle.state::<ShutdownGate>().complete(success);
        if success {
            handle.exit(0);
        } else {
            handle.dialog().message("未能完成退出检查，窗口已保留！请核实运行记录和系统进程状态。")
                .title("未能安全退出").kind(MessageDialogKind::Error).show(|_| {});
        }
    });
}

pub fn run() -> tauri::Result<()> {
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(Workbench::new())
        .manage(ShutdownGate::default());
    let app = commands::attach(builder)
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                if !window.state::<ShutdownGate>().approved() {
                    api.prevent_close();
                    request_shutdown(window.app_handle());
                }
            }
        })
        .build(tauri::generate_context!())?;
    app.run(|handle, event| {
        if let RunEvent::ExitRequested { api, .. } = event {
            if !handle.state::<ShutdownGate>().approved() {
                api.prevent_exit();
                request_shutdown(handle);
            }
        }
    });
    Ok(())
}
