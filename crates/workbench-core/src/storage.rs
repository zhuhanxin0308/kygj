//! 项目独占目录、追加版本、预检消费与不可变运行快照的真实SQLite实现。

use std::{fs::{self, OpenOptions}, io::Write, path::{Path, PathBuf}};
use chrono::{SecondsFormat, Utc};
use rusqlite::{Connection, OpenFlags, OptionalExtension, TransactionBehavior};
use serde::{Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};
use uuid::Uuid;
use crate::{CoreError, CoreResult, limits::{SCHEMA_VERSION, SQLITE_BUSY_TIMEOUT}, protocol, schema, types::*, validation::validate_project_name};

const METADATA_DIRECTORY: &str = ".gravity";
const DATABASE_FILE: &str = "workbench.sqlite";
const MAX_LABEL_BYTES: usize = 1024;

#[derive(Debug, Clone)]
pub struct ProjectStore { directory: PathBuf }

pub fn timestamp() -> String { Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true) }
pub fn sha256_bytes(bytes: &[u8]) -> String { Sha256::digest(bytes).iter().map(|byte| format!("{byte:02x}")).collect() }
fn encode<T: Serialize>(value: &T) -> CoreResult<String> { serde_json::to_string(value).map_err(|_| schema::database_error()) }
fn decode<T: DeserializeOwned>(value: &str) -> CoreResult<T> { serde_json::from_str(value).map_err(|_| CoreError::new("corrupt_project", "项目记录损坏或与当前格式不兼容")) }
fn io_error() -> CoreError { CoreError::new("file_access_error", "无法访问已选择的项目目录或写入所需文件") }

/// 拒绝项目内部元数据链接，防止导入目录把数据库写入重定向到无关位置。
fn reject_link(path: &Path) -> CoreResult<()> {
    let metadata = fs::symlink_metadata(path).map_err(|_| io_error())?;
    let mut linked = metadata.file_type().is_symlink();
    #[cfg(windows)] {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
        linked |= metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0;
    }
    if linked { return Err(CoreError::new("unsafe_project_path", "项目元数据不能通过符号链接或重解析点重定向")); }
    Ok(())
}

impl ProjectStore {
    pub fn create(parent: &Path, name: &str) -> CoreResult<Self> {
        validate_project_name(name)?;
        let parent = parent.canonicalize().map_err(|_| io_error())?;
        if !parent.is_dir() { return Err(io_error()); }
        let directory = parent.join(name);
        // create_dir是排他创建；绝不使用create_dir_all去复用同名现有项目。
        fs::create_dir(&directory).map_err(|e| if e.kind() == std::io::ErrorKind::AlreadyExists {
            CoreError::new("project_exists", "同名目录已经存在，未覆盖原有内容")
        } else { io_error() })?;
        let metadata = directory.join(METADATA_DIRECTORY);
        fs::create_dir(&metadata).map_err(|_| io_error())?;
        let store = Self { directory };
        let mut connection = Connection::open(store.database_path()).map_err(|_| schema::database_error())?;
        configure(&connection)?;
        let project = ProjectSummary { id: Uuid::new_v4().to_string(), name: name.into(), path: store.directory.to_string_lossy().into(), created_at: timestamp(), schema_version: SCHEMA_VERSION };
        schema::initialize(&mut connection, &project)?;
        Ok(store)
    }

    pub fn open(directory: &Path) -> CoreResult<Self> {
        let directory = directory.canonicalize().map_err(|_| io_error())?;
        if !directory.is_dir() { return Err(io_error()); }
        reject_link(&directory.join(METADATA_DIRECTORY))?;
        reject_link(&directory.join(METADATA_DIRECTORY).join(DATABASE_FILE))?;
        let store = Self { directory };
        let connection = store.connection()?;
        schema::verify(&connection)?;
        store.summary()?;
        Ok(store)
    }

    pub fn directory(&self) -> &Path { &self.directory }
    fn database_path(&self) -> PathBuf { self.directory.join(METADATA_DIRECTORY).join(DATABASE_FILE) }
    fn connection(&self) -> CoreResult<Connection> {
        reject_link(&self.directory.join(METADATA_DIRECTORY))?;
        reject_link(&self.database_path())?;
        let connection = Connection::open_with_flags(self.database_path(), OpenFlags::SQLITE_OPEN_READ_WRITE).map_err(|_| schema::database_error())?;
        configure(&connection)?;
        schema::verify(&connection)?;
        Ok(connection)
    }

    pub fn summary(&self) -> CoreResult<ProjectSummary> {
        let connection = self.connection()?;
        let count: i64 = connection.query_row("SELECT count(*) FROM projects", [], |row| row.get(0)).map_err(|_| schema::database_error())?;
        if count != 1 { return Err(CoreError::new("corrupt_project", "项目身份记录不唯一")); }
        let json: String = connection.query_row("SELECT record_json FROM projects", [], |row| row.get(0)).map_err(|_| schema::database_error())?;
        let mut project: ProjectSummary = decode(&json)?;
        Uuid::parse_str(&project.id).map_err(|_| CoreError::new("corrupt_project", "项目身份无效"))?;
        validate_project_name(&project.name)?;
        if project.schema_version != SCHEMA_VERSION { return Err(CoreError::new("unsupported_project", "项目格式版本不受支持")); }
        // 项目整体移动后保留身份，只更新返回给界面的实际当前位置。
        project.path = self.directory.to_string_lossy().into();
        Ok(project)
    }

    pub fn state(&self) -> CoreResult<ProjectState> {
        let connection = self.connection()?;
        let project = self.summary()?;
        let mut models = connection.prepare("SELECT id,record_json FROM models ORDER BY rowid DESC").map_err(|_| schema::database_error())?;
        let model_json = models.query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))).map_err(|_| schema::database_error())?;
        let model_records = model_json.map(|row| {
            let (id, json) = row.map_err(|_| schema::database_error())?;
            let model: ModelVersion = decode(&json)?;
            if model.id != id { return Err(corrupt()); }
            Ok(model)
        }).collect::<CoreResult<Vec<ModelVersion>>>()?;
        let mut runs = connection.prepare("SELECT id,record_json FROM runs ORDER BY rowid DESC").map_err(|_| schema::database_error())?;
        let run_json = runs.query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))).map_err(|_| schema::database_error())?;
        let run_records = run_json.map(|row| {
            let (id, json) = row.map_err(|_| schema::database_error())?;
            let run: RunRecord = decode(&json)?;
            if run.id != id { return Err(corrupt()); }
            Ok(run)
        }).collect::<CoreResult<Vec<RunRecord>>>()?;
        for model in &model_records { validate_model_record(&connection, model, &project.id)?; }
        for run in &run_records { validate_run_record(&connection, run, &project.id)?; }
        Ok(ProjectState { project, models: model_records, runs: run_records })
    }

    pub fn save_model(&self, label: &str, config: EllisConfig) -> CoreResult<ModelVersion> {
        config.validate()?;
        if label.trim().is_empty() || label.len() > MAX_LABEL_BYTES || label.chars().any(char::is_control) { return Err(CoreError::new("invalid_model_label", "模型版本标签不能为空、超长或包含控制字符")); }
        let project = self.summary()?;
        let content_hash = sha256_bytes(encode(&config)?.as_bytes());
        let model = ModelVersion { id: Uuid::new_v4().to_string(), project_id: project.id, label: label.into(), created_at: timestamp(), config, content_hash };
        let mut connection = self.connection()?;
        let transaction = connection.transaction().map_err(|_| schema::database_error())?;
        transaction.execute("INSERT INTO models(id,project_id,label,created_at,record_json) VALUES (?1,?2,?3,?4,?5)",
            (&model.id, &model.project_id, &model.label, &model.created_at, encode(&model)?)).map_err(|_| schema::database_error())?;
        transaction.commit().map_err(|_| schema::database_error())?;
        Ok(model)
    }

    pub fn model(&self, id: &str) -> CoreResult<ModelVersion> {
        let connection = self.connection()?;
        let model: ModelVersion = read_record(&connection, "SELECT record_json FROM models WHERE id=?1", id)?;
        if model.id != id { return Err(corrupt()); }
        validate_model_record(&connection, &model, &self.summary()?.id)?;
        Ok(model)
    }

    pub fn check_writable(&self) -> CoreResult<()> {
        let path = self.directory.join(METADATA_DIRECTORY).join(format!("write-probe-{}", Uuid::new_v4()));
        let mut file = OpenOptions::new().write(true).create_new(true).open(&path).map_err(|_| io_error())?;
        let outcome = file.write_all(b"workbench-write-probe").and_then(|_| file.sync_all()).map_err(|_| io_error());
        drop(file);
        let cleanup = fs::remove_file(path).map_err(|_| io_error());
        outcome.and(cleanup)
    }

    pub fn prepare(&self, model_id: &str, environment: EnvironmentInfo, execution_limits: ExecutionLimits, mut issues: Vec<CoreError>) -> CoreResult<PreflightReport> {
        let model = self.model(model_id)?;
        if let Err(error) = self.check_writable() { issues.push(error); }
        let report = PreflightReport { id: Uuid::new_v4().to_string(), project_id: model.project_id, model_version_id: model.id,
            config: model.config, environment, created_at: timestamp(), status: if issues.is_empty() { PreflightStatus::Ready } else { PreflightStatus::Blocked }, issues, execution_limits };
        let mut connection = self.connection()?;
        let transaction = connection.transaction().map_err(|_| schema::database_error())?;
        transaction.execute("INSERT INTO preflights(id,project_id,record_json) VALUES (?1,?2,?3)", (&report.id, &report.project_id, encode(&report)?)).map_err(|_| schema::database_error())?;
        transaction.commit().map_err(|_| schema::database_error())?;
        Ok(report)
    }

    pub fn preflight(&self, id: &str) -> CoreResult<PreflightReport> {
        let connection = self.connection()?;
        let report = read_record(&connection, "SELECT record_json FROM preflights WHERE id=?1", id)?;
        validate_preflight(&connection, &report, id, &self.summary()?.id)?;
        Ok(report)
    }

    pub fn consume_preflight(&self, id: &str, current_environment: &EnvironmentInfo) -> CoreResult<RunRecord> {
        let mut connection = self.connection()?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate).map_err(|_| schema::database_error())?;
        let report: PreflightReport = read_record(&transaction, "SELECT record_json FROM preflights WHERE id=?1", id)?;
        validate_preflight(&transaction, &report, id, &self.summary()?.id)?;
        let consumed: i64 = transaction.query_row("SELECT consumed FROM preflights WHERE id=?1", [id], |row| row.get(0)).map_err(|_| schema::database_error())?;
        if consumed != 0 { return Err(CoreError::new("preflight_consumed", "预检已经用于提交，不能重复执行")); }
        if report.status != PreflightStatus::Ready || !report.issues.is_empty() { return Err(CoreError::new("preflight_blocked", "预检存在阻断项，不能提交")); }
        let model: ModelVersion = read_record(&transaction, "SELECT record_json FROM models WHERE id=?1", &report.model_version_id)?;
        if model.config != report.config || &report.environment != current_environment { return Err(CoreError::new("preflight_stale", "模型或执行环境已改变，请重新预检")); }
        report.config.validate()?;
        let run = RunRecord { id: Uuid::new_v4().to_string(), project_id: report.project_id, model_version_id: report.model_version_id,
            created_at: timestamp(), started_at: None, finished_at: None, state: RunState::Queued, validation_status: ValidationStatus::NotRun,
            request: TraceRequest::new(Uuid::new_v4().to_string(), report.config), environment: report.environment, result: None, error: None };
        transaction.execute("UPDATE preflights SET consumed=1 WHERE id=?1", [id]).map_err(|_| schema::database_error())?;
        transaction.execute("INSERT INTO runs(id,project_id,model_id,created_at,state,request_json,environment_json,record_json) VALUES (?1,?2,?3,?4,'queued',?5,?6,?7)",
            (&run.id, &run.project_id, &run.model_version_id, &run.created_at, encode(&run.request)?, encode(&run.environment)?, encode(&run)?)).map_err(|_| schema::database_error())?;
        transaction.commit().map_err(|_| schema::database_error())?;
        Ok(run)
    }

    pub fn run(&self, id: &str) -> CoreResult<RunRecord> {
        let connection = self.connection()?;
        let run: RunRecord = read_record(&connection, "SELECT record_json FROM runs WHERE id=?1", id)?;
        if run.id != id { return Err(corrupt()); }
        validate_run_record(&connection, &run, &self.summary()?.id)?;
        Ok(run)
    }

    pub fn transition(&self, id: &str, next: RunState, result: Option<TraceResult>, error: Option<CoreError>) -> CoreResult<RunRecord> {
        let mut connection = self.connection()?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate).map_err(|_| schema::database_error())?;
        let mut run: RunRecord = read_record(&transaction, "SELECT record_json FROM runs WHERE id=?1", id)?;
        if run.id != id { return Err(corrupt()); }
        validate_run_record(&transaction, &run, &self.summary()?.id)?;
        if !run.state.permits(next) { return Err(CoreError::new("invalid_run_transition", "运行状态不允许该操作，原有终态不会改写")); }
        if next == RunState::Completed {
            let response = result.as_ref().ok_or_else(|| CoreError::new("missing_result", "缺少合法计算结果，不能登记完成"))?;
            protocol::validate_result(response, &run.request, &run.environment)?;
            run.validation_status = response.validation_status();
        } else if result.is_some() { return Err(CoreError::new("invalid_run_result", "当前执行状态不能登记完整结果")); }
        run.state = next;
        if next == RunState::Running { run.started_at = Some(timestamp()); }
        if next.terminal() { run.finished_at = Some(timestamp()); }
        run.result = result;
        run.error = error;
        validate_run_payload(&run)?;
        let state = encode(&next)?;
        transaction.execute("UPDATE runs SET state=?1,record_json=?2 WHERE id=?3", (state.trim_matches('"'), encode(&run)?, id)).map_err(|_| schema::database_error())?;
        transaction.commit().map_err(|_| schema::database_error())?;
        Ok(run)
    }

    pub fn run_directory(&self, run_id: &str) -> CoreResult<PathBuf> {
        let id = Uuid::parse_str(run_id).map_err(|_| CoreError::new("invalid_run_id", "运行身份无效"))?;
        let root = self.directory.join(METADATA_DIRECTORY).join("runs");
        if !root.exists() { fs::create_dir(&root).map_err(|_| io_error())?; }
        reject_link(&root)?;
        let path = root.join(id.to_string());
        fs::create_dir(&path).map_err(|_| io_error())?;
        Ok(path)
    }

    pub fn export_run(&self, run_id: &str, destination: &Path) -> CoreResult<ExportedRun> {
        let run = self.run(run_id)?;
        let id = Uuid::parse_str(&run.id).map_err(|_| CoreError::new("corrupt_project", "运行身份无效，不能导出"))?;
        let destination = destination.canonicalize().map_err(|_| io_error())?;
        if !destination.is_dir() { return Err(io_error()); }
        let payload = serde_json::json!({"schemaVersion":SCHEMA_VERSION,"project":self.summary()?,"model":self.model(&run.model_version_id)?,"run":run});
        let bytes = serde_json::to_vec_pretty(&payload).map_err(|_| schema::database_error())?;
        let path = destination.join(format!("run-{id}.json"));
        let mut file = OpenOptions::new().write(true).create_new(true).open(&path).map_err(|e| if e.kind() == std::io::ErrorKind::AlreadyExists {
            CoreError::new("export_exists", "导出文件已经存在，未覆盖原文件")
        } else { io_error() })?;
        let written = file.write_all(&bytes).and_then(|_| file.sync_all());
        drop(file);
        if written.is_err() { let _ = fs::remove_file(&path); return Err(io_error()); }
        Ok(ExportedRun { path: path.to_string_lossy().into(), sha256: sha256_bytes(&bytes) })
    }
}

fn configure(connection: &Connection) -> CoreResult<()> {
    connection.busy_timeout(SQLITE_BUSY_TIMEOUT).map_err(|_| schema::database_error())?;
    connection.pragma_update(None, "foreign_keys", true).map_err(|_| schema::database_error())?;
    Ok(())
}

fn read_record<T: DeserializeOwned>(connection: &Connection, sql: &str, id: &str) -> CoreResult<T> {
    let record: Option<String> = connection.query_row(sql, [id], |row| row.get(0)).optional().map_err(|_| schema::database_error())?;
    decode(&record.ok_or_else(|| CoreError::new("record_not_found", "当前项目中不存在所需记录"))?)
}

fn corrupt() -> CoreError { CoreError::new("corrupt_project", "项目记录的身份、冻结内容或结果关联不一致，已拒绝使用") }

fn valid_time(value: &str) -> bool {
    chrono::DateTime::parse_from_rfc3339(value).is_ok_and(|time| time.offset().local_minus_utc() == 0)
}

fn valid_environment(environment: &EnvironmentInfo) -> bool {
    [&environment.python_executable, &environment.engine_version, &environment.python_version, &environment.numpy_version, &environment.scipy_version]
        .iter().all(|s| !s.trim().is_empty() && !s.chars().any(char::is_control))
        && environment.engine_source_hash.len() == 64 && environment.engine_source_hash.bytes().all(|b| b.is_ascii_hexdigit())
}

/// 同时验证关系列与JSON内部身份；SQLite外键不能替代这一步。
fn validate_model_record(connection: &Connection, model: &ModelVersion, project_id: &str) -> CoreResult<()> {
    let (stored_project, label, created_at): (String, String, String) = connection.query_row(
        "SELECT project_id,label,created_at FROM models WHERE id=?1", [&model.id], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?))
    ).map_err(|_| corrupt())?;
    if Uuid::parse_str(&model.id).is_err() || model.project_id != project_id || stored_project != project_id
        || model.label != label || model.created_at != created_at || !valid_time(&model.created_at)
        || model.label.trim().is_empty() || model.config.validate().is_err()
        || sha256_bytes(encode(&model.config)?.as_bytes()) != model.content_hash { return Err(corrupt()); }
    Ok(())
}

fn validate_preflight(connection: &Connection, report: &PreflightReport, id: &str, project_id: &str) -> CoreResult<()> {
    let stored_project: String = connection.query_row("SELECT project_id FROM preflights WHERE id=?1", [id], |row| row.get(0)).map_err(|_| corrupt())?;
    let model: ModelVersion = read_record(connection, "SELECT record_json FROM models WHERE id=?1", &report.model_version_id)?;
    validate_model_record(connection, &model, project_id)?;
    if model.id != report.model_version_id || report.id != id || report.project_id != project_id || stored_project != project_id
        || !valid_time(&report.created_at) || !valid_environment(&report.environment)
        || encode(&report.config)? != encode(&model.config)?
        || (report.status == PreflightStatus::Ready) != report.issues.is_empty()
        || report.execution_limits.max_wall_time_seconds == 0
        || report.execution_limits.max_output_bytes == 0
        || report.execution_limits.max_output_bytes > crate::limits::MAX_RESPONSE_BYTES
        || report.execution_limits.max_total_samples != crate::limits::MAX_TOTAL_SAMPLES { return Err(corrupt()); }
    Ok(())
}

fn validate_run_payload(run: &RunRecord) -> CoreResult<()> {
    if Uuid::parse_str(&run.id).is_err() || !valid_time(&run.created_at)
        || run.started_at.as_ref().is_some_and(|t| !valid_time(t))
        || run.finished_at.as_ref().is_some_and(|t| !valid_time(t))
        || run.request.protocol_version != crate::limits::PROTOCOL_VERSION || run.request.action != "traceEllis"
        || run.request.request_id.trim().is_empty() || run.request.config.validate().is_err()
        || !valid_environment(&run.environment) { return Err(corrupt()); }
    if run.state.terminal() != run.finished_at.is_some() { return Err(corrupt()); }
    if matches!(run.state, RunState::Running | RunState::Completed) && run.started_at.is_none() { return Err(corrupt()); }
    if run.state == RunState::Completed {
        let result = run.result.as_ref().ok_or_else(corrupt)?;
        protocol::validate_result(result, &run.request, &run.environment).map_err(|_| corrupt())?;
        if run.validation_status != result.validation_status() || run.error.is_some() { return Err(corrupt()); }
    } else if run.result.is_some() || run.validation_status != ValidationStatus::NotRun { return Err(corrupt()); }
    if run.state == RunState::Failed && run.error.is_none() { return Err(corrupt()); }
    Ok(())
}

fn validate_run_record(connection: &Connection, run: &RunRecord, project_id: &str) -> CoreResult<()> {
    validate_run_payload(run)?;
    let (stored_project, model_id, created_at, state, request, environment): (String, String, String, String, String, String) = connection.query_row(
        "SELECT project_id,model_id,created_at,state,request_json,environment_json FROM runs WHERE id=?1", [&run.id],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?))
    ).map_err(|_| corrupt())?;
    let model: ModelVersion = read_record(connection, "SELECT record_json FROM models WHERE id=?1", &run.model_version_id)?;
    validate_model_record(connection, &model, project_id)?;
    if model.id != run.model_version_id || run.project_id != project_id || stored_project != project_id || run.model_version_id != model_id
        || run.created_at != created_at || encode(&run.state)?.trim_matches('"') != state
        || encode(&run.request)? != request || encode(&run.environment)? != environment
        || encode(&run.request.config)? != encode(&model.config)? { return Err(corrupt()); }
    Ok(())
}
