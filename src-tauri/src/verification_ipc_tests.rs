//! 验证接口必须穿过正式命令注册表，不能以直接调用核心代替桌面边界验收。

use serde_json::{Value, json};
use tauri::{Manager, WebviewWindow, test::{MockRuntime, mock_builder, mock_context, noop_assets}};
use workbench_core::{Workbench, types::{ProjectState, ProjectSummary, RunRecord}};
use super::tests::{call, invoke};

#[test]
fn verification_commands_expose_real_project_rules_and_reject_forged_identity() {
    let app = super::attach(mock_builder().manage(Workbench::new()))
        .build(mock_context(noop_assets())).unwrap();
    let window = tauri::WebviewWindowBuilder::new(&app, "main", Default::default()).build().unwrap();
    let parent = tempfile::tempdir().unwrap();
    let project: ProjectState = call(&window, "create_project", json!({
        "parentDirectory": parent.path(), "name": "独立验证IPC验收",
    }));
    let rules: Value = call(&window, "list_verification_rules", json!({
        "projectId": project.project.id, "offset": 0, "limit": 20,
    }));
    assert!(!rules["rules"].as_array().expect("规则分页必须是数组").is_empty());
    // 无效身份必须由真实核心拒绝；如果命令没有注册，返回值不会是结构化领域错误。
    let error = invoke(&window, "get_verification_record", json!({
        "projectId": project.project.id, "recordId": "不存在的验证记录",
    })).unwrap_err();
    assert!(error["code"].is_string(), "缺少明确的领域错误：{error}");
    assert!(invoke(&window, "list_verification_rules", json!({
        "projectId": project.project.id, "offset": 0, "limit": 20, "execute": true,
    })).is_err(), "查询接口不能夹带执行参数");
    app.state::<Workbench>().shutdown().unwrap();
}

/// 在真实Python完成的同一运行上检验阈值修订、幂等和历史不可变，避免再启动重复计算。
pub(super) fn assert_real_verification_history(window: &WebviewWindow<MockRuntime>, project_id: &str, run: &RunRecord) {
    let page: Value = call(window, "list_verification_rules", json!({"projectId":project_id,"offset":0,"limit":20}));
    let base = &page["rules"][0];
    let before: Value = call(window, "get_run_verification_state", json!({
        "projectId":project_id,"runId":run.id,"ruleVersionId":base["id"],
    }));
    assert_eq!(before["conclusion"], "not_run");
    // 收紧真实非零残差门槛，必须得到数值失败，不能沿用原引擎的通过布尔值。
    let thresholds: Vec<Value> = base["checks"].as_array().unwrap().iter().map(|check| json!({
        "metricId":check["metricId"],
        "threshold":if check["metricId"] == "null_error" { json!(0) } else { check["threshold"].clone() },
        "basis":"IPC验收：零容差用于证明执行完成与数值未通过相互独立。",
    })).collect();
    let revised: Value = call(window, "save_verification_rule_version", json!({"projectId":project_id,"draft":{
        "baseVersionId":base["id"],"title":"零条件严格复检","changeReason":"验证阈值敏感性，保留原始检查历史。","thresholds":thresholds,
    }}));
    assert_ne!(base["id"], revised["id"]);
    assert_eq!(revised["parentVersionId"], base["id"]);
    assert_eq!(revised["builtin"], false);
    let request = json!({"projectId":project_id,"request":{
        "runId":run.id,"ruleVersionId":revised["id"],"clientRequestId":"ipc-real-reassessment-1","previousRecordId":null,
    }});
    let first: Value = call(window, "execute_verification", request.clone());
    assert_eq!(first["executionStatus"], "completed");
    assert_eq!(first["conclusion"], "failed");
    assert_eq!(first["source"]["resultOrigin"], "captured_at_completion");
    assert_eq!(first, call::<Value>(window, "execute_verification", request));
    let second: Value = call(window, "execute_verification", json!({"projectId":project_id,"request":{
        "runId":run.id,"ruleVersionId":revised["id"],"clientRequestId":"ipc-real-reassessment-2","previousRecordId":first["id"],
    }}));
    assert_ne!(first["id"], second["id"]);
    assert_eq!(second["previousRecordId"], first["id"]);
    let persisted: Value = call(window, "get_verification_record", json!({"projectId":project_id,"recordId":first["id"]}));
    assert_eq!(persisted, first);
    let history: Value = call(window, "list_verification_records", json!({"projectId":project_id,"runId":run.id,"offset":0,"limit":1}));
    assert_eq!(history["total"], 2);
    assert_eq!(history["nextOffset"], 1);
    assert_eq!(history["records"][0], first);
    let unchanged: RunRecord = call(window, "get_run", json!({"projectId":project_id,"runId":run.id}));
    assert_eq!(&unchanged, run, "独立验证不得改写原运行的科学检查或冻结输入");
}

#[test]
fn migration_ipc_requires_reviewed_plan_and_preserves_a_readable_version_one_backup() {
    let app = super::attach(mock_builder().manage(Workbench::new()))
        .build(mock_context(noop_assets())).unwrap();
    let window = tauri::WebviewWindowBuilder::new(&app, "main", Default::default()).build().unwrap();
    let parent = tempfile::tempdir().unwrap();
    let path = parent.path().join("待审阅旧项目");
    std::fs::create_dir_all(path.join(".gravity")).unwrap();
    let database = path.join(".gravity/workbench.sqlite");
    let connection = rusqlite::Connection::open(&database).unwrap();
    // 使用固定旧版契约夹具，不能以当前生产建库逻辑伪造一次兼容性迁移。
    connection.execute_batch(include_str!("../../crates/workbench-core/tests/fixtures/project-v1.sql")).unwrap();
    let summary = ProjectSummary { id: "e1daeb9d-730c-4d1b-8909-9efac31476cd".into(), name: "待审阅旧项目".into(),
        path: path.to_string_lossy().into(), created_at: workbench_core::storage::timestamp(), schema_version: 1 };
    connection.execute("INSERT INTO projects(id,record_json) VALUES (?1,?2)", (&summary.id, serde_json::to_string(&summary).unwrap())).unwrap();
    let before: String = connection.query_row("SELECT record_json FROM projects", [], |row| row.get(0)).unwrap();
    let error = invoke(&window, "open_project", json!({"directory":path})).unwrap_err();
    assert_eq!(error["code"], "migration_required");
    let plan: Value = call(&window, "prepare_project_migration", json!({"directory":path}));
    assert_eq!(plan["requiresConfirmation"], true);
    assert_eq!(plan["projectId"], summary.id);
    assert!(!std::path::Path::new(plan["backupPath"].as_str().unwrap()).exists(), "预览不得提前写备份");
    assert_eq!(connection.query_row("SELECT record_json FROM projects", [], |row| row.get::<_, String>(0)).unwrap(), before);
    assert!(invoke(&window, "apply_project_migration", json!({"directory":path,"planId":"未经审阅的计划"})).is_err());
    let receipt: Value = call(&window, "apply_project_migration", json!({"directory":path,"planId":plan["id"]}));
    assert_eq!(receipt["planId"], plan["id"]);
    assert_eq!(receipt["backupPath"], plan["backupPath"]);
    let backup = rusqlite::Connection::open(receipt["backupPath"].as_str().unwrap()).unwrap();
    assert_eq!(backup.pragma_query_value(None, "user_version", |row| row.get::<_, u32>(0)).unwrap(), 1);
    assert_eq!(backup.query_row("SELECT record_json FROM projects", [], |row| row.get::<_, String>(0)).unwrap(), before);
    let opened: ProjectState = call(&window, "open_project", json!({"directory":path}));
    assert_eq!(opened.project.id, summary.id);
    assert_eq!(opened.project.schema_version, 2);
    assert!(invoke(&window, "apply_project_migration", json!({"directory":path,"planId":plan["id"]})).is_err());
    app.state::<Workbench>().shutdown().unwrap();
}
