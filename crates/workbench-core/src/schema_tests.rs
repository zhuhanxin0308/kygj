//! 真实SQLite故障注入测试，与生产实现分文件以保持业务覆盖率口径。

use super::*;
use uuid::Uuid;

fn project_fixture() -> ProjectSummary {
    ProjectSummary { id: Uuid::new_v4().to_string(), name: "事务故障注入".into(), path: "仅用于内存数据库测试".into(), created_at: "2026-10-06T00:00:00Z".into(), schema_version: SCHEMA_VERSION }
}

fn table_names(connection: &Connection) -> Vec<String> {
    let mut statement = connection.prepare("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name").unwrap();
    statement.query_map([], |row| row.get(0)).unwrap().collect::<Result<Vec<_>, _>>().unwrap()
}

#[test]
fn initialization_conflict_on_second_table_rolls_back_first_table() {
    let mut connection = Connection::open_in_memory().unwrap();
    // 真实SQLite冲突在第二张表触发，第一张已创建表必须随事务撤销。
    connection.execute_batch("CREATE TABLE models(original_value TEXT NOT NULL); INSERT INTO models VALUES ('保留原记录');").unwrap();
    let result = initialize(&mut connection, &project_fixture());
    assert_eq!(result.unwrap_err().code, "storage_error");
    assert_eq!(table_names(&connection), vec!["models"]);
    let existing: String = connection.query_row("SELECT original_value FROM models", [], |row| row.get(0)).unwrap();
    assert_eq!(existing, "保留原记录");
    let version: u32 = connection.pragma_query_value(None, "user_version", |row| row.get(0)).unwrap();
    let identity: i32 = connection.pragma_query_value(None, "application_id", |row| row.get(0)).unwrap();
    assert_eq!(version, 0);
    assert_eq!(identity, 0);
    assert!(connection.is_autocommit(), "失败事务必须已经结束，不能遗留写锁");
}

#[test]
fn late_trigger_conflict_rolls_back_all_new_tables_and_preserves_existing_format() {
    let mut connection = Connection::open_in_memory().unwrap();
    // 全部新表创建后才遇到同名触发器，验证后半段失败不会留下半套模式。
    connection.execute_batch(
        "CREATE TABLE sentinel(value INTEGER NOT NULL);
         INSERT INTO sentinel VALUES (42);
         CREATE TRIGGER models_no_update BEFORE INSERT ON sentinel BEGIN SELECT 1; END;
         PRAGMA user_version=7;
         PRAGMA application_id=123;"
    ).unwrap();
    let result = initialize(&mut connection, &project_fixture());
    assert_eq!(result.unwrap_err().code, "storage_error");
    assert_eq!(table_names(&connection), vec!["sentinel"]);
    let trigger_count: i64 = connection.query_row("SELECT count(*) FROM sqlite_master WHERE type='trigger' AND name='models_no_update' AND tbl_name='sentinel'", [], |row| row.get(0)).unwrap();
    let stored: i64 = connection.query_row("SELECT value FROM sentinel", [], |row| row.get(0)).unwrap();
    assert_eq!(trigger_count, 1);
    assert_eq!(stored, 42);
    let version: u32 = connection.pragma_query_value(None, "user_version", |row| row.get(0)).unwrap();
    let identity: i32 = connection.pragma_query_value(None, "application_id", |row| row.get(0)).unwrap();
    assert_eq!(version, 7);
    assert_eq!(identity, 123);
    assert!(connection.is_autocommit());
}
