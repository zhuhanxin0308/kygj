//! IPC只做类型解码和后台转发，不在WebView线程执行磁盘或科学运算。

use tauri::State;
use workbench_core::{CoreError, CoreResult, Workbench, types::*};
use crate::requests::*;

// 正式窗口与无窗口IPC测试使用同一注册表，避免测试覆盖了生产中未注册的命令。
pub fn attach<R: tauri::Runtime>(builder: tauri::Builder<R>) -> tauri::Builder<R> {
    builder.invoke_handler(tauri::generate_handler![
        create_project, open_project, get_project, save_model, probe_environment,
        prepare_run, start_run, get_run, cancel_run, export_run,
    ])
}

async fn background<T: Send + 'static>(core: Workbench, action: impl FnOnce(Workbench) -> CoreResult<T> + Send + 'static) -> CoreResult<T> {
    tauri::async_runtime::spawn_blocking(move || action(core)).await
        .map_err(|_| CoreError::new("host_task_failed", "桌面后台操作未能完成，请检查项目状态！"))?
}

#[tauri::command]
pub async fn create_project(core: State<'_, Workbench>, request: CreateProject) -> CoreResult<ProjectState> {
    background(core.inner().clone(), move |core| core.create_project(&request.parent_directory, &request.name)).await
}

#[tauri::command]
pub async fn open_project(core: State<'_, Workbench>, request: OpenProject) -> CoreResult<ProjectState> {
    background(core.inner().clone(), move |core| core.open_project(&request.directory)).await
}

#[tauri::command]
pub async fn get_project(core: State<'_, Workbench>, request: ProjectIdentity) -> CoreResult<ProjectState> {
    background(core.inner().clone(), move |core| core.get_project(&request.project_id)).await
}

#[tauri::command]
pub async fn save_model(core: State<'_, Workbench>, request: SaveModel) -> CoreResult<ModelVersion> {
    background(core.inner().clone(), move |core| core.save_model(&request.project_id, &request.label, request.config)).await
}

#[tauri::command]
pub async fn probe_environment(core: State<'_, Workbench>, request: ProbeEnvironment) -> CoreResult<EnvironmentInfo> {
    background(core.inner().clone(), move |core| core.probe_environment(&request.python_executable)).await
}

#[tauri::command]
pub async fn prepare_run(core: State<'_, Workbench>, request: PrepareRun) -> CoreResult<PreflightReport> {
    background(core.inner().clone(), move |core| core.prepare_run(&request.project_id, &request.model_version_id, &request.python_executable)).await
}

#[tauri::command]
pub async fn start_run(core: State<'_, Workbench>, request: StartRun) -> CoreResult<RunRecord> {
    background(core.inner().clone(), move |core| core.start_run(&request.project_id, &request.preflight_id)).await
}

#[tauri::command]
pub async fn get_run(core: State<'_, Workbench>, request: RunIdentity) -> CoreResult<RunRecord> {
    background(core.inner().clone(), move |core| core.get_run(&request.project_id, &request.run_id)).await
}

#[tauri::command]
pub async fn cancel_run(core: State<'_, Workbench>, request: RunIdentity) -> CoreResult<RunRecord> {
    background(core.inner().clone(), move |core| core.cancel_run(&request.project_id, &request.run_id)).await
}

#[tauri::command]
pub async fn export_run(core: State<'_, Workbench>, request: ExportRun) -> CoreResult<ExportedRun> {
    background(core.inner().clone(), move |core| core.export_run(&request.project_id, &request.run_id, &request.destination_directory)).await
}

#[cfg(test)]
#[path = "ipc_tests.rs"]
mod tests;
