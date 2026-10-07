//! 独立验证必须依据实际测量值、保存不可变历史，并保持运行快照原样。
use workbench_core::{storage::ProjectStore, types::*, Workbench};

fn setup() -> (tempfile::TempDir, Workbench, ProjectState, ProjectStore, RunRecord) {
    setup_config(EllisConfig { throat_radius:1.0,initial_radius:10.0,impact_parameters:vec![0.0],max_affine_parameter:20.0,sample_count:3,relative_tolerance:1e-10,absolute_tolerance:1e-12 })
}

fn setup_config(config: EllisConfig) -> (tempfile::TempDir, Workbench, ProjectState, ProjectStore, RunRecord) {
    let dir = tempfile::tempdir().unwrap(); let core = Workbench::new();
    let project = core.create_project(dir.path(), "独立验证").unwrap();
    let store = ProjectStore::open(std::path::Path::new(&project.project.path)).unwrap();
    let model = store.save_model("径向模型", config).unwrap();
    let env = EnvironmentInfo { python_executable:"python".into(),engine_version:"0.1.0".into(),python_version:"3.12".into(),numpy_version:"2".into(),scipy_version:"1".into(),engine_source_hash:"a".repeat(64) };
    let preflight = store.prepare(&model.id, env.clone(), ExecutionLimits::default(), vec![]).unwrap();
    let run = store.consume_preflight(&preflight.id, &env).unwrap();
    (dir, core, project, store, run)
}

fn result_fixture(run: &RunRecord, reported_error: f64) -> TraceResult {
    let radius = run.request.config.initial_radius;
    let samples: Vec<_> = [0.0,radius,2.0*radius].iter().map(|t| serde_json::json!({"affine":t,"t":t,"l":radius-t,"theta":std::f64::consts::FRAC_PI_2,"phi":0.0,"kt":1.0,"kl":-1.0,"kTheta":0.0,"kPhi":0.0})).collect();
    serde_json::from_value(serde_json::json!({"protocolVersion":1,"requestId":run.request.request_id,"type":"result","config":run.request.config,
        "environment":{"pythonVersion":"3.12","numpyVersion":"2","scipyVersion":"1"},"trajectories":[{"impactParameter":0.0,"termination":"through","samples":samples,
        "events":[{"kind":"throat","affine":radius,"radius":0.0},{"kind":"exit_negative","affine":2.0*radius,"radius":-radius}],
        "diagnostics":{"maxEnergyError":reported_error,"maxAngularMomentumError":0.0,"maxNullError":0.0,"maxEquatorialError":0.0,"turningRadiusError":null,"radialAnalyticError":0.0,"azimuthReferenceError":0.0},
        "validation":{"status":"passed","checks":[{"name":"旧宽松检查","actual":reported_error,"threshold":1e-3,"passed":true}]}}]})).unwrap()
}
fn complete(store: &ProjectStore, run: &RunRecord, reported_error: f64) -> RunRecord {
    let result=result_fixture(run,reported_error);
    store.transition(&run.id, RunState::Running, None, None).unwrap();
    store.transition(&run.id, RunState::Completed, Some(result), None).unwrap()
}

#[test]
fn rejects_false_radial_return_and_reversed_tangent() {
    for wrong_return in [true,false] {
        let (_dir,core,project,store,run)=setup();let mut result=result_fixture(&run,0.0);
        if wrong_return {
            result.trajectories[0].termination="returned".into();
            result.trajectories[0].events.push(Event {kind:"return_positive".into(),affine:20.0,radius:10.0});
        } else { result.trajectories[0].samples[0].kl=1.0; }
        store.transition(&run.id,RunState::Running,None,None).unwrap();
        store.transition(&run.id,RunState::Completed,Some(result),None).unwrap();
        let rule=core.list_verification_rules(&project.project.id,0,20).unwrap().rules.remove(0);
        let record=core.execute_verification(&project.project.id,ExecuteVerification {run_id:run.id,rule_version_id:rule.id,client_request_id:"错误物理支".into(),previous_record_id:None}).unwrap();
        assert_eq!(record.conclusion,VerificationConclusion::Failed);
    }
}

#[test]
fn multiline_reason_is_preserved_but_control_bytes_are_rejected() {
    let (_dir,core,project,_store,_run)=setup();let rule=core.list_verification_rules(&project.project.id,0,20).unwrap().rules.remove(0);
    let draft=VerificationRuleDraft {base_version_id:rule.id,title:"分段依据".into(),change_reason:"第一段\n第二段".into(),thresholds:rule.checks.iter().map(|c|VerificationThreshold {metric_id:c.metric_id,threshold:c.threshold,basis:"依据\n定位：SCI07\t条目".into()}).collect()};
    assert_eq!(core.save_verification_rule_version(&project.project.id,draft.clone()).unwrap().change_reason,draft.change_reason);
    let mut invalid=draft;invalid.change_reason.push('\0');assert!(core.save_verification_rule_version(&project.project.id,invalid).is_err());
}

#[test]
fn reassessment_does_not_trust_old_passed_and_never_mutates_run() {
    let (_dir, core, project, store, run) = setup();
    let original = complete(&store, &run, 1e-6);
    let rules = core.list_verification_rules(&project.project.id, 0, 20).unwrap();
    let rule = &rules.rules[0];
    let before = core.get_run_verification_state(&project.project.id, &run.id, &rule.id).unwrap();
    assert_eq!(before.conclusion, VerificationConclusion::NotRun);
    let request = ExecuteVerification { run_id:run.id.clone(),rule_version_id:rule.id.clone(),client_request_id:"审阅一".into(),previous_record_id:None };
    let first = core.execute_verification(&project.project.id, request.clone()).unwrap();
    assert_eq!(first.conclusion, VerificationConclusion::Failed);
    assert_eq!(core.execute_verification(&project.project.id, request).unwrap(), first);
    assert_eq!(serde_json::to_value(store.run(&run.id).unwrap()).unwrap(), serde_json::to_value(original).unwrap());
    assert_eq!(first.source.result_origin, ResultIdentityOrigin::CapturedAtCompletion);
    assert_eq!(core.list_verification_records(&project.project.id, &run.id, 0, 20).unwrap().total, 1);
}

#[test]
fn changed_threshold_requires_reason_and_keeps_old_record() {
    let (_dir, core, project, store, run) = setup(); complete(&store, &run, 1e-6);
    let rule = core.list_verification_rules(&project.project.id, 0, 20).unwrap().rules.remove(0);
    let mut draft = VerificationRuleDraft { base_version_id:rule.id.clone(),title:"敏感性检查".into(),change_reason:String::new(),thresholds:rule.checks.iter().map(|c| VerificationThreshold { metric_id:c.metric_id,threshold:if c.metric_id == VerificationMetric::EnergyError {1e-5} else {c.threshold},basis:"研究者自定的敏感性门槛".into() }).collect() };
    assert!(core.save_verification_rule_version(&project.project.id, draft.clone()).is_err());
    draft.change_reason = "检查阈值敏感性，保留基准结果".into();
    let changed = core.save_verification_rule_version(&project.project.id, draft).unwrap();
    assert_ne!(changed.content_hash, rule.content_hash); assert!(!changed.builtin);
    assert_eq!(core.list_verification_rules(&project.project.id, 0, 1).unwrap().next_offset, Some(1));
    assert!(core.list_verification_rules(&project.project.id, 0, 101).is_err());
    let result = core.execute_verification(&project.project.id, ExecuteVerification {run_id:run.id.clone(),rule_version_id:changed.id,client_request_id:"复评".into(),previous_record_id:None}).unwrap();
    assert_eq!(result.conclusion, VerificationConclusion::Passed);
    assert!(result.checks.iter().any(|c| c.conclusion == VerificationConclusion::NotApplicable));
}

#[test]
fn missing_output_is_a_record_not_zero_error() {
    let (_dir, core, project, store, run) = setup();
    store.transition(&run.id, RunState::Failed, None, Some(workbench_core::CoreError::new("process_spawn_failed", "未能启动"))).unwrap();
    let rule = core.list_verification_rules(&project.project.id, 0, 20).unwrap().rules.remove(0);
    let record = core.execute_verification(&project.project.id, ExecuteVerification {run_id:run.id,rule_version_id:rule.id,client_request_id:"缺失产物".into(),previous_record_id:None}).unwrap();
    assert_eq!(record.conclusion, VerificationConclusion::MissingArtifact);
    assert!(record.checks.iter().all(|c| c.actual.is_none()));
}

#[test]
fn reopening_closes_abandoned_request_without_recomputing() {
    let (_dir,core,project,store,run)=setup();complete(&store,&run,0.0);
    let rule=core.list_verification_rules(&project.project.id,0,20).unwrap().rules.remove(0);
    let request=ExecuteVerification {run_id:run.id.clone(),rule_version_id:rule.id.clone(),client_request_id:"中断请求".into(),previous_record_id:None};
    let finished=core.execute_verification(&project.project.id,request.clone()).unwrap();
    let db=rusqlite::Connection::open(store.directory().join(".gravity/workbench.sqlite")).unwrap();
    // 故障夹具只删除完成行，保留已提交的请求，等价于两次提交之间终止进程。
    db.execute_batch("DROP TRIGGER verification_records_immutable_delete").unwrap();
    db.execute("DELETE FROM verification_records WHERE id=?1",[&finished.id]).unwrap();
    assert!(core.get_run_verification_state(&project.project.id,&run.id,&rule.id).unwrap().pending_request_id.is_some());
    let reopened=Workbench::new();reopened.open_project(store.directory()).unwrap();
    let state=reopened.get_run_verification_state(&project.project.id,&run.id,&rule.id).unwrap();
    assert!(state.pending_request_id.is_none());
    assert_eq!(state.latest_record.as_ref().unwrap().execution_status,VerificationExecutionStatus::Interrupted);
    assert_eq!(reopened.execute_verification(&project.project.id,request).unwrap().id,state.latest_record.unwrap().id);
}

#[test]
fn same_content_tampering_is_caught_by_result_hash() {
    let (_dir,core,project,store,run)=setup();let finished=complete(&store,&run,0.0);
    let db=rusqlite::Connection::open(store.directory().join(".gravity/workbench.sqlite")).unwrap();
    db.execute_batch("DROP TRIGGER runs_transitions;DROP TRIGGER runs_frozen;").unwrap();
    let mut changed=finished;changed.result.as_mut().unwrap().trajectories[0].samples[1].t+=0.01;
    db.execute("UPDATE runs SET record_json=?1 WHERE id=?2",(serde_json::to_string(&changed).unwrap(),&run.id)).unwrap();
    assert_eq!(core.get_run(&project.project.id,&run.id).unwrap_err().code,"result_integrity_failed");
    assert!(store.state().is_err());
}

#[test]
fn reassessment_normalizes_length_time_and_angular_errors_by_throat_scale() {
    const RELATIVE_TIME_ERROR: f64 = 2e-7;
    const RELATIVE_ANGULAR_ERROR: f64 = 2e-9;
    const ROUNDING_BUDGET: f64 = 1e-14;
    for scale in [0.5, 1.0, 4.0] {
        let input = EllisConfig { throat_radius:scale,initial_radius:10.0*scale,impact_parameters:vec![0.0],max_affine_parameter:20.0*scale,sample_count:3,relative_tolerance:1e-10,absolute_tolerance:1e-12 };
        let (_dir, core, project, store, run) = setup_config(input);
        let mut result = result_fixture(&run, 0.0);
        // 同一无量纲扰动在长度单位变化后应产生同一检查值，避免a=1掩盖单位错误。
        result.trajectories[0].samples[1].t += RELATIVE_TIME_ERROR * scale;
        result.trajectories[0].samples[1].k_phi = RELATIVE_ANGULAR_ERROR / scale;
        store.transition(&run.id, RunState::Running, None, None).unwrap();
        store.transition(&run.id, RunState::Completed, Some(result), None).unwrap();
        let rule = core.list_verification_rules(&project.project.id, 0, 20).unwrap().rules.remove(0);
        let record = core.execute_verification(&project.project.id, ExecuteVerification {run_id:run.id,rule_version_id:rule.id,client_request_id:"尺度一致性".into(),previous_record_id:None}).unwrap();
        let actual = |metric| record.checks.iter().find(|check|check.metric_id==metric).unwrap().actual.unwrap();
        assert!((actual(VerificationMetric::RadialAnalyticError)-RELATIVE_TIME_ERROR).abs() < ROUNDING_BUDGET);
        assert!((actual(VerificationMetric::AngularMomentumError)-RELATIVE_ANGULAR_ERROR).abs() < ROUNDING_BUDGET);
        assert_eq!(record.conclusion, VerificationConclusion::Passed);
    }
}

#[test]
fn changed_source_during_request_commit_never_returns_a_passed_record() {
    let (_dir, core, project, store, run) = setup(); complete(&store, &run, 0.0);
    let db = rusqlite::Connection::open(store.directory().join(".gravity/workbench.sqlite")).unwrap();
    // 故障夹具在请求落盘与完成落盘之间修改产物；生产触发器不支持这种修改。
    db.execute_batch("DROP TRIGGER runs_transitions;
        CREATE TRIGGER inject_source_change AFTER INSERT ON verification_requests BEGIN
          UPDATE runs SET record_json=json_set(record_json,'$.result.trajectories[0].samples[1].t',10.25) WHERE id=NEW.run_id;
        END;").unwrap();
    let rule = core.list_verification_rules(&project.project.id, 0, 20).unwrap().rules.remove(0);
    let outcome = core.execute_verification(&project.project.id, ExecuteVerification {run_id:run.id,rule_version_id:rule.id,client_request_id:"源改变故障".into(),previous_record_id:None});
    assert!(outcome.is_err(), "请求提交后产物已改变，不能返回原缓存内容的绿色结论");
    assert_eq!(db.query_row("SELECT count(*) FROM verification_records", [], |row|row.get::<_,i64>(0)).unwrap(), 0);
}

#[test]
fn live_verification_lock_blocks_recovery_and_new_execution_until_released() {
    let (_dir, core, project, store, run) = setup(); complete(&store, &run, 0.0);
    let rule = core.list_verification_rules(&project.project.id, 0, 20).unwrap().rules.remove(0);
    let request = ExecuteVerification {run_id:run.id.clone(),rule_version_id:rule.id.clone(),client_request_id:"仍在执行".into(),previous_record_id:None};
    let record = core.execute_verification(&project.project.id, request.clone()).unwrap();
    let db = rusqlite::Connection::open(store.directory().join(".gravity/workbench.sqlite")).unwrap();
    db.execute_batch("DROP TRIGGER verification_records_immutable_delete").unwrap();
    db.execute("DELETE FROM verification_records WHERE id=?1", [&record.id]).unwrap();
    let live_lock = std::fs::OpenOptions::new().read(true).write(true).open(store.directory().join(".gravity/verification.lock")).unwrap();
    live_lock.try_lock().unwrap();
    let second_host = Workbench::new(); second_host.open_project(store.directory()).unwrap();
    assert_eq!(second_host.get_run_verification_state(&project.project.id, &run.id, &rule.id).unwrap().record_count, 0);
    assert_eq!(second_host.execute_verification(&project.project.id, request.clone()).unwrap_err().code, "verification_busy");
    drop(live_lock);
    second_host.open_project(store.directory()).unwrap();
    let recovered = second_host.execute_verification(&project.project.id, request).unwrap();
    assert_eq!(recovered.execution_status, VerificationExecutionStatus::Interrupted);
    assert_eq!(recovered.conclusion, VerificationConclusion::Inconclusive);
    assert_eq!(recovered.error.unwrap().code, "verification_interrupted");
}

#[test]
fn idempotent_request_conflicts_and_host_shutdown_do_not_append_history() {
    let (_dir, core, project, store, run) = setup(); complete(&store, &run, 0.0);
    let rule = core.list_verification_rules(&project.project.id, 0, 20).unwrap().rules.remove(0);
    let request = ExecuteVerification {run_id:run.id.clone(),rule_version_id:rule.id,client_request_id:"单一身份".into(),previous_record_id:None};
    let original = core.execute_verification(&project.project.id, request.clone()).unwrap();
    let mut conflicting = request.clone(); conflicting.previous_record_id = Some(original.id);
    assert_eq!(core.execute_verification(&project.project.id, conflicting).unwrap_err().code, "verification_request_conflict");
    core.shutdown().unwrap();
    assert_eq!(core.execute_verification(&project.project.id, request).unwrap_err().code, "host_stopping");
    assert_eq!(core.list_verification_records(&project.project.id, &run.id, 0, 20).unwrap().total, 1);
}

#[test]
fn active_runs_invalid_requests_and_cross_run_history_never_append_verification() {
    const PAGE_SIZE: usize = 20;
    let (_dir, core, project, store, run) = setup();
    let project_id = &project.project.id;
    let rule = core.list_verification_rules(project_id, 0, PAGE_SIZE).unwrap().rules.remove(0);
    let request = ExecuteVerification {run_id:run.id.clone(),rule_version_id:rule.id.clone(),client_request_id:"等待终态".into(),previous_record_id:None};
    for state in [RunState::Queued, RunState::Running, RunState::Cancelling] {
        if state != RunState::Queued { store.transition(&run.id, state, None, None).unwrap(); }
        assert_eq!(core.execute_verification(project_id, request.clone()).unwrap_err().code, "verification_run_active");
    }
    store.transition(&run.id, RunState::Cancelled, None, None).unwrap();
    for client in ["", "包含\0控制字节"] {
        let mut invalid = request.clone(); invalid.client_request_id = client.into();
        assert_eq!(core.execute_verification(project_id, invalid).unwrap_err().code, "invalid_verification_request");
    }
    assert_eq!(core.list_verification_records(project_id, &run.id, 0, PAGE_SIZE).unwrap().total, 0);
    let first = core.execute_verification(project_id, request).unwrap();
    let preflight = store.prepare(&run.model_version_id, run.environment.clone(), ExecutionLimits::default(), vec![]).unwrap();
    let other = store.consume_preflight(&preflight.id, &run.environment).unwrap();
    store.transition(&other.id, RunState::Failed, None, Some(workbench_core::CoreError::new("process_spawn_failed", "启动失败"))).unwrap();
    let foreign_history = ExecuteVerification {run_id:other.id.clone(),rule_version_id:rule.id.clone(),client_request_id:"跨运行历史".into(),previous_record_id:Some(first.id.clone())};
    assert_eq!(core.execute_verification(project_id, foreign_history).unwrap_err().code, "verification_history_conflict");
    assert_eq!(core.list_verification_records(project_id, &other.id, 0, PAGE_SIZE).unwrap().total, 0);
    // 同一运行的显式重检可关联旧记录，并在分页中保留两次独立事实。
    let second = core.execute_verification(project_id, ExecuteVerification {run_id:run.id.clone(),rule_version_id:rule.id.clone(),client_request_id:"同运行重检".into(),previous_record_id:Some(first.id.clone())}).unwrap();
    let page = core.list_verification_records(project_id, &run.id, 0, 1).unwrap();
    assert_eq!(page.records, vec![first]); assert_eq!(page.total, 2); assert_eq!(page.next_offset, Some(1));
    let final_page = core.list_verification_records(project_id, &run.id, 1, 1).unwrap();
    assert_eq!(final_page.records, vec![second.clone()]); assert_eq!(final_page.next_offset, None);
    assert_eq!(core.get_verification_record(project_id, &second.id).unwrap(), second);
}

#[test]
fn tampered_verification_record_or_request_is_rejected_on_all_history_reads() {
    const PAGE_SIZE: usize = 20;
    for target in ["record", "request"] {
        let (_dir, core, project, store, run) = setup(); complete(&store, &run, 0.0);
        let project_id = &project.project.id;
        let rule = core.list_verification_rules(project_id, 0, PAGE_SIZE).unwrap().rules.remove(0);
        let request = ExecuteVerification {run_id:run.id.clone(),rule_version_id:rule.id.clone(),client_request_id:"验证账本篡改".into(),previous_record_id:None};
        let record = core.execute_verification(project_id, request.clone()).unwrap();
        let db = rusqlite::Connection::open(store.directory().join(".gravity/workbench.sqlite")).unwrap();
        // 模拟磁盘被外部工具篡改，正常接口仍由不可变触发器保护。
        if target == "record" {
            db.execute_batch("DROP TRIGGER verification_records_immutable_update").unwrap();
            db.execute("UPDATE verification_records SET record_json=json_set(record_json,'$.checks[0].actual',0.5) WHERE id=?1", [&record.id]).unwrap();
        } else {
            db.execute_batch("DROP TRIGGER verification_requests_immutable_update").unwrap();
            db.execute("UPDATE verification_requests SET record_json=json_set(record_json,'$.request.clientRequestId','外部替换') WHERE id=?1", [&record.request_id]).unwrap();
        }
        assert_eq!(core.get_verification_record(project_id, &record.id).unwrap_err().code, "corrupt_verification");
        assert_eq!(core.list_verification_records(project_id, &run.id, 0, PAGE_SIZE).unwrap_err().code, "corrupt_verification");
        assert_eq!(core.get_run_verification_state(project_id, &run.id, &rule.id).unwrap_err().code, "corrupt_verification");
        assert_eq!(core.execute_verification(project_id, request).unwrap_err().code, "corrupt_verification");
        assert_eq!(db.query_row("SELECT count(*) FROM verification_records", [], |row|row.get::<_,i64>(0)).unwrap(), 1);
    }
}

#[test]
fn pending_idempotent_request_does_not_execute_again_before_recovery() {
    let (_dir, core, project, store, run) = setup(); complete(&store, &run, 0.0);
    let rule = core.list_verification_rules(&project.project.id, 0, 20).unwrap().rules.remove(0);
    let request = ExecuteVerification {run_id:run.id.clone(),rule_version_id:rule.id,client_request_id:"待确认的请求".into(),previous_record_id:None};
    let record = core.execute_verification(&project.project.id, request.clone()).unwrap();
    let db = rusqlite::Connection::open(store.directory().join(".gravity/workbench.sqlite")).unwrap();
    // 精确表示请求已提交、完成行尚未提交的中断窗口。
    db.execute_batch("DROP TRIGGER verification_records_immutable_delete").unwrap();
    db.execute("DELETE FROM verification_records WHERE id=?1", [&record.id]).unwrap();
    assert_eq!(core.execute_verification(&project.project.id, request).unwrap_err().code, "verification_pending");
    assert_eq!(db.query_row("SELECT count(*) FROM verification_requests", [], |row|row.get::<_,i64>(0)).unwrap(), 1);
    assert_eq!(db.query_row("SELECT count(*) FROM verification_records", [], |row|row.get::<_,i64>(0)).unwrap(), 0);
}
