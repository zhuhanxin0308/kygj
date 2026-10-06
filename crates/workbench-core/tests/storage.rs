//! 通过真实临时目录和SQLite验证追加版本、事务提交及预检只消费一次。

use workbench_core::{storage::ProjectStore, types::*};

fn config() -> EllisConfig { EllisConfig { throat_radius:1.0,initial_radius:10.0,impact_parameters:vec![0.0],max_affine_parameter:2.0,sample_count:3,relative_tolerance:1e-9,absolute_tolerance:1e-11 } }
fn env() -> EnvironmentInfo { EnvironmentInfo { python_executable:"python".into(),engine_version:"0.1.0".into(),python_version:"3.12".into(),numpy_version:"2".into(),scipy_version:"1".into(),engine_source_hash:"a".repeat(64) } }

#[test]
fn create_open_versions_and_export_are_real_and_nonoverwriting() {
    let dir = tempfile::tempdir().unwrap();
    let store = ProjectStore::create(dir.path(), "真实研究").unwrap();
    assert!(ProjectStore::create(dir.path(), "真实研究").is_err());
    let first = store.save_model("版本一", config()).unwrap();
    let mut changed = config(); changed.throat_radius = 2.0;
    let second = store.save_model("版本二", changed).unwrap();
    assert_ne!(first.content_hash, second.content_hash);
    let opened = ProjectStore::open(store.directory()).unwrap();
    let state = opened.state().unwrap();
    assert_eq!(state.models.len(), 2); assert_eq!(state.models[0].id, second.id);
    assert_eq!(state.project.id, store.summary().unwrap().id);
    assert_eq!(opened.model(&first.id).unwrap().config, config());
}

#[test]
fn preflight_consumption_and_snapshot_are_transactional() {
    let dir = tempfile::tempdir().unwrap(); let store = ProjectStore::create(dir.path(), "事务测试").unwrap();
    let model = store.save_model("模型", config()).unwrap();
    let report = store.prepare(&model.id, env(), ExecutionLimits::default(), vec![]).unwrap();
    let run = store.consume_preflight(&report.id, &env()).unwrap();
    assert_eq!(run.state, RunState::Queued); assert_eq!(run.validation_status, ValidationStatus::NotRun);
    assert_eq!(run.request.config, model.config); assert!(run.result.is_none());
    assert!(store.consume_preflight(&report.id, &env()).is_err());
    assert_eq!(store.state().unwrap().runs.len(), 1);
    let report2 = store.prepare(&model.id, env(), ExecutionLimits::default(), vec![]).unwrap();
    let mut changed_env = env(); changed_env.engine_source_hash = "b".repeat(64);
    assert!(store.consume_preflight(&report2.id, &changed_env).is_err());
    assert_eq!(store.state().unwrap().runs.len(), 1);
    let exported = store.export_run(&run.id, dir.path()).unwrap();
    assert!(std::path::Path::new(&exported.path).is_file()); assert_eq!(exported.sha256.len(), 64);
    assert!(store.export_run(&run.id, dir.path()).is_err());
}

#[test]
fn immutable_database_rows_and_state_machine_reject_invalid_updates() {
    let dir = tempfile::tempdir().unwrap(); let store = ProjectStore::create(dir.path(), "状态测试").unwrap();
    let model = store.save_model("模型", config()).unwrap();
    let report = store.prepare(&model.id, env(), ExecutionLimits::default(), vec![]).unwrap();
    let run = store.consume_preflight(&report.id, &env()).unwrap();
    assert!(store.transition(&run.id, RunState::Completed, None, None).is_err());
    store.transition(&run.id, RunState::Running, None, None).unwrap();
    store.transition(&run.id, RunState::Cancelling, None, None).unwrap();
    store.transition(&run.id, RunState::Cancelled, None, None).unwrap();
    assert!(store.transition(&run.id, RunState::Running, None, None).is_err());
    let conn = rusqlite::Connection::open(store.directory().join(".gravity/workbench.sqlite")).unwrap();
    assert!(conn.execute("UPDATE models SET label='被覆盖'", []).is_err());
    assert!(conn.execute("UPDATE runs SET request_json='{}'", []).is_err());
    assert!(conn.execute("DELETE FROM models", []).is_err());
    let finished = store.run(&run.id).unwrap(); assert!(finished.finished_at.is_some());
}

#[test]
fn blocked_preflight_and_corrupt_or_unrelated_directory_are_rejected() {
    let dir = tempfile::tempdir().unwrap(); assert!(ProjectStore::open(dir.path()).is_err());
    let store = ProjectStore::create(dir.path(), "预检测试").unwrap();
    let model = store.save_model("模型", config()).unwrap();
    let report = store.prepare(&model.id, env(), ExecutionLimits::default(), vec![workbench_core::CoreError::new("disk_full", "磁盘空间不足")]).unwrap();
    assert_eq!(report.status, PreflightStatus::Blocked);
    assert!(store.consume_preflight(&report.id, &env()).is_err());
    assert!(store.state().unwrap().runs.is_empty());
}

#[test]
fn rejects_tampered_model_identity_and_hash_on_every_read_path() {
    for field in ["projectId", "contentHash"] {
        let dir = tempfile::tempdir().unwrap(); let store = ProjectStore::create(dir.path(), "损坏模型").unwrap();
        let model = store.save_model("模型", config()).unwrap();
        let connection = rusqlite::Connection::open(store.directory().join(".gravity/workbench.sqlite")).unwrap();
        // 模拟磁盘上的外部篡改，而不是通过正常业务接口绕过不可变记录。
        connection.execute_batch("DROP TRIGGER models_no_update").unwrap();
        let mut json = serde_json::to_value(&model).unwrap(); json[field] = "被篡改".into();
        connection.execute("UPDATE models SET record_json=?1 WHERE id=?2", (json.to_string(), &model.id)).unwrap();
        assert!(store.model(&model.id).is_err()); assert!(store.state().is_err());
    }
}

fn result_fixture(run: &RunRecord) -> TraceResult {
    let samples = (0..run.request.config.sample_count).map(|i| serde_json::json!({
        "affine":i as f64,"t":i as f64,"l":10.0-i as f64,"theta":std::f64::consts::FRAC_PI_2,"phi":0.0,"kt":1.0,"kl":-1.0,"kTheta":0.0,"kPhi":0.0
    })).collect::<Vec<_>>();
    serde_json::from_value(serde_json::json!({"protocolVersion":1,"requestId":run.request.request_id,"type":"result","config":run.request.config,
        "environment":{"pythonVersion":run.environment.python_version,"numpyVersion":run.environment.numpy_version,"scipyVersion":run.environment.scipy_version},
        "trajectories":[{"impactParameter":0.0,"termination":"budget_exhausted","samples":samples,"events":[],
        "diagnostics":{"maxEnergyError":0.0,"maxAngularMomentumError":0.0,"maxNullError":0.0,"maxEquatorialError":0.0,"turningRadiusError":null,"radialAnalyticError":null,"azimuthReferenceError":null},
        "validation":{"status":"inconclusive","checks":[{"name":"energy","actual":0.0,"threshold":1e-8,"passed":true}]}}]})).unwrap()
}

#[test]
fn rejects_tampered_frozen_request_and_result_identity_before_export() {
    for corrupt_result in [false, true] {
        let dir = tempfile::tempdir().unwrap(); let store = ProjectStore::create(dir.path(), "损坏运行").unwrap();
        let model = store.save_model("模型", config()).unwrap();
        let report = store.prepare(&model.id, env(), ExecutionLimits::default(), vec![]).unwrap();
        let run = store.consume_preflight(&report.id, &env()).unwrap();
        store.transition(&run.id, RunState::Running, None, None).unwrap();
        let complete = store.transition(&run.id, RunState::Completed, Some(result_fixture(&run)), None).unwrap();
        let mut json = serde_json::to_value(complete).unwrap();
        if corrupt_result { json["result"]["requestId"] = "wrong-result".into(); }
        else { json["request"]["requestId"] = "wrong-request".into(); }
        let connection = rusqlite::Connection::open(store.directory().join(".gravity/workbench.sqlite")).unwrap();
        connection.execute_batch("DROP TRIGGER runs_frozen; DROP TRIGGER runs_transitions;").unwrap();
        connection.execute("UPDATE runs SET record_json=?1 WHERE id=?2", (json.to_string(), &run.id)).unwrap();
        assert!(store.run(&run.id).is_err()); assert!(store.state().is_err());
        assert!(store.export_run(&run.id, dir.path()).is_err());
    }
}
