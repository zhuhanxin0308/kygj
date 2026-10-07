//! v1迁移必须先预览，备份一致性状态，并在任何失败时保留原格式。
use workbench_core::{Workbench,types::*,storage::sha256_bytes};
fn legacy() -> (tempfile::TempDir,std::path::PathBuf) {
    let dir=tempfile::tempdir().unwrap(); let project=dir.path().join("旧项目"); std::fs::create_dir(&project).unwrap(); std::fs::create_dir(project.join(".gravity")).unwrap();
    let connection=rusqlite::Connection::open(project.join(".gravity/workbench.sqlite")).unwrap();
    connection.execute_batch(include_str!("fixtures/project-v1.sql")).unwrap();
    let summary=ProjectSummary {id:uuid::Uuid::new_v4().to_string(),name:"旧项目".into(),path:project.to_string_lossy().into(),created_at:workbench_core::storage::timestamp(),schema_version:1};
    connection.execute("INSERT INTO projects(id,record_json) VALUES (?1,?2)",(&summary.id,serde_json::to_string(&summary).unwrap())).unwrap();
    (dir,project)
}
#[test]
fn preview_is_read_only_and_confirm_creates_verified_backup() {
    let (_dir,path)=legacy(); let core=Workbench::new();
    assert_eq!(core.open_project(&path).unwrap_err().code,"migration_required");
    let plan=core.prepare_project_migration(&path).unwrap();
    assert!(!std::path::Path::new(&plan.backup_path).exists());
    assert_eq!(core.open_project(&path).unwrap_err().code,"migration_required");
    assert!(Workbench::new().apply_project_migration(&path,&plan.id).is_err());
    let receipt=core.apply_project_migration(&path,&plan.id).unwrap();
    assert_eq!(core.open_project(&path).unwrap().project.schema_version,2);
    let backup=rusqlite::Connection::open(&receipt.backup_path).unwrap();
    assert_eq!(backup.pragma_query_value(None,"user_version",|r|r.get::<_,u32>(0)).unwrap(),1);
    assert_eq!(receipt.backup_sha256.len(),64);
    assert!(core.apply_project_migration(&path,&plan.id).is_err());
}
#[test]
fn stale_plan_and_transaction_failure_leave_version_one() {
    let (_dir,path)=legacy(); let core=Workbench::new();
    let plan=core.prepare_project_migration(&path).unwrap();
    let connection=rusqlite::Connection::open(path.join(".gravity/workbench.sqlite")).unwrap();
    connection.execute_batch("CREATE INDEX source_changed ON projects(id)").unwrap();
    assert_eq!(core.apply_project_migration(&path,&plan.id).unwrap_err().code,"migration_plan_stale");
    connection.execute_batch("CREATE TRIGGER block_migration BEFORE UPDATE ON projects BEGIN SELECT RAISE(ABORT,'injected failure'); END").unwrap();
    let fresh=core.prepare_project_migration(&path).unwrap();
    assert!(core.apply_project_migration(&path,&fresh.id).is_err());
    assert_eq!(connection.pragma_query_value(None,"user_version",|r|r.get::<_,u32>(0)).unwrap(),1);
    assert_eq!(connection.query_row("SELECT count(*) FROM sqlite_master WHERE name='verification_rules'",[],|r|r.get::<_,i64>(0)).unwrap(),0);
    let backup=rusqlite::Connection::open(&fresh.backup_path).unwrap();
    assert_eq!(backup.query_row("PRAGMA quick_check",[],|r|r.get::<_,String>(0)).unwrap(),"ok");
}

/// 固定v1结构中写入完整径向解析轨迹，专门验收WAL和迁移原字节保留。
fn append_legacy_run(connection: &mut rusqlite::Connection) -> String {
    let project_id: String = connection.query_row("SELECT id FROM projects", [], |row| row.get(0)).unwrap();
    let time = workbench_core::storage::timestamp();
    let config = EllisConfig { throat_radius:1.0,initial_radius:10.0,impact_parameters:vec![0.0],max_affine_parameter:20.0,sample_count:2,relative_tolerance:1e-10,absolute_tolerance:1e-12 };
    let model = ModelVersion {id:uuid::Uuid::new_v4().to_string(),project_id:project_id.clone(),label:"v1径向源模型".into(),created_at:time.clone(),content_hash:sha256_bytes(&serde_json::to_vec(&config).unwrap()),config:config.clone()};
    let environment = EnvironmentInfo {python_executable:"已归档环境".into(),engine_version:"0.1.0".into(),python_version:"3.12".into(),numpy_version:"2".into(),scipy_version:"1".into(),engine_source_hash:"a".repeat(64)};
    let request = TraceRequest::new(uuid::Uuid::new_v4().to_string(), config.clone());
    let samples: Vec<_> = [0.0,20.0].iter().map(|value|serde_json::json!({"affine":value,"t":value,"l":10.0-value,"theta":std::f64::consts::FRAC_PI_2,"phi":0.0,"kt":1.0,"kl":-1.0,"kTheta":0.0,"kPhi":0.0})).collect();
    let result: TraceResult = serde_json::from_value(serde_json::json!({"protocolVersion":1,"requestId":request.request_id,"type":"result","config":config,
        "environment":{"pythonVersion":"3.12","numpyVersion":"2","scipyVersion":"1"},"trajectories":[{"impactParameter":0.0,"termination":"through","samples":samples,
        "events":[{"kind":"throat","affine":10.0,"radius":0.0},{"kind":"exit_negative","affine":20.0,"radius":-10.0}],
        "diagnostics":{"maxEnergyError":0.0,"maxAngularMomentumError":0.0,"maxNullError":0.0,"maxEquatorialError":0.0,"turningRadiusError":null,"radialAnalyticError":0.0,"azimuthReferenceError":0.0},
        "validation":{"status":"passed","checks":[{"name":"v1径向解析检查","actual":0.0,"threshold":1e-8,"passed":true}]}}]})).unwrap();
    let run = RunRecord {id:uuid::Uuid::new_v4().to_string(),project_id:project_id.clone(),model_version_id:model.id.clone(),created_at:time.clone(),started_at:Some(time.clone()),finished_at:Some(time),state:RunState::Completed,validation_status:ValidationStatus::Passed,request,environment,result:Some(result),error:None};
    let transaction = connection.transaction().unwrap();
    transaction.execute("INSERT INTO models(id,project_id,label,created_at,record_json) VALUES (?1,?2,?3,?4,?5)",(&model.id,&project_id,&model.label,&model.created_at,serde_json::to_string(&model).unwrap())).unwrap();
    transaction.execute("INSERT INTO runs(id,project_id,model_id,created_at,state,request_json,environment_json,record_json) VALUES (?1,?2,?3,?4,'completed',?5,?6,?7)",
        (&run.id,&project_id,&model.id,&run.created_at,serde_json::to_string(&run.request).unwrap(),serde_json::to_string(&run.environment).unwrap(),serde_json::to_string(&run).unwrap())).unwrap();
    transaction.commit().unwrap(); run.id
}

#[test]
fn wal_backup_contains_committed_v1_results_and_migration_preserves_source_bytes() {
    let (_dir, path) = legacy();
    let mut connection = rusqlite::Connection::open(path.join(".gravity/workbench.sqlite")).unwrap();
    connection.execute_batch("PRAGMA journal_mode=WAL; PRAGMA wal_autocheckpoint=0;").unwrap();
    let run_id = append_legacy_run(&mut connection);
    let original: String = connection.query_row("SELECT record_json FROM runs WHERE id=?1", [&run_id], |row|row.get(0)).unwrap();
    assert!(std::fs::metadata(path.join(".gravity/workbench.sqlite-wal")).unwrap().len()>0, "验收必须实际包含尚在WAL中的提交");
    let core = Workbench::new(); let plan = core.prepare_project_migration(&path).unwrap();
    let receipt = core.apply_project_migration(&path, &plan.id).unwrap();
    let backup = rusqlite::Connection::open(&receipt.backup_path).unwrap();
    assert_eq!(backup.query_row("SELECT record_json FROM runs WHERE id=?1", [&run_id], |row|row.get::<_,String>(0)).unwrap(), original);
    assert_eq!(connection.query_row("SELECT record_json FROM runs WHERE id=?1", [&run_id], |row|row.get::<_,String>(0)).unwrap(), original);
    assert_eq!(receipt.backup_sha256, sha256_bytes(&std::fs::read(&receipt.backup_path).unwrap()));
    assert_eq!(receipt.legacy_result_count, 1);
    let state = core.open_project(&path).unwrap(); let rule = core.list_verification_rules(&state.project.id, 0, 20).unwrap().rules.remove(0);
    assert_eq!(core.get_run_verification_state(&state.project.id, &run_id, &rule.id).unwrap().conclusion, VerificationConclusion::NotRun);
    let record = core.execute_verification(&state.project.id, ExecuteVerification {run_id,rule_version_id:rule.id,client_request_id:"迁移后显式重评".into(),previous_record_id:None}).unwrap();
    assert_eq!(record.source.result_origin, ResultIdentityOrigin::ObservedAtMigration);
}

#[test]
fn rollback_after_backup_preserves_populated_scientific_records_and_recovery_material() {
    let (_dir, path) = legacy();
    let mut connection = rusqlite::Connection::open(path.join(".gravity/workbench.sqlite")).unwrap();
    let run_id = append_legacy_run(&mut connection);
    let original: String = connection.query_row("SELECT record_json FROM runs WHERE id=?1", [&run_id], |row|row.get(0)).unwrap();
    connection.execute_batch("CREATE TRIGGER prevent_migration BEFORE UPDATE ON projects BEGIN SELECT RAISE(ABORT,'事务故障注入'); END;").unwrap();
    let core = Workbench::new(); let plan = core.prepare_project_migration(&path).unwrap();
    assert!(core.apply_project_migration(&path, &plan.id).is_err());
    assert_eq!(connection.query_row("SELECT record_json FROM runs WHERE id=?1", [&run_id], |row|row.get::<_,String>(0)).unwrap(), original);
    assert_eq!(core.open_project(&path).unwrap_err().code, "migration_required");
    let backup = rusqlite::Connection::open(&plan.backup_path).unwrap();
    assert_eq!(backup.query_row("SELECT record_json FROM runs WHERE id=?1", [&run_id], |row|row.get::<_,String>(0)).unwrap(), original);
    assert_eq!(connection.query_row("SELECT count(*) FROM sqlite_schema WHERE name='result_identities'", [], |row|row.get::<_,i64>(0)).unwrap(), 0);
}
