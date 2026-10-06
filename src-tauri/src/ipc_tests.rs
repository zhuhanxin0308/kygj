//! MockRuntime只替代操作系统窗口；命令分发、SQLite与Python计算均真实执行。

use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use std::{path::PathBuf, time::{Duration, Instant}};
use tauri::{Manager, WebviewWindow, ipc::{CallbackFn, InvokeBody}, test::{MockRuntime, mock_builder, mock_context, noop_assets, get_ipc_response}, webview::InvokeRequest};
use workbench_core::{Workbench, types::*};

const INTEGRATION_TIMEOUT: Duration = Duration::from_secs(60);
const POLL_INTERVAL: Duration = Duration::from_millis(25);

pub(super) fn invoke(window: &WebviewWindow<MockRuntime>, command: &str, request: Value) -> Result<Value, Value> {
    get_ipc_response(window, InvokeRequest {
        cmd: command.into(), callback: CallbackFn(0), error: CallbackFn(1),
        url: (if cfg!(windows) { "http://tauri.localhost" } else { "tauri://localhost" }).parse().unwrap(),
        body: InvokeBody::Json(json!({ "request": request })),
        headers: Default::default(), invoke_key: tauri::test::INVOKE_KEY.into(),
    }).map(|body| body.deserialize::<Value>().unwrap())
}

pub(super) fn call<T: DeserializeOwned>(window: &WebviewWindow<MockRuntime>, command: &str, request: Value) -> T {
    let response = invoke(window, command, request).unwrap_or_else(|error| panic!("{command}未通过：{error}"));
    serde_json::from_value(response).unwrap()
}

fn python() -> PathBuf {
    std::env::var_os("GRAVITY_TEST_PYTHON").map(PathBuf::from).unwrap_or_else(|| {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf();
        root.join(if cfg!(windows) { ".venv/Scripts/python.exe" } else { ".venv/bin/python" })
    })
}

#[test]
fn registered_ipc_executes_and_exports_a_real_scientific_run() {
    let app = super::attach(mock_builder().manage(Workbench::new()))
        .build(mock_context(noop_assets())).unwrap();
    let window = tauri::WebviewWindowBuilder::new(&app, "main", Default::default()).build().unwrap();
    let directory = tempfile::tempdir().unwrap();
    let state: ProjectState = call(&window, "create_project", json!({ "parentDirectory": directory.path(), "name": "IPC科研验收" }));
    let identity = state.project.id.clone();
    let reopened: ProjectState = call(&window, "open_project", json!({ "directory": state.project.path }));
    assert_eq!(reopened.project.id, identity);
    let queried: ProjectState = call(&window, "get_project", json!({ "projectId": identity }));
    assert!(queried.models.is_empty());

    let config = json!({ "throatRadius": 1, "initialRadius": 10, "impactParameters": [0, 0.5, 2],
        "maxAffineParameter": 40, "sampleCount": 41, "relativeTolerance": 1e-10, "absoluteTolerance": 1e-12 });
    let model: ModelVersion = call(&window, "save_model", json!({ "projectId": identity, "label": "独立进程基准", "config": config }));
    let environment: EnvironmentInfo = call(&window, "probe_environment", json!({ "pythonExecutable": python() }));
    assert!(!environment.scipy_version.is_empty());
    let preflight: PreflightReport = call(&window, "prepare_run", json!({ "projectId": identity, "modelVersionId": model.id, "pythonExecutable": python() }));
    assert_eq!(preflight.status, PreflightStatus::Ready);
    let started: RunRecord = call(&window, "start_run", json!({ "projectId": identity, "preflightId": preflight.id }));
    let until = Instant::now() + INTEGRATION_TIMEOUT;
    let completed = loop {
        let run: RunRecord = call(&window, "get_run", json!({ "projectId": identity, "runId": started.id }));
        if run.state.terminal() { break run; }
        assert!(Instant::now() < until, "真实计算未在集成时限内结束");
        std::thread::sleep(POLL_INTERVAL);
    };
    assert_eq!(completed.state, RunState::Completed, "{:?}", completed.error);
    assert_eq!(completed.validation_status, ValidationStatus::Passed);
    assert_eq!(completed.result.as_ref().unwrap().trajectories[2].termination, "returned");
    super::verification_tests::assert_real_verification_history(&window, &identity, &completed);
    // 终态后的取消不得改写科研结果。
    let cancelled: RunRecord = call(&window, "cancel_run", json!({ "projectId": identity, "runId": completed.id }));
    assert_eq!(cancelled, completed);
    let exported: ExportedRun = call(&window, "export_run", json!({ "projectId": identity, "runId": completed.id, "destinationDirectory": directory.path() }));
    let payload: Value = serde_json::from_slice(&std::fs::read(&exported.path).unwrap()).unwrap();
    assert_eq!(payload["run"]["request"]["config"], serde_json::to_value(&completed.request.config).unwrap());
    assert_eq!(exported.sha256.len(), 64);
    assert!(invoke(&window, "start_run", json!({ "projectId": identity, "preflightId": preflight.id })).is_err());
    let after_duplicate: ProjectState = call(&window, "get_project", json!({ "projectId": identity }));
    assert_eq!(after_duplicate.runs.len(), 1, "重复提交不得产生额外运行");
    app.state::<Workbench>().shutdown().unwrap();
}

#[test]
fn ipc_rejects_extra_fields_and_unregistered_project_identity() {
    let app = super::attach(mock_builder().manage(Workbench::new()))
        .build(mock_context(noop_assets())).unwrap();
    let window = tauri::WebviewWindowBuilder::new(&app, "main", Default::default()).build().unwrap();
    assert!(invoke(&window, "get_project", json!({ "projectId": "unregistered", "directory": "untrusted" })).is_err());
    assert!(invoke(&window, "get_project", json!({ "projectId": "unregistered" })).is_err());
    app.state::<Workbench>().shutdown().unwrap();
}
