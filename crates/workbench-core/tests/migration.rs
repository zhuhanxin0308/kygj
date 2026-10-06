//! v1迁移必须先预览，备份一致性状态，并在任何失败时保留原格式。
use workbench_core::{Workbench,types::ProjectSummary};
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
