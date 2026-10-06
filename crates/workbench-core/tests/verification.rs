//! 独立验证必须依据实际测量值、保存不可变历史，并保持运行快照原样。
use workbench_core::{storage::ProjectStore, types::*, Workbench};

fn setup() -> (tempfile::TempDir, Workbench, ProjectState, ProjectStore, RunRecord) {
    let dir = tempfile::tempdir().unwrap(); let core = Workbench::new();
    let project = core.create_project(dir.path(), "独立验证").unwrap();
    let store = ProjectStore::open(std::path::Path::new(&project.project.path)).unwrap();
    let config = EllisConfig { throat_radius:1.0,initial_radius:10.0,impact_parameters:vec![0.0],max_affine_parameter:20.0,sample_count:3,relative_tolerance:1e-10,absolute_tolerance:1e-12 };
    let model = store.save_model("径向模型", config).unwrap();
    let env = EnvironmentInfo { python_executable:"python".into(),engine_version:"0.1.0".into(),python_version:"3.12".into(),numpy_version:"2".into(),scipy_version:"1".into(),engine_source_hash:"a".repeat(64) };
    let preflight = store.prepare(&model.id, env.clone(), ExecutionLimits::default(), vec![]).unwrap();
    let run = store.consume_preflight(&preflight.id, &env).unwrap();
    (dir, core, project, store, run)
}

fn result_fixture(run: &RunRecord, reported_error: f64) -> TraceResult {
    let samples: Vec<_> = [0.0,10.0,20.0].iter().map(|t| serde_json::json!({"affine":t,"t":t,"l":10.0-t,"theta":std::f64::consts::FRAC_PI_2,"phi":0.0,"kt":1.0,"kl":-1.0,"kTheta":0.0,"kPhi":0.0})).collect();
    serde_json::from_value(serde_json::json!({"protocolVersion":1,"requestId":run.request.request_id,"type":"result","config":run.request.config,
        "environment":{"pythonVersion":"3.12","numpyVersion":"2","scipyVersion":"1"},"trajectories":[{"impactParameter":0.0,"termination":"through","samples":samples,
        "events":[{"kind":"throat","affine":10.0,"radius":0.0},{"kind":"exit_negative","affine":20.0,"radius":-10.0}],
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
