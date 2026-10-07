//! 通过两个真实SQLite连接固定状态提交时序，验证读取和完整性校验的快照一致性。

use super::*;

fn queued_run(store: &ProjectStore) -> RunRecord {
    let config = EllisConfig { throat_radius:1.0, initial_radius:10.0, impact_parameters:vec![0.0],
        max_affine_parameter:2.0, sample_count:3, relative_tolerance:1e-9, absolute_tolerance:1e-11 };
    let environment = EnvironmentInfo { python_executable:"python".into(), engine_version:"0.1.0".into(),
        python_version:"3.12".into(), numpy_version:"2".into(), scipy_version:"1".into(), engine_source_hash:"a".repeat(64) };
    let model = store.save_model("并发读取模型", config).unwrap();
    let preflight = store.prepare(&model.id, environment.clone(), ExecutionLimits::default(), vec![]).unwrap();
    store.consume_preflight(&preflight.id, &environment).unwrap()
}

#[test]
fn status_commit_between_payload_and_identity_reads_keeps_the_original_snapshot() {
    let directory = tempfile::tempdir().unwrap();
    let store = ProjectStore::create(directory.path(), "运行读取快照").unwrap();
    let run = queued_run(&store);
    store.transition(&run.id, RunState::Running, None, None).unwrap();
    // WAL允许写连接在读取者持有快照时提交，精确覆盖真实后台线程的竞态窗口。
    store.connection().unwrap().pragma_update(None, "journal_mode", "WAL").unwrap();
    let observed = store.read_snapshot(|connection| {
        let snapshot: RunRecord = read_record(connection, "SELECT record_json FROM runs WHERE id=?1", &run.id)?;
        let failed = store.transition(&run.id, RunState::Failed, None,
            Some(CoreError::new("output_limit_exceeded", "真实输出超过宿主限制")))?;
        assert_eq!(failed.state, RunState::Failed);
        assert_eq!(snapshot.state, RunState::Running);
        validate_run_record(connection, &snapshot, &run.project_id)?;
        Ok(snapshot)
    }).unwrap();
    assert_eq!(observed.state, RunState::Running);
    let persisted = store.run(&run.id).unwrap();
    assert_eq!(persisted.state, RunState::Failed);
    assert_eq!(persisted.error.unwrap().code, "output_limit_exceeded");
    assert_eq!(store.state().unwrap().runs[0].state, RunState::Failed);
    let exported = store.export_run(&run.id, directory.path()).unwrap();
    let bytes = std::fs::read(exported.path).unwrap();
    assert_eq!(exported.sha256, sha256_bytes(&bytes));
    let payload: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(payload["run"]["error"]["code"], "output_limit_exceeded");
}

fn result_fixture(run: &RunRecord) -> TraceResult {
    // 构造满足真实协议的径向预算耗尽结果，保留完整样本和验证诊断。
    let samples = (0..run.request.config.sample_count).map(|index| serde_json::json!({
        "affine":index as f64,"t":index as f64,"l":run.request.config.initial_radius-index as f64,
        "theta":std::f64::consts::FRAC_PI_2,"phi":0.0,"kt":1.0,"kl":-1.0,"kTheta":0.0,"kPhi":0.0
    })).collect::<Vec<_>>();
    serde_json::from_value(serde_json::json!({"protocolVersion":1,"requestId":run.request.request_id,"type":"result","config":run.request.config,
        "environment":{"pythonVersion":run.environment.python_version,"numpyVersion":run.environment.numpy_version,"scipyVersion":run.environment.scipy_version},
        "trajectories":[{"impactParameter":0.0,"termination":"budget_exhausted","samples":samples,"events":[],
        "diagnostics":{"maxEnergyError":0.0,"maxAngularMomentumError":0.0,"maxNullError":0.0,"maxEquatorialError":0.0,"turningRadiusError":null,"radialAnalyticError":null,"azimuthReferenceError":null},
        "validation":{"status":"inconclusive","checks":[{"name":"energy","actual":0.0,"threshold":1e-8,"passed":true}]}}]})).unwrap()
}

#[test]
fn completion_and_result_identity_become_visible_in_the_same_snapshot() {
    let directory = tempfile::tempdir().unwrap();
    let store = ProjectStore::create(directory.path(), "完成结果快照").unwrap();
    let run = queued_run(&store);
    store.transition(&run.id, RunState::Running, None, None).unwrap();
    store.connection().unwrap().pragma_update(None, "journal_mode", "WAL").unwrap();
    store.read_snapshot(|connection| {
        let snapshot: RunRecord = read_record(connection, "SELECT record_json FROM runs WHERE id=?1", &run.id)?;
        store.transition(&run.id, RunState::Completed, Some(result_fixture(&run)), None)?;
        // 运行尚未完成的旧快照不能看到随后提交的result_identities行。
        validate_run_record(connection, &snapshot, &run.project_id)?;
        assert_eq!(crate::verification_storage::source_identity(connection, &snapshot)?.result_origin, ResultIdentityOrigin::NoResult);
        Ok(())
    }).unwrap();
    let complete = store.run(&run.id).unwrap();
    assert_eq!(complete.state, RunState::Completed);
    let identity = crate::verification_storage::source_identity(&store.connection().unwrap(), &complete).unwrap();
    assert_eq!(identity.result_origin, ResultIdentityOrigin::CapturedAtCompletion);
    assert_eq!(identity.result_hash, Some(crate::verification::typed_hash(complete.result.as_ref().unwrap()).unwrap()));
    assert_eq!(store.state().unwrap().runs[0], complete);
}
