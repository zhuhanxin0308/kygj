//! IPC只做类型解码和后台转发，不在WebView线程执行磁盘或科学运算。

use tauri::State;
use workbench_core::{CoreError, CoreResult, Workbench, types::*};
use crate::requests::*;

// 正式窗口与无窗口IPC测试使用同一注册表，避免测试覆盖了生产中未注册的命令。
pub fn attach<R: tauri::Runtime>(builder: tauri::Builder<R>) -> tauri::Builder<R> {
    builder.invoke_handler(tauri::generate_handler![
        create_project, open_project, get_project, save_model, probe_environment,
        prepare_run, start_run, get_run, cancel_run, export_run,
        list_verification_rules, save_verification_rule_version, get_run_verification_state,
        execute_verification, list_verification_records, get_verification_record,
        prepare_project_migration, apply_project_migration,
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

// 验证与迁移沿用受控后台通道，避免哈希、SQLite备份及评估阻塞窗口线程。
#[tauri::command]
pub async fn list_verification_rules(core: State<'_, Workbench>, request: ListVerificationRules) -> CoreResult<VerificationRulePage> {
    background(core.inner().clone(), move |core| core.list_verification_rules(&request.project_id, request.offset, request.limit)).await
}

#[tauri::command]
pub async fn save_verification_rule_version(core: State<'_, Workbench>, request: SaveVerificationRule) -> CoreResult<VerificationRuleVersion> {
    background(core.inner().clone(), move |core| core.save_verification_rule_version(&request.project_id, request.draft)).await
}

#[tauri::command]
pub async fn get_run_verification_state(core: State<'_, Workbench>, request: RunVerificationIdentity) -> CoreResult<RunVerificationState> {
    background(core.inner().clone(), move |core| core.get_run_verification_state(&request.project_id, &request.run_id, &request.rule_version_id)).await
}

#[tauri::command]
pub async fn execute_verification(core: State<'_, Workbench>, request: ExecuteVerificationRequest) -> CoreResult<VerificationRecord> {
    background(core.inner().clone(), move |core| core.execute_verification(&request.project_id, request.request)).await
}

#[tauri::command]
pub async fn list_verification_records(core: State<'_, Workbench>, request: ListVerificationRecords) -> CoreResult<VerificationRecordPage> {
    background(core.inner().clone(), move |core| core.list_verification_records(&request.project_id, &request.run_id, request.offset, request.limit)).await
}

#[tauri::command]
pub async fn get_verification_record(core: State<'_, Workbench>, request: VerificationRecordIdentity) -> CoreResult<VerificationRecord> {
    background(core.inner().clone(), move |core| core.get_verification_record(&request.project_id, &request.record_id)).await
}

#[tauri::command]
pub async fn prepare_project_migration(core: State<'_, Workbench>, request: OpenProject) -> CoreResult<ProjectMigrationPlan> {
    background(core.inner().clone(), move |core| core.prepare_project_migration(&request.directory)).await
}

#[tauri::command]
pub async fn apply_project_migration(core: State<'_, Workbench>, request: ApplyProjectMigration) -> CoreResult<ProjectMigrationReceipt> {
    background(core.inner().clone(), move |core| core.apply_project_migration(&request.directory, &request.plan_id)).await
}

#[cfg(test)]
#[path = "ipc_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "verification_ipc_tests.rs"]
mod verification_tests;
