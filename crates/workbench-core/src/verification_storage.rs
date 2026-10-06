//! 验证账本追加存储与内容身份；任何重检都不能改写运行或旧验证记录。
use rusqlite::{Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::{CoreError, CoreResult, schema, storage::{ProjectStore, timestamp}, types::*, verification::*};

pub(crate) fn initialize(connection: &Connection, project_id: &str) -> CoreResult<()> {
    connection.execute_batch("CREATE TABLE result_identities(run_id TEXT PRIMARY KEY REFERENCES runs(id),record_json TEXT NOT NULL);
        CREATE TABLE verification_rules(id TEXT PRIMARY KEY,project_id TEXT NOT NULL REFERENCES projects(id),parent_id TEXT REFERENCES verification_rules(id),record_json TEXT NOT NULL);
        CREATE TABLE verification_requests(id TEXT PRIMARY KEY,project_id TEXT NOT NULL REFERENCES projects(id),run_id TEXT NOT NULL REFERENCES runs(id),rule_id TEXT NOT NULL REFERENCES verification_rules(id),client_id TEXT NOT NULL,record_json TEXT NOT NULL,content_hash TEXT NOT NULL,UNIQUE(project_id,client_id));
        CREATE TABLE verification_records(id TEXT PRIMARY KEY,request_id TEXT NOT NULL UNIQUE REFERENCES verification_requests(id),project_id TEXT NOT NULL REFERENCES projects(id),run_id TEXT NOT NULL REFERENCES runs(id),rule_id TEXT NOT NULL REFERENCES verification_rules(id),record_json TEXT NOT NULL);
        CREATE INDEX verification_records_run ON verification_records(project_id,run_id);
        CREATE INDEX verification_records_rule ON verification_records(project_id,run_id,rule_id);
        CREATE INDEX verification_requests_run ON verification_requests(project_id,run_id);
        CREATE TABLE migration_history(id TEXT PRIMARY KEY,record_json TEXT NOT NULL);") .map_err(|_| schema::database_error())?;
    // 表名均为固定内部常量，不接受外部标识符拼接。
    for table in ["result_identities","verification_rules","verification_requests","verification_records","migration_history"] {
        connection.execute_batch(&format!("CREATE TRIGGER {table}_immutable_update BEFORE UPDATE ON {table} BEGIN SELECT RAISE(ABORT,'immutable verification'); END; CREATE TRIGGER {table}_immutable_delete BEFORE DELETE ON {table} BEGIN SELECT RAISE(ABORT,'immutable verification'); END;")).map_err(|_| schema::database_error())?;
    }
    insert_rule(connection, &builtin_rule(project_id)?)
}

fn corrupt() -> CoreError { CoreError::new("corrupt_verification", "验证账本身份、规则或内容校验失败，已停止使用") }
fn json<T: Serialize>(value: &T) -> CoreResult<String> { serde_json::to_string(value).map_err(|_| corrupt()) }
fn decode<T: serde::de::DeserializeOwned>(value: &str) -> CoreResult<T> { serde_json::from_str(value).map_err(|_| corrupt()) }

pub(crate) fn capture_result_identity(connection: &Connection, run: &RunRecord, origin: ResultIdentityOrigin) -> CoreResult<()> {
    if let Some(result) = &run.result {
        let source = VerificationSourceIdentity { request_hash:typed_hash(&run.request)?,environment_hash:typed_hash(&run.environment)?,
            result_hash:Some(typed_hash(result)?),hash_format:HASH_FORMAT.into(),result_origin:origin };
        connection.execute("INSERT INTO result_identities(run_id,record_json) VALUES (?1,?2)", (&run.id,json(&source)?)).map_err(|_| schema::database_error())?;
    }
    Ok(())
}

pub(crate) fn source_identity(connection: &Connection, run: &RunRecord) -> CoreResult<VerificationSourceIdentity> {
    let row: Option<String> = connection.query_row("SELECT record_json FROM result_identities WHERE run_id=?1", [&run.id], |row| row.get(0)).optional().map_err(|_| corrupt())?;
    let expected = VerificationSourceIdentity {request_hash:typed_hash(&run.request)?,environment_hash:typed_hash(&run.environment)?,
        result_hash:run.result.as_ref().map(typed_hash).transpose()?,hash_format:HASH_FORMAT.into(),result_origin:ResultIdentityOrigin::NoResult};
    if run.result.is_none() { if row.is_some() {return Err(corrupt());} return Ok(expected); }
    let actual: VerificationSourceIdentity = decode(&row.ok_or_else(corrupt)?)?;
    if actual.request_hash != expected.request_hash || actual.environment_hash != expected.environment_hash || actual.result_hash != expected.result_hash
        || actual.hash_format != HASH_FORMAT || actual.result_origin == ResultIdentityOrigin::NoResult { return Err(CoreError::new("result_integrity_failed", "运行结果内容与已登记SHA-256不一致，已阻止读取及重检")); }
    Ok(actual)
}

fn insert_rule(connection: &Connection, rule: &VerificationRuleVersion) -> CoreResult<()> {
    validate_rule(rule)?;
    connection.execute("INSERT INTO verification_rules(id,project_id,parent_id,record_json) VALUES (?1,?2,?3,?4)",(&rule.id,&rule.project_id,&rule.parent_version_id,json(rule)?)).map_err(|_| schema::database_error())?; Ok(())
}
fn read_rule(connection: &Connection, project: &str, id: &str) -> CoreResult<VerificationRuleVersion> {
    let row: Option<(String,Option<String>,String)> = connection.query_row("SELECT project_id,parent_id,record_json FROM verification_rules WHERE id=?1",[id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional().map_err(|_| corrupt())?;
    let (stored_project,parent,json) = row.ok_or_else(|| CoreError::new("record_not_found","当前项目中不存在该验证规则"))?;
    let rule: VerificationRuleVersion = decode(&json)?; validate_rule(&rule)?;
    if rule.id != id || rule.project_id != project || stored_project != project || rule.parent_version_id != parent {return Err(corrupt());}
    let mut ancestry=std::collections::HashSet::from([rule.id.clone()]);let mut current=rule.clone();
    while let Some(parent)=&current.parent_version_id {
        if !ancestry.insert(parent.clone()) || current.builtin {return Err(corrupt());}
        let parent_row:(String,Option<String>,String)=connection.query_row("SELECT project_id,parent_id,record_json FROM verification_rules WHERE id=?1",[parent],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).map_err(|_|corrupt())?;
        let base:VerificationRuleVersion=decode(&parent_row.2)?;validate_rule(&base)?;
        if base.id!=*parent || base.project_id!=project || parent_row.0!=project || base.parent_version_id!=parent_row.1 || base.rule_family_id!=rule.rule_family_id{return Err(corrupt());}
        current=base;
    }
    if !current.builtin || current.rule_family_id!=current.id{return Err(corrupt());}
    Ok(rule)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
struct SavedRequest {id:String,project_id:String,request:ExecuteVerification,started_at:String,source:VerificationSourceIdentity}

fn read_request(connection: &Connection, id: &str) -> CoreResult<SavedRequest> {
    let (project,run,rule,client,json,hash):(String,String,String,String,String,String)=connection.query_row("SELECT project_id,run_id,rule_id,client_id,record_json,content_hash FROM verification_requests WHERE id=?1",[id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?))).map_err(|_|corrupt())?;
    let request:SavedRequest=decode(&json)?;
    if request.id!=id || request.project_id!=project || request.request.run_id!=run || request.request.rule_version_id!=rule || request.request.client_request_id!=client || typed_hash(&request)?!=hash {return Err(corrupt());}
    Ok(request)
}

fn read_record(connection: &Connection, project: &str, id: &str) -> CoreResult<VerificationRecord> {
    let row:Option<(String,String,String,String,String)>=connection.query_row("SELECT project_id,run_id,rule_id,request_id,record_json FROM verification_records WHERE id=?1",[id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?))).optional().map_err(|_|corrupt())?;
    let (stored_project,run,rule_id,request_id,json)=row.ok_or_else(||CoreError::new("record_not_found","当前项目中不存在该验证记录"))?;
    let record:VerificationRecord=decode(&json)?; let request=read_request(connection,&request_id)?;
    let rule=read_rule(connection,project,&rule_id)?;
    if record.id!=id || record.project_id!=project || stored_project!=project || record.run_id!=run || record.rule_version_id!=rule_id
        || record.request_id!=request_id || request.project_id!=project || request.request.run_id!=run || request.request.rule_version_id!=rule_id
        || record.client_request_id!=request.request.client_request_id || record.previous_record_id!=request.request.previous_record_id || record.source!=request.source
        || record.started_at!=request.started_at || record.method_id!=rule.method_id || record.method_version!=rule.method_version || record.content_hash!=record_hash(&record)?
        || (record.execution_status!=VerificationExecutionStatus::Completed && record.conclusion==VerificationConclusion::Passed)
        || (record.execution_status==VerificationExecutionStatus::Completed && record.conclusion!=aggregate(&record.checks)) {return Err(corrupt());}
    for check in &record.checks {
        let definition=rule.checks.iter().find(|c|c.metric_id==check.metric_id).ok_or_else(corrupt)?;
        if check.threshold!=definition.threshold || check.basis!=definition.basis || check.unit!=definition.unit || check.title!=definition.title
            || check.actual.is_some_and(|v|!v.is_finite() || v<0.0) || (check.conclusion==VerificationConclusion::Passed && (check.actual.is_none_or(|v|v>check.threshold) || check.evidence_paths.is_empty())) {return Err(corrupt());}
    }
    let run_json:String=connection.query_row("SELECT record_json FROM runs WHERE id=?1",[&record.run_id],|r|r.get(0)).map_err(|_|corrupt())?;
    let source_run:RunRecord=decode(&run_json)?;crate::storage::validate_run_record(connection,&source_run,project)?;
    if source_run.id!=record.run_id || source_identity(connection,&source_run)?!=record.source{return Err(corrupt());}
    Ok(record)
}

fn append_record(connection: &Connection, record: &VerificationRecord) -> CoreResult<()> {
    connection.execute("INSERT INTO verification_records(id,request_id,project_id,run_id,rule_id,record_json) VALUES (?1,?2,?3,?4,?5,?6)",(&record.id,&record.request_id,&record.project_id,&record.run_id,&record.rule_version_id,json(record)?)).map_err(|_|schema::database_error())?; Ok(())
}

impl ProjectStore {
    /// 操作系统文件锁覆盖请求提交至完成提交；进程退出自动释放，重开可据此确认无活动验证者。
    fn verification_lock(&self)->CoreResult<std::fs::File>{
        let path=self.directory().join(".gravity/verification.lock");
        crate::storage::reject_link(&self.directory().join(".gravity"))?;
        if path.exists(){crate::storage::reject_link(&path)?;}
        let file=std::fs::OpenOptions::new().read(true).write(true).create(true).truncate(false).open(&path).map_err(|_|CoreError::new("verification_lock_failed","无法访问验证账本锁"))?;
        file.try_lock().map_err(|_|CoreError::new("verification_busy","另一个验证操作尚未释放账本，请稍后重试"))?;Ok(file)
    }

    pub(crate) fn recover_verification_requests(&self)->CoreResult<()> {
        let _guard=match self.verification_lock(){Ok(guard)=>guard,Err(error) if error.code=="verification_busy"=>return Ok(()),Err(error)=>return Err(error)};
        let project=self.summary()?.id;let mut connection=self.connection()?;
        let transaction=connection.transaction_with_behavior(TransactionBehavior::Immediate).map_err(|_|schema::database_error())?;
        let mut statement=transaction.prepare("SELECT q.id FROM verification_requests q WHERE q.project_id=?1 AND NOT EXISTS(SELECT 1 FROM verification_records v WHERE v.request_id=q.id) ORDER BY q.rowid").map_err(|_|corrupt())?;
        let ids=statement.query_map([&project],|r|r.get::<_,String>(0)).map_err(|_|corrupt())?.collect::<Result<Vec<_>,_>>().map_err(|_|corrupt())?;drop(statement);
        for id in ids {
            let saved=read_request(&transaction,&id)?;let rule=read_rule(&transaction,&project,&saved.request.rule_version_id)?;
            let run=self.run(&saved.request.run_id)?;
            if source_identity(&transaction,&run)?!=saved.source{return Err(corrupt());}
            let mut record=VerificationRecord {id:Uuid::new_v4().to_string(),project_id:project.clone(),request_id:saved.id,
                client_request_id:saved.request.client_request_id,run_id:saved.request.run_id,rule_version_id:saved.request.rule_version_id,previous_record_id:saved.request.previous_record_id,
                started_at:saved.started_at,finished_at:timestamp(),executed_by:"host_recovery".into(),method_id:rule.method_id,method_version:rule.method_version,
                execution_status:VerificationExecutionStatus::Interrupted,conclusion:VerificationConclusion::Inconclusive,source:saved.source,checks:vec![],
                error:Some(CoreError::new("verification_interrupted","先前验证请求没有完成记录，执行锁已释放；已保留中断事实，未自动重检")),content_hash:String::new()};
            record.content_hash=record_hash(&record)?;append_record(&transaction,&record)?;
        }
        transaction.commit().map_err(|_|schema::database_error())?;Ok(())
    }
    pub fn list_verification_rules(&self, offset:usize, limit:usize) -> CoreResult<VerificationRulePage> {
        page_valid(offset,limit)?; let project=self.summary()?.id; let connection=self.connection()?;
        let total:i64=connection.query_row("SELECT count(*) FROM verification_rules WHERE project_id=?1",[&project],|r|r.get(0)).map_err(|_|corrupt())?;
        let total=usize::try_from(total).map_err(|_|corrupt())?;
        let mut stmt=connection.prepare("SELECT id FROM verification_rules WHERE project_id=?1 ORDER BY rowid ASC LIMIT ?2 OFFSET ?3").map_err(|_|corrupt())?;
        let ids=stmt.query_map((&project,limit as i64,offset as i64),|r|r.get::<_,String>(0)).map_err(|_|corrupt())?.collect::<Result<Vec<_>,_>>().map_err(|_|corrupt())?;
        let rules=ids.iter().map(|id|read_rule(&connection,&project,id)).collect::<CoreResult<Vec<_>>>()?;
        let next=offset.saturating_add(rules.len()); Ok(VerificationRulePage {rules,total,next_offset:(next<total).then_some(next)})
    }

    pub fn save_verification_rule_version(&self, draft:VerificationRuleDraft) -> CoreResult<VerificationRuleVersion> {
        let project=self.summary()?.id; let mut connection=self.connection()?;
        let transaction=connection.transaction_with_behavior(TransactionBehavior::Immediate).map_err(|_|schema::database_error())?;
        let base=read_rule(&transaction,&project,&draft.base_version_id)?;
        let rule=derive_rule(&base,draft)?; insert_rule(&transaction,&rule)?; transaction.commit().map_err(|_|schema::database_error())?; Ok(rule)
    }

    pub fn get_verification_record(&self, id:&str) -> CoreResult<VerificationRecord> {
        let project=self.summary()?.id; let connection=self.connection()?;
        let record=read_record(&connection,&project,id)?; self.run(&record.run_id)?; Ok(record)
    }

    pub fn list_verification_records(&self, run:&str, offset:usize, limit:usize) -> CoreResult<VerificationRecordPage> {
        page_valid(offset,limit)?; self.run(run)?; let project=self.summary()?.id; let connection=self.connection()?;
        let total:i64=connection.query_row("SELECT count(*) FROM verification_records WHERE project_id=?1 AND run_id=?2",(&project,run),|r|r.get(0)).map_err(|_|corrupt())?;
        let total=usize::try_from(total).map_err(|_|corrupt())?;
        let mut stmt=connection.prepare("SELECT id FROM verification_records WHERE project_id=?1 AND run_id=?2 ORDER BY rowid ASC LIMIT ?3 OFFSET ?4").map_err(|_|corrupt())?;
        let ids=stmt.query_map((&project,run,limit as i64,offset as i64),|r|r.get::<_,String>(0)).map_err(|_|corrupt())?.collect::<Result<Vec<_>,_>>().map_err(|_|corrupt())?;
        let records=ids.iter().map(|id|read_record(&connection,&project,id)).collect::<CoreResult<Vec<_>>>()?;
        let next=offset.saturating_add(records.len()); Ok(VerificationRecordPage {records,total,next_offset:(next<total).then_some(next)})
    }

    pub fn get_run_verification_state(&self, run:&str, rule:&str) -> CoreResult<RunVerificationState> {
        self.run(run)?; let project=self.summary()?.id; let connection=self.connection()?; read_rule(&connection,&project,rule)?;
        let record_count:i64=connection.query_row("SELECT count(*) FROM verification_records WHERE project_id=?1 AND run_id=?2 AND rule_id=?3",(&project,run,rule),|r|r.get(0)).map_err(|_|corrupt())?;
        let record_count=usize::try_from(record_count).map_err(|_|corrupt())?;
        let latest:Option<String>=connection.query_row("SELECT id FROM verification_records WHERE project_id=?1 AND run_id=?2 AND rule_id=?3 ORDER BY rowid DESC LIMIT 1",(&project,run,rule),|r|r.get(0)).optional().map_err(|_|corrupt())?;
        let pending:Option<String>=connection.query_row("SELECT q.id FROM verification_requests q WHERE q.project_id=?1 AND q.run_id=?2 AND q.rule_id=?3 AND NOT EXISTS (SELECT 1 FROM verification_records v WHERE v.request_id=q.id) ORDER BY q.rowid DESC LIMIT 1",(&project,run,rule),|r|r.get(0)).optional().map_err(|_|corrupt())?;
        let latest_record=latest.as_ref().map(|id|read_record(&connection,&project,id)).transpose()?;
        let conclusion=if pending.is_some(){VerificationConclusion::Inconclusive}else{latest_record.as_ref().map_or(VerificationConclusion::NotRun,|r|r.conclusion)};
        Ok(RunVerificationState {run_id:run.into(),rule_version_id:rule.into(),conclusion,latest_record,record_count,pending_request_id:pending})
    }

    pub fn execute_verification(&self, request:ExecuteVerification) -> CoreResult<VerificationRecord> {
        let _guard=self.verification_lock()?;
        if !text_valid(&request.client_request_id) {return Err(CoreError::new("invalid_verification_request","客户端请求身份不能为空、超长或含控制字符"));}
        let project=self.summary()?.id; let run=self.run(&request.run_id)?;
        if matches!(run.state,RunState::Queued|RunState::Running|RunState::Cancelling) {return Err(CoreError::new("verification_run_active","活动运行尚未冻结终态，请等待真实执行结束"));}
        let mut connection=self.connection()?;
        let transaction=connection.transaction_with_behavior(TransactionBehavior::Immediate).map_err(|_|schema::database_error())?;
        let existing:Option<String>=transaction.query_row("SELECT id FROM verification_requests WHERE project_id=?1 AND client_id=?2",(&project,&request.client_request_id),|r|r.get(0)).optional().map_err(|_|corrupt())?;
        if let Some(id)=existing {
            let old=read_request(&transaction,&id)?;
            if old.request!=request {return Err(CoreError::new("verification_request_conflict","同一客户端请求身份对应不同检查，未重复执行"));}
            let record_id:Option<String>=transaction.query_row("SELECT id FROM verification_records WHERE request_id=?1",[&id],|r|r.get(0)).optional().map_err(|_|corrupt())?;
            return record_id.map(|id|read_record(&transaction,&project,&id)).unwrap_or_else(||Err(CoreError::new("verification_pending","已有相同验证请求仍待确认，不会重复执行")));
        }
        let rule=read_rule(&transaction,&project,&request.rule_version_id)?;
        if let Some(previous)=&request.previous_record_id {let record=read_record(&transaction,&project,previous)?; if record.run_id!=run.id{return Err(CoreError::new("verification_history_conflict","重检只能关联同一运行的旧验证记录"));}}
        let saved=SavedRequest {id:Uuid::new_v4().to_string(),project_id:project.clone(),request,started_at:timestamp(),source:source_identity(&transaction,&run)?};
        transaction.execute("INSERT INTO verification_requests(id,project_id,run_id,rule_id,client_id,record_json,content_hash) VALUES (?1,?2,?3,?4,?5,?6,?7)",(&saved.id,&project,&run.id,&rule.id,&saved.request.client_request_id,json(&saved)?,typed_hash(&saved)?)).map_err(|_|schema::database_error())?;
        transaction.commit().map_err(|_|schema::database_error())?;
        // 请求先提交；发生崩溃时它仍可被识别，不允许相同clientRequestId重算。
        let checks=evaluate(&run,&rule);
        let mut record=VerificationRecord {id:Uuid::new_v4().to_string(),project_id:project,request_id:saved.id,client_request_id:saved.request.client_request_id,run_id:run.id.clone(),rule_version_id:rule.id,
            previous_record_id:saved.request.previous_record_id,started_at:saved.started_at,finished_at:timestamp(),executed_by:"local_user".into(),method_id:METHOD_ID.into(),method_version:METHOD_VERSION,
            execution_status:VerificationExecutionStatus::Completed,conclusion:aggregate(&checks),source:saved.source,checks,error:None,content_hash:String::new()};
        record.content_hash=record_hash(&record)?;
        let transaction=connection.transaction_with_behavior(TransactionBehavior::Immediate).map_err(|_|schema::database_error())?;
        if source_identity(&transaction,&run)?!=record.source {return Err(CoreError::new("verification_source_changed","验证期间产物身份发生变化，未登记完成"));}
        append_record(&transaction,&record)?; transaction.commit().map_err(|_|schema::database_error())?; Ok(record)
    }
}
