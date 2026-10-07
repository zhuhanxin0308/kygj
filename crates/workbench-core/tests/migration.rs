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

/// 拒绝预览时不能改写数据库，也不能提前生成看似可用的迁移备份。
fn assert_preview_rejected(path: &std::path::Path, expected_code: &str) {
    let database = path.join(".gravity/workbench.sqlite");
    let original = std::fs::read(&database).unwrap();
    let error = Workbench::new().prepare_project_migration(path).unwrap_err();
    assert_eq!(error.code, expected_code, "拒绝原因不符合预期：{}", error.message);
    assert_eq!(std::fs::read(database).unwrap(), original);
    assert!(std::fs::read_dir(path.join(".gravity")).unwrap().all(|entry| {
        !entry.unwrap().file_name().to_string_lossy().starts_with("migration-v1-backup-")
    }));
}

/// 失败后的旧库不能残留半套新格式对象；备份是否保留由具体失败阶段单独断言。
fn assert_original_schema(connection: &rusqlite::Connection) {
    assert_eq!(connection.pragma_query_value(None, "user_version", |row| row.get::<_, u32>(0)).unwrap(), 1);
    let introduced: i64 = connection.query_row(
        "SELECT count(*) FROM sqlite_schema WHERE type='table' AND name IN ('result_identities','verification_rules','verification_requests','verification_records','migration_history')",
        [], |row| row.get(0),
    ).unwrap();
    assert_eq!(introduced, 0);
}

#[test]
fn preview_rejects_missing_paths_files_and_unreadable_database_shapes() {
    let directory = tempfile::tempdir().unwrap();
    let missing = directory.path().join("不存在的项目");
    assert_eq!(Workbench::new().prepare_project_migration(&missing).unwrap_err().code, "file_access_error");
    let ordinary_file = directory.path().join("普通文件");
    std::fs::write(&ordinary_file, b"not a project directory").unwrap();
    assert_eq!(Workbench::new().prepare_project_migration(&ordinary_file).unwrap_err().code, "invalid_project_path");
    assert_eq!(Workbench::new().prepare_project_migration(directory.path()).unwrap_err().code, "file_access_error");

    for database_is_directory in [false, true] {
        let (_directory, path) = legacy();
        let database = path.join(".gravity/workbench.sqlite");
        std::fs::remove_file(&database).unwrap();
        if database_is_directory {
            std::fs::create_dir(&database).unwrap();
            assert_eq!(Workbench::new().prepare_project_migration(&path).unwrap_err().code, "migration_storage_error");
        } else {
            assert_eq!(Workbench::new().prepare_project_migration(&path).unwrap_err().code, "file_access_error");
            // 扩展名相同的普通文件必须在解析SQLite头时失败，不能被当成空项目重建。
            std::fs::write(&database, b"this is not a SQLite database").unwrap();
            assert_preview_rejected(&path, "migration_storage_error");
        }
    }
}

#[test]
fn preview_only_accepts_recognized_version_one_headers() {
    for (pragma, value, expected) in [
        ("application_id", 0, "migration_not_supported"),
        ("user_version", 0, "migration_not_supported"),
        ("user_version", 2, "migration_not_supported"),
        ("user_version", -1, "migration_storage_error"),
    ] {
        let (_directory, path) = legacy();
        let connection = rusqlite::Connection::open(path.join(".gravity/workbench.sqlite")).unwrap();
        connection.pragma_update(None, pragma, value).unwrap();
        assert_preview_rejected(&path, expected);
    }
}

#[test]
fn preview_rejects_absent_duplicate_and_invalid_project_identity() {
    for duplicate in [false, true] {
        let (_directory, path) = legacy();
        let connection = rusqlite::Connection::open(path.join(".gravity/workbench.sqlite")).unwrap();
        if duplicate {
            connection.execute("INSERT INTO projects(id,record_json) SELECT ?1,record_json FROM projects", [uuid::Uuid::new_v4().to_string()]).unwrap();
        } else {
            connection.execute("DELETE FROM projects", []).unwrap();
        }
        assert_preview_rejected(&path, "corrupt_project");
    }

    for field in ["id", "schemaVersion", "name"] {
        let (_directory, path) = legacy();
        let connection = rusqlite::Connection::open(path.join(".gravity/workbench.sqlite")).unwrap();
        let original: String = connection.query_row("SELECT record_json FROM projects", [], |row| row.get(0)).unwrap();
        let mut project: serde_json::Value = serde_json::from_str(&original).unwrap();
        project[field] = match field {
            "id" => uuid::Uuid::new_v4().to_string().into(),
            "schemaVersion" => 2.into(),
            _ => "../其他项目".into(),
        };
        connection.execute("UPDATE projects SET record_json=?1", [project.to_string()]).unwrap();
        assert_preview_rejected(&path, if field == "name" { "invalid_project_name" } else { "corrupt_project" });
    }

    let (_directory, path) = legacy();
    let connection = rusqlite::Connection::open(path.join(".gravity/workbench.sqlite")).unwrap();
    // 列值和JSON相同也不代表身份合法：两者都不是UUID时仍必须拒绝。
    connection.execute("UPDATE projects SET id='invalid-project-id',record_json=json_set(record_json,'$.id','invalid-project-id')", []).unwrap();
    assert_preview_rejected(&path, "corrupt_project");
}

#[test]
fn preview_rejects_malformed_project_payload_and_missing_legacy_schema() {
    for sql in [
        "UPDATE projects SET record_json='{'",
        "UPDATE projects SET record_json=X'00FF'",
        "ALTER TABLE projects RENAME COLUMN record_json TO unexpected_payload",
    ] {
        let (_directory, path) = legacy();
        let connection = rusqlite::Connection::open(path.join(".gravity/workbench.sqlite")).unwrap();
        connection.execute_batch(sql).unwrap();
        assert_preview_rejected(&path, if sql.contains("='{'") { "corrupt_project" } else { "migration_storage_error" });
    }
    for table in ["projects", "models", "runs", "preflights"] {
        let (_directory, path) = legacy();
        let connection = rusqlite::Connection::open(path.join(".gravity/workbench.sqlite")).unwrap();
        // 表名来自封闭的测试矩阵，覆盖身份读取、科学记录读取和逻辑指纹三个阶段。
        connection.execute_batch(&format!("DROP TABLE {table}")).unwrap();
        assert_preview_rejected(&path, "migration_storage_error");
    }
}

#[test]
fn confirmation_rejects_check_constraint_corruption_before_backup() {
    let (_directory, path) = legacy();
    let database = path.join(".gravity/workbench.sqlite");
    let connection = rusqlite::Connection::open(&database).unwrap();
    // 模拟外部工具绕过真实v1约束留下的损坏记录，不能迁移或生成貌似可靠的备份。
    connection.pragma_update(None, "ignore_check_constraints", true).unwrap();
    connection.execute("INSERT INTO preflights(id,project_id,record_json,consumed) SELECT ?1,id,'{}',2 FROM projects", [uuid::Uuid::new_v4().to_string()]).unwrap();
    connection.pragma_update(None, "ignore_check_constraints", false).unwrap();
    assert_ne!(connection.query_row("PRAGMA quick_check", [], |row| row.get::<_, String>(0)).unwrap(), "ok", "夹具必须产生真实SQLite完整性失败");
    // SQLite只读连接不加载CHECK表达式；确认阶段的可写连接必须重新检查，不能信任预览结论。
    let original = std::fs::read(&database).unwrap();
    let core = Workbench::new();
    let plan = core.prepare_project_migration(&path).unwrap();
    assert_eq!(std::fs::read(&database).unwrap(), original);
    assert_eq!(core.apply_project_migration(&path, &plan.id).unwrap_err().code, "corrupt_project");
    assert_eq!(std::fs::read(&database).unwrap(), original);
    assert_original_schema(&connection);
    assert!(!std::path::Path::new(&plan.backup_path).exists());
}

#[test]
fn preview_rejects_malformed_scientific_json_and_wrong_sqlite_value_types() {
    for table in ["models", "runs"] {
        for corruption in ["malformed_json", "blob_identity", "blob_payload"] {
            let (_directory, path) = legacy();
            let mut connection = rusqlite::Connection::open(path.join(".gravity/workbench.sqlite")).unwrap();
            append_legacy_run(&mut connection);
            // 模拟磁盘篡改时显式绕过外键和不可变触发器，迁移入口仍须独立检查类型及内容。
            connection.pragma_update(None, "foreign_keys", false).unwrap();
            connection.execute_batch("DROP TRIGGER models_no_update; DROP TRIGGER runs_frozen; DROP TRIGGER runs_transitions;").unwrap();
            let assignment = match corruption {
                "malformed_json" => "record_json='{'",
                "blob_identity" => "id=X'00FF'",
                _ => "record_json=X'00FF'",
            };
            connection.execute_batch(&format!("UPDATE {table} SET {assignment}")).unwrap();
            connection.pragma_update(None, "foreign_keys", true).unwrap();
            assert_preview_rejected(&path, if corruption == "malformed_json" { "corrupt_project" } else { "migration_storage_error" });
        }
    }
}

#[test]
fn preview_rejects_scientific_identity_hash_and_frozen_relation_tampering() {
    for (table, assignment) in [
        ("models", "record_json=json_set(record_json,'$.id','00000000-0000-4000-8000-000000000001')"),
        ("runs", "record_json=json_set(record_json,'$.id','00000000-0000-4000-8000-000000000001')"),
        ("models", "record_json=json_set(record_json,'$.contentHash','')"),
        ("models", "label='被外部改写的标签'"),
        ("runs", "environment_json='{}'"),
        ("runs", "record_json=json_set(record_json,'$.result.requestId','其他请求')"),
    ] {
        let (_directory, path) = legacy();
        let mut connection = rusqlite::Connection::open(path.join(".gravity/workbench.sqlite")).unwrap();
        append_legacy_run(&mut connection);
        connection.execute_batch("DROP TRIGGER models_no_update; DROP TRIGGER runs_frozen; DROP TRIGGER runs_transitions;").unwrap();
        connection.execute_batch(&format!("UPDATE {table} SET {assignment}")).unwrap();
        assert_preview_rejected(&path, "corrupt_project");
    }
}

#[test]
fn apply_rejects_a_different_project_path_and_consumes_the_plan() {
    let (_source_directory, source) = legacy();
    let (_other_directory, other) = legacy();
    let core = Workbench::new();
    let plan = core.prepare_project_migration(&source).unwrap();
    assert_eq!(core.apply_project_migration(&other, &plan.id).unwrap_err().code, "migration_plan_stale");
    assert_eq!(core.apply_project_migration(&source, &plan.id).unwrap_err().code, "migration_plan_stale");
    assert!(!std::path::Path::new(&plan.backup_path).exists());
    for path in [&source, &other] {
        assert_original_schema(&rusqlite::Connection::open(path.join(".gravity/workbench.sqlite")).unwrap());
    }
}

#[test]
fn apply_rechecks_project_identity_and_new_records_after_preview() {
    for identity_changed in [false, true] {
        let (_directory, path) = legacy();
        let core = Workbench::new();
        let plan = core.prepare_project_migration(&path).unwrap();
        let connection = rusqlite::Connection::open(path.join(".gravity/workbench.sqlite")).unwrap();
        if identity_changed {
            let replacement_id = uuid::Uuid::new_v4().to_string();
            connection.execute("UPDATE projects SET id=?1,record_json=json_set(record_json,'$.id',?1)", [&replacement_id]).unwrap();
        } else {
            // 预览后追加的数据也属于源指纹，确认不能无声地迁移未经预览的新状态。
            connection.execute("INSERT INTO preflights(id,project_id,record_json) SELECT ?1,id,'{}' FROM projects", [uuid::Uuid::new_v4().to_string()]).unwrap();
        }
        assert_eq!(core.apply_project_migration(&path, &plan.id).unwrap_err().code, "migration_plan_stale");
        assert_original_schema(&connection);
        assert!(!std::path::Path::new(&plan.backup_path).exists());
    }
}

#[test]
fn existing_backup_files_and_directories_are_never_overwritten() {
    const EXISTING_RECOVERY_DATA: &[u8] = b"recovery material owned by another operation";
    for backup_is_directory in [false, true] {
        let (_directory, path) = legacy();
        let core = Workbench::new();
        let plan = core.prepare_project_migration(&path).unwrap();
        let backup = std::path::Path::new(&plan.backup_path);
        if backup_is_directory { std::fs::create_dir(backup).unwrap(); }
        else { std::fs::write(backup, EXISTING_RECOVERY_DATA).unwrap(); }
        assert_eq!(core.apply_project_migration(&path, &plan.id).unwrap_err().code, "migration_backup_failed");
        assert_original_schema(&rusqlite::Connection::open(path.join(".gravity/workbench.sqlite")).unwrap());
        if backup_is_directory { assert!(backup.is_dir()); }
        else { assert_eq!(std::fs::read(backup).unwrap(), EXISTING_RECOVERY_DATA); }
    }
}

#[test]
fn a_competing_writer_prevents_migration_without_partial_schema_or_backup() {
    let (_directory, path) = legacy();
    let core = Workbench::new();
    let plan = core.prepare_project_migration(&path).unwrap();
    let mut connection = rusqlite::Connection::open(path.join(".gravity/workbench.sqlite")).unwrap();
    // 保持真实写锁，覆盖确认阶段取得Immediate事务失败后的回滚与计划消费。
    let writer = connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate).unwrap();
    assert_eq!(core.apply_project_migration(&path, &plan.id).unwrap_err().code, "migration_storage_error");
    assert_original_schema(&writer);
    assert!(!std::path::Path::new(&plan.backup_path).exists());
    writer.rollback().unwrap();
    let refreshed = core.prepare_project_migration(&path).unwrap();
    assert!(core.apply_project_migration(&path, &refreshed.id).is_ok());
}

#[test]
fn scientific_changes_caused_by_migration_triggers_roll_back_and_keep_the_backup() {
    let (_directory, path) = legacy();
    let mut connection = rusqlite::Connection::open(path.join(".gravity/workbench.sqlite")).unwrap();
    append_legacy_run(&mut connection);
    let original: (String, String) = connection.query_row("SELECT label,record_json FROM models", [], |row| Ok((row.get(0)?, row.get(1)?))).unwrap();
    // 源库携带的触发器可能在项目版本更新时改写科学数据，必须由迁移末尾的复核阻止。
    connection.execute_batch("DROP TRIGGER models_no_update; CREATE TRIGGER mutate_scientific_source AFTER UPDATE ON projects BEGIN UPDATE models SET label='迁移时错误改写'; END;").unwrap();
    let core = Workbench::new();
    let plan = core.prepare_project_migration(&path).unwrap();
    assert_eq!(core.apply_project_migration(&path, &plan.id).unwrap_err().code, "migration_source_changed");
    assert_original_schema(&connection);
    let persisted: (String, String) = connection.query_row("SELECT label,record_json FROM models", [], |row| Ok((row.get(0)?, row.get(1)?))).unwrap();
    assert_eq!(persisted, original);
    let backup = rusqlite::Connection::open(&plan.backup_path).unwrap();
    let recovered: (String, String) = backup.query_row("SELECT label,record_json FROM models", [], |row| Ok((row.get(0)?, row.get(1)?))).unwrap();
    assert_eq!(recovered, original);
    assert_original_schema(&backup);
}

#[test]
fn conflicting_extension_tables_roll_back_partial_initialization_and_keep_recovery_data() {
    let (_directory, path) = legacy();
    let connection = rusqlite::Connection::open(path.join(".gravity/workbench.sqlite")).unwrap();
    // 旧库中的同名扩展对象不能被覆盖；此前成功创建的新表也必须随事务回滚。
    connection.execute_batch("CREATE TABLE verification_rules(id TEXT PRIMARY KEY,record_json TEXT NOT NULL); INSERT INTO verification_rules VALUES ('legacy-extension','original-data');").unwrap();
    let core = Workbench::new();
    let plan = core.prepare_project_migration(&path).unwrap();
    assert_eq!(core.apply_project_migration(&path, &plan.id).unwrap_err().code, "storage_error");
    assert_eq!(connection.pragma_query_value(None, "user_version", |row| row.get::<_, u32>(0)).unwrap(), 1);
    assert_eq!(connection.query_row("SELECT count(*) FROM sqlite_schema WHERE name='result_identities'", [], |row| row.get::<_, i64>(0)).unwrap(), 0);
    assert_eq!(connection.query_row("SELECT record_json FROM verification_rules", [], |row| row.get::<_, String>(0)).unwrap(), "original-data");
    let backup = rusqlite::Connection::open(&plan.backup_path).unwrap();
    assert_eq!(backup.query_row("SELECT record_json FROM verification_rules", [], |row| row.get::<_, String>(0)).unwrap(), "original-data");
    assert_eq!(backup.query_row("PRAGMA quick_check", [], |row| row.get::<_, String>(0)).unwrap(), "ok");
}

#[test]
fn unfinished_legacy_runs_keep_their_bytes_until_explicit_project_recovery() {
    for state in [RunState::Queued, RunState::Running, RunState::Cancelling] {
        let (_directory, path) = legacy();
        let mut connection = rusqlite::Connection::open(path.join(".gravity/workbench.sqlite")).unwrap();
        let run_id = append_legacy_run(&mut connection);
        let completed: String = connection.query_row("SELECT record_json FROM runs WHERE id=?1", [&run_id], |row| row.get(0)).unwrap();
        let mut run: RunRecord = serde_json::from_str(&completed).unwrap();
        run.state = state;
        if state == RunState::Queued { run.started_at = None; }
        run.finished_at = None;
        run.result = None;
        run.validation_status = ValidationStatus::NotRun;
        let original = serde_json::to_string(&run).unwrap();
        connection.execute_batch("DROP TRIGGER runs_transitions;").unwrap();
        let encoded_state = serde_json::to_string(&state).unwrap();
        connection.execute("UPDATE runs SET state=?1,record_json=?2 WHERE id=?3", (encoded_state.trim_matches('"'), &original, &run_id)).unwrap();
        let core = Workbench::new();
        let plan = core.prepare_project_migration(&path).unwrap();
        let receipt = core.apply_project_migration(&path, &plan.id).unwrap();
        assert_eq!(receipt.legacy_result_count, 0);
        assert_eq!(connection.query_row("SELECT record_json FROM runs WHERE id=?1", [&run_id], |row| row.get::<_, String>(0)).unwrap(), original);
        assert_eq!(connection.query_row("SELECT count(*) FROM result_identities", [], |row| row.get::<_, i64>(0)).unwrap(), 0);
        let backup = rusqlite::Connection::open(&receipt.backup_path).unwrap();
        assert_eq!(backup.query_row("SELECT record_json FROM runs WHERE id=?1", [&run_id], |row| row.get::<_, String>(0)).unwrap(), original);
        // 只有重新打开项目的现有恢复逻辑才把无句柄的活动记录标为Unknown。
        let recovered = core.open_project(&path).unwrap().runs.remove(0);
        assert_eq!(recovered.state, RunState::Unknown);
        assert_eq!(recovered.error.unwrap().code, "process_state_unknown");
    }
}

#[test]
fn commit_blocked_by_a_reader_rolls_back_the_schema_and_keeps_a_verified_backup() {
    let (_directory, path) = legacy();
    let core = Workbench::new();
    let plan = core.prepare_project_migration(&path).unwrap();
    let mut connection = rusqlite::Connection::open(path.join(".gravity/workbench.sqlite")).unwrap();
    let reader = connection.transaction().unwrap();
    // 回滚日志模式中的实际读锁允许备份与事务写入，但禁止最后提交取得排他锁。
    assert_eq!(reader.pragma_query_value(None, "journal_mode", |row| row.get::<_, String>(0)).unwrap(), "delete");
    assert_eq!(reader.query_row("SELECT count(*) FROM projects", [], |row| row.get::<_, i64>(0)).unwrap(), 1);
    assert_eq!(core.apply_project_migration(&path, &plan.id).unwrap_err().code, "migration_storage_error");
    reader.rollback().unwrap();
    assert_original_schema(&connection);
    let backup = rusqlite::Connection::open(&plan.backup_path).unwrap();
    assert_original_schema(&backup);
    assert_eq!(backup.query_row("PRAGMA quick_check", [], |row| row.get::<_, String>(0)).unwrap(), "ok");
}

#[test]
fn multi_step_backup_preserves_every_historical_preflight_and_its_diagnostics() {
    const HISTORICAL_PREFLIGHT_COUNT: usize = 192;
    const DIAGNOSTIC_REPETITIONS: usize = 1024;
    const BACKUP_STEP_PAGE_BUDGET: i64 = 128;
    let (_directory, path) = legacy();
    let mut connection = rusqlite::Connection::open(path.join(".gravity/workbench.sqlite")).unwrap();
    let run_id = append_legacy_run(&mut connection);
    let stored_run: String = connection.query_row("SELECT record_json FROM runs WHERE id=?1", [&run_id], |row| row.get(0)).unwrap();
    let run: RunRecord = serde_json::from_str(&stored_run).unwrap();
    let transaction = connection.transaction().unwrap();
    let mut originals = Vec::with_capacity(HISTORICAL_PREFLIGHT_COUNT);
    for _ in 0..HISTORICAL_PREFLIGHT_COUNT {
        // 多次环境准备失败会积累较长诊断；大项目备份不能只复制首批数据库页面。
        let report = PreflightReport {
            id: uuid::Uuid::new_v4().to_string(), project_id: run.project_id.clone(), model_version_id: run.model_version_id.clone(),
            config: run.request.config.clone(), environment: run.environment.clone(), created_at: workbench_core::storage::timestamp(),
            status: PreflightStatus::Blocked,
            issues: vec![workbench_core::CoreError::new("environment_probe_failed", "环境准备诊断；".repeat(DIAGNOSTIC_REPETITIONS))],
            execution_limits: ExecutionLimits::default(),
        };
        let encoded = serde_json::to_string(&report).unwrap();
        transaction.execute("INSERT INTO preflights(id,project_id,record_json) VALUES (?1,?2,?3)", (&report.id, &report.project_id, &encoded)).unwrap();
        originals.push((report.id, encoded));
    }
    transaction.commit().unwrap();
    assert!(connection.pragma_query_value(None, "page_count", |row| row.get::<_, i64>(0)).unwrap() > BACKUP_STEP_PAGE_BUDGET);
    let core = Workbench::new();
    let plan = core.prepare_project_migration(&path).unwrap();
    let receipt = core.apply_project_migration(&path, &plan.id).unwrap();
    let backup = rusqlite::Connection::open(&receipt.backup_path).unwrap();
    // SQLite整数以i64读取，比较前显式检查测试记录数量的转换范围。
    let expected_count = i64::try_from(HISTORICAL_PREFLIGHT_COUNT).unwrap();
    for database in [&connection, &backup] {
        assert_eq!(database.query_row("SELECT count(*) FROM preflights", [], |row| row.get::<_, i64>(0)).unwrap(), expected_count);
        for (id, original) in &originals {
            assert_eq!(database.query_row("SELECT record_json FROM preflights WHERE id=?1", [id], |row| row.get::<_, String>(0)).unwrap(), *original);
        }
    }
    assert_eq!(receipt.backup_sha256, sha256_bytes(&std::fs::read(&receipt.backup_path).unwrap()));
}
