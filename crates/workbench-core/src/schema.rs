//! 所有建表、触发器与格式标记在同一SQLite事务中完成。

use rusqlite::Connection;
use crate::{CoreError, CoreResult, limits::SCHEMA_VERSION, types::ProjectSummary};

pub const DATABASE_APPLICATION_ID: i32 = 0x4752_5742;

pub fn initialize(connection: &mut Connection, project: &ProjectSummary) -> CoreResult<()> {
    let transaction = connection.transaction().map_err(|_| database_error())?;
    transaction.execute_batch(
        "CREATE TABLE projects(id TEXT PRIMARY KEY, record_json TEXT NOT NULL);
         CREATE TABLE models(id TEXT PRIMARY KEY, project_id TEXT NOT NULL REFERENCES projects(id),
             label TEXT NOT NULL, created_at TEXT NOT NULL, record_json TEXT NOT NULL);
         CREATE TABLE preflights(id TEXT PRIMARY KEY, project_id TEXT NOT NULL REFERENCES projects(id),
             record_json TEXT NOT NULL, consumed INTEGER NOT NULL DEFAULT 0 CHECK(consumed IN (0,1)));
         CREATE TABLE runs(id TEXT PRIMARY KEY, project_id TEXT NOT NULL REFERENCES projects(id),
             model_id TEXT NOT NULL REFERENCES models(id), created_at TEXT NOT NULL,
             state TEXT NOT NULL, request_json TEXT NOT NULL, environment_json TEXT NOT NULL,
             record_json TEXT NOT NULL);
         CREATE TRIGGER models_no_update BEFORE UPDATE ON models BEGIN SELECT RAISE(ABORT,'immutable model'); END;
         CREATE TRIGGER models_no_delete BEFORE DELETE ON models BEGIN SELECT RAISE(ABORT,'immutable model'); END;
         CREATE TRIGGER runs_no_delete BEFORE DELETE ON runs BEGIN SELECT RAISE(ABORT,'immutable run'); END;
         CREATE TRIGGER runs_frozen BEFORE UPDATE ON runs
           WHEN OLD.id != NEW.id OR OLD.project_id != NEW.project_id OR OLD.model_id != NEW.model_id
             OR OLD.created_at != NEW.created_at OR OLD.request_json != NEW.request_json
             OR OLD.environment_json != NEW.environment_json
             OR json_extract(OLD.record_json,'$.request') != json_extract(NEW.record_json,'$.request')
             OR json_extract(OLD.record_json,'$.environment') != json_extract(NEW.record_json,'$.environment')
           BEGIN SELECT RAISE(ABORT,'immutable snapshot'); END;
         CREATE TRIGGER runs_transitions BEFORE UPDATE ON runs
           WHEN NOT ((OLD.state='queued' AND NEW.state IN ('running','failed','cancelling','unknown'))
             OR (OLD.state='running' AND NEW.state IN ('completed','failed','cancelling','unknown'))
             OR (OLD.state='cancelling' AND NEW.state IN ('cancelled','unknown')))
           BEGIN SELECT RAISE(ABORT,'invalid transition'); END;
         CREATE TRIGGER preflight_frozen BEFORE UPDATE ON preflights
           WHEN OLD.id != NEW.id OR OLD.project_id != NEW.project_id OR OLD.record_json != NEW.record_json
             OR OLD.consumed != 0 OR NEW.consumed != 1
           BEGIN SELECT RAISE(ABORT,'immutable preflight'); END;"
    ).map_err(|_| database_error())?;
    let record = serde_json::to_string(project).map_err(|_| database_error())?;
    transaction.execute("INSERT INTO projects(id,record_json) VALUES (?1,?2)", (&project.id, &record)).map_err(|_| database_error())?;
    transaction.execute_batch(&format!("PRAGMA application_id={DATABASE_APPLICATION_ID}; PRAGMA user_version={SCHEMA_VERSION};")).map_err(|_| database_error())?;
    transaction.commit().map_err(|_| database_error())
}

pub fn verify(connection: &Connection) -> CoreResult<()> {
    let app: i32 = connection.pragma_query_value(None, "application_id", |row| row.get(0)).map_err(|_| database_error())?;
    let version: u32 = connection.pragma_query_value(None, "user_version", |row| row.get(0)).map_err(|_| database_error())?;
    if app != DATABASE_APPLICATION_ID || version != SCHEMA_VERSION {
        return Err(CoreError::new("unsupported_project", "目录不是受支持的工作台项目，或项目格式版本不兼容"));
    }
    Ok(())
}

pub fn database_error() -> CoreError { CoreError::new("storage_error", "项目存储操作失败，原有事务未被部分提交") }

#[cfg(test)]
#[path = "schema_tests.rs"]
mod tests;
