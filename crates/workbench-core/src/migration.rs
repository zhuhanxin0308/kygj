//! v1到v2的显式迁移：只读预览、会话授权、锁定复核、一致性备份与原子提交。
use std::{fs::{self,OpenOptions},io::Read,path::{Path,PathBuf},time::{Duration,Instant}};
use rusqlite::{backup::{Backup,StepResult},Connection,OpenFlags,TransactionBehavior,types::ValueRef};
use sha2::{Digest,Sha256};
use uuid::Uuid;
use crate::{CoreError,CoreResult,limits::{SCHEMA_VERSION,SQLITE_BUSY_TIMEOUT},schema,storage::{timestamp,reject_link,validate_run_record,validate_model_record},types::*,verification_storage};

const LEGACY_VERSION:u32=1;
const BACKUP_PAGES_PER_STEP:i32=128;
const BACKUP_TIMEOUT:Duration=Duration::from_secs(60);
const HASH_BUFFER_BYTES:usize=64*1024;
fn error(code:&str,message:&str)->CoreError{CoreError::new(code,message)}
fn sql_error()->CoreError{error("migration_storage_error","迁移存储操作失败，未提交的更改已回滚；已完成的一致性备份仍可恢复")}

fn open_database(directory:&Path,readonly:bool)->CoreResult<(PathBuf,Connection)> {
    let directory=directory.canonicalize().map_err(|_|error("file_access_error","无法打开待迁移项目目录"))?;
    if !directory.is_dir(){return Err(error("invalid_project_path","项目路径必须是目录"));}
    reject_link(&directory.join(".gravity"))?; reject_link(&directory.join(".gravity/workbench.sqlite"))?;
    let flags=if readonly{OpenFlags::SQLITE_OPEN_READ_ONLY}else{OpenFlags::SQLITE_OPEN_READ_WRITE};
    let connection=Connection::open_with_flags(directory.join(".gravity/workbench.sqlite"),flags).map_err(|_|sql_error())?;
    connection.busy_timeout(SQLITE_BUSY_TIMEOUT).map_err(|_|sql_error())?;
    connection.pragma_update(None,"foreign_keys",true).map_err(|_|sql_error())?;
    Ok((directory,connection))
}

fn legacy_project(connection:&Connection)->CoreResult<ProjectSummary> {
    let app:i32=connection.pragma_query_value(None,"application_id",|r|r.get(0)).map_err(|_|sql_error())?;
    let version:u32=connection.pragma_query_value(None,"user_version",|r|r.get(0)).map_err(|_|sql_error())?;
    if app!=schema::DATABASE_APPLICATION_ID || version!=LEGACY_VERSION{return Err(error("migration_not_supported","此计划仅支持已识别的v1项目迁移至v2"));}
    let integrity:String=connection.query_row("PRAGMA quick_check",[],|r|r.get(0)).map_err(|_|sql_error())?;
    if integrity!="ok" {return Err(error("corrupt_project","SQLite完整性检查未通过，禁止迁移"));}
    let count:i64=connection.query_row("SELECT count(*) FROM projects",[],|r|r.get(0)).map_err(|_|sql_error())?;
    if count!=1{return Err(error("corrupt_project","旧项目身份不唯一"));}
    let (id,json):(String,String)=connection.query_row("SELECT id,record_json FROM projects",[],|r|Ok((r.get(0)?,r.get(1)?))).map_err(|_|sql_error())?;
    let project:ProjectSummary=serde_json::from_str(&json).map_err(|_|error("corrupt_project","旧项目元数据损坏"))?;
    if id!=project.id || Uuid::parse_str(&id).is_err() || project.schema_version!=LEGACY_VERSION{return Err(error("corrupt_project","旧项目身份或格式字段不一致"));}
    crate::validation::validate_project_name(&project.name)?;
    for table in ["models","runs"] {
        let mut stmt=connection.prepare(&format!("SELECT id,record_json FROM {table} ORDER BY rowid")).map_err(|_|sql_error())?;
        let mut rows=stmt.query([]).map_err(|_|sql_error())?;
        while let Some(row)=rows.next().map_err(|_|sql_error())? {
            let id:String=row.get(0).map_err(|_|sql_error())?; let json:String=row.get(1).map_err(|_|sql_error())?;
            if table=="models" {
                let model:ModelVersion=serde_json::from_str(&json).map_err(|_|error("corrupt_project","旧模型记录损坏"))?;
                if model.id!=id{return Err(error("corrupt_project","旧模型身份冲突"));} validate_model_record(connection,&model,&project.id)?;
            } else {
                let run:RunRecord=serde_json::from_str(&json).map_err(|_|error("corrupt_project","旧运行记录损坏"))?;
                if run.id!=id{return Err(error("corrupt_project","旧运行身份冲突"));} validate_run_record(connection,&run,&project.id)?;
            }
        }
    }
    Ok(project)
}

/// 哈希逻辑行与完整schema，读取SQLite快照，包含尚在WAL中的已提交数据。
fn logical_fingerprint(connection:&Connection)->CoreResult<String> { fingerprint_rows(connection,true) }
fn fingerprint_rows(connection:&Connection,include_metadata:bool)->CoreResult<String> {
    let mut digest=Sha256::new();
    for sql in ["SELECT type,name,tbl_name,sql FROM sqlite_schema ORDER BY type,name", "SELECT id,record_json FROM projects ORDER BY id",
        "SELECT id,project_id,label,created_at,record_json FROM models ORDER BY id", "SELECT id,project_id,record_json,consumed FROM preflights ORDER BY id",
        "SELECT id,project_id,model_id,created_at,state,request_json,environment_json,record_json FROM runs ORDER BY id"] {
        if !include_metadata && (sql.contains("sqlite_schema") || sql.contains(" FROM projects ")){continue;}
        digest.update((sql.len() as u64).to_le_bytes());digest.update(sql.as_bytes());
        let mut stmt=connection.prepare(sql).map_err(|_|sql_error())?; let columns=stmt.column_count();let mut rows=stmt.query([]).map_err(|_|sql_error())?;
        while let Some(row)=rows.next().map_err(|_|sql_error())? {
            digest.update(b"row");
            for column in 0..columns {
                match row.get_ref(column).map_err(|_|sql_error())? {
                    ValueRef::Null=>digest.update(b"null"),
                    ValueRef::Integer(value)=>{digest.update(b"int");digest.update(value.to_le_bytes());},
                    ValueRef::Real(value)=>{digest.update(b"real");digest.update(value.to_bits().to_le_bytes());},
                    ValueRef::Text(value)=>{digest.update(b"text");digest.update((value.len() as u64).to_le_bytes());digest.update(value);},
                    ValueRef::Blob(value)=>{digest.update(b"blob");digest.update((value.len() as u64).to_le_bytes());digest.update(value);},
                }
            }
        }
    }
    Ok(digest.finalize().iter().map(|b|format!("{b:02x}")).collect())
}

/// Windows保留禁止删除共享的目录句柄，备份期间父目录不能被替换为重解析路径。
fn guard_directory(path:&Path)->CoreResult<Option<fs::File>> {
    #[cfg(windows)] {
        use std::os::windows::fs::OpenOptionsExt;
        const SHARE_READ_WRITE:u32=0x0000_0001|0x0000_0002;
        const BACKUP_SEMANTICS:u32=0x0200_0000;
        let file=OpenOptions::new().read(true).share_mode(SHARE_READ_WRITE).custom_flags(BACKUP_SEMANTICS).open(path).map_err(|_|error("migration_path_lock_failed","无法锁定迁移目录身份，原项目未修改"))?;
        Ok(Some(file))
    }
    #[cfg(not(windows))] {let _=path;Ok(None)}
}

fn physical_fingerprint(directory:&Path)->CoreResult<String> {
    let mut identity=directory.to_string_lossy().to_string();
    // 创建时间用于识别预览后路径被替换；不依赖会随WAL检查点变化的修改时间。
    for path in [directory.to_path_buf(),directory.join(".gravity"),directory.join(".gravity/workbench.sqlite")] {
        let metadata=fs::metadata(path).map_err(|_|error("file_access_error","无法核验项目文件身份"))?;
        let created=metadata.created().or_else(|_|metadata.modified()).map_err(|_|error("file_access_error","无法取得项目文件身份时间"))?;
        identity.push_str(&format!("|{created:?}"));
    }
    Ok(identity)
}
fn fingerprint(connection:&Connection,directory:&Path)->CoreResult<String>{crate::verification::typed_hash(&(logical_fingerprint(connection)?,physical_fingerprint(directory)?))}

pub(crate) fn prepare(directory:&Path)->CoreResult<ProjectMigrationPlan> {
    let (directory,mut connection)=open_database(directory,true)?;
    let transaction=connection.transaction().map_err(|_|sql_error())?;
    let project=legacy_project(&transaction)?;let source_fingerprint=fingerprint(&transaction,&directory)?;
    let id=Uuid::new_v4().to_string();let backup_path=directory.join(".gravity").join(format!("migration-v1-backup-{id}.sqlite"));
    Ok(ProjectMigrationPlan {id,directory:directory.to_string_lossy().into(),project_id:project.id,project_name:project.name,from_version:LEGACY_VERSION,to_version:SCHEMA_VERSION,created_at:timestamp(),source_fingerprint,backup_path:backup_path.to_string_lossy().into(),
        changes:vec!["事务新增独立验证规则、请求、记录及产物内容身份表".into(),"原模型、运行快照与旧数值状态保持原字节；项目格式升为v2".into(),"迁移前保存可独立读取的v1 SQLite一致性备份".into()],
        warnings:vec!["旧产物仅登记迁移时实际所见内容SHA-256，不证明生成以来未被修改".into(),"备份保存本次元数据迁移所涉及的完整数据库；本次不移动或改写外部大文件".into(),"迁移不补写历史验证通过记录，不自动执行计算或检查".into()],requires_confirmation:true})
}

fn file_hash(path:&Path)->CoreResult<String> {
    let mut file=fs::File::open(path).map_err(|_|sql_error())?;let mut buffer=[0u8;HASH_BUFFER_BYTES];let mut hash=Sha256::new();
    loop {let count=file.read(&mut buffer).map_err(|_|sql_error())?;if count==0{break;}hash.update(&buffer[..count]);}
    Ok(hash.finalize().iter().map(|b|format!("{b:02x}")).collect())
}

pub(crate) fn apply(directory:&Path,plan:&ProjectMigrationPlan)->CoreResult<ProjectMigrationReceipt> {
    let (directory,mut connection)=open_database(directory,false)?;
    if directory.to_string_lossy()!=plan.directory {return Err(error("migration_plan_stale","项目路径与预览计划不一致，请重新预览"));}
    let _directory_guard=guard_directory(&directory)?;let _metadata_guard=guard_directory(&directory.join(".gravity"))?;
    let transaction=connection.transaction_with_behavior(TransactionBehavior::Immediate).map_err(|_|sql_error())?;
    let project=legacy_project(&transaction)?;
    if project.id!=plan.project_id || fingerprint(&transaction,&directory)?!=plan.source_fingerprint{return Err(error("migration_plan_stale","项目内容或文件身份已改变，请重新预览迁移计划"));}
    let frozen_scientific_content=fingerprint_rows(&transaction,false)?;
    let backup_path=PathBuf::from(&plan.backup_path);let expected=directory.join(".gravity").join(format!("migration-v1-backup-{}.sqlite",plan.id));
    if backup_path!=expected{return Err(error("migration_plan_stale","备份路径与会话计划不一致"));}
    reject_link(&directory.join(".gravity"))?;
    // 排他创建；已有文件、链接或重解析路径一律不能作为备份目标。
    let mut reserve_options=OpenOptions::new();reserve_options.read(true).write(true).create_new(true);
    #[cfg(windows)] {use std::os::windows::fs::OpenOptionsExt;const SHARE_READ_WRITE:u32=0x0000_0001|0x0000_0002;reserve_options.share_mode(SHARE_READ_WRITE);}
    let reserved=reserve_options.open(&backup_path).map_err(|_|error("migration_backup_failed","无法排他创建备份文件，原项目未修改"))?;
    let backup_outcome=(||->CoreResult<String>{
        let (_,mut source)=open_database(&directory,true)?;let snapshot=source.transaction().map_err(|_|sql_error())?;
        if fingerprint(&snapshot,&directory)?!=plan.source_fingerprint{return Err(error("migration_plan_stale","建立备份前源状态已改变"));}
        reject_link(&backup_path)?;
        let mut destination=Connection::open_with_flags(&backup_path,OpenFlags::SQLITE_OPEN_READ_WRITE|OpenFlags::SQLITE_OPEN_NOFOLLOW).map_err(|_|sql_error())?;
        {let backup=Backup::new(&snapshot,&mut destination).map_err(|_|sql_error())?;let started=Instant::now();loop {
            if started.elapsed()>BACKUP_TIMEOUT{return Err(error("migration_backup_failed","一致性备份超过等待预算，原项目未修改"));}
            match backup.step(BACKUP_PAGES_PER_STEP).map_err(|_|sql_error())? {StepResult::Done=>break,StepResult::More=>{},_=>return Err(error("migration_backup_failed","备份锁定失败，原项目未修改"))}
        }}
        if logical_fingerprint(&destination)?!=logical_fingerprint(&snapshot)?{return Err(error("migration_backup_failed","备份逻辑内容校验未通过"));}
        let check:String=destination.query_row("PRAGMA quick_check",[],|r|r.get(0)).map_err(|_|sql_error())?;if check!="ok"{return Err(error("migration_backup_failed","备份完整性校验未通过"));}
        drop(destination);OpenOptions::new().read(true).write(true).open(&backup_path).and_then(|f|f.sync_all()).map_err(|_|sql_error())?;file_hash(&backup_path)
    })();
    let backup_sha256=match backup_outcome {Ok(hash)=>hash,Err(err)=>{drop(reserved);let _=fs::remove_file(&backup_path);return Err(err);}};
    let _backup_guard=reserved;
    // 此后任意失败仅回滚事务，不删除已经完整且可恢复的v1备份。
    verification_storage::initialize(&transaction,&project.id)?;
    let mut legacy_result_count=0;
    {let mut stmt=transaction.prepare("SELECT record_json FROM runs ORDER BY rowid").map_err(|_|sql_error())?;let mut rows=stmt.query([]).map_err(|_|sql_error())?;
        while let Some(row)=rows.next().map_err(|_|sql_error())? {let json:String=row.get(0).map_err(|_|sql_error())?;let run:RunRecord=serde_json::from_str(&json).map_err(|_|sql_error())?;
            if run.result.is_some(){verification_storage::capture_result_identity(&transaction,&run,ResultIdentityOrigin::ObservedAtMigration)?;legacy_result_count+=1;}
        }}
    let mut updated=project.clone();updated.schema_version=SCHEMA_VERSION;
    transaction.execute("UPDATE projects SET record_json=?1 WHERE id=?2",(serde_json::to_string(&updated).map_err(|_|sql_error())?,&project.id)).map_err(|_|sql_error())?;
    if fingerprint_rows(&transaction,false)?!=frozen_scientific_content{return Err(error("migration_source_changed","迁移期间原模型、预检或运行内容被改变，已回滚"));}
    let receipt=ProjectMigrationReceipt {plan_id:plan.id.clone(),directory:plan.directory.clone(),project_id:project.id,from_version:LEGACY_VERSION,to_version:SCHEMA_VERSION,backup_path:plan.backup_path.clone(),backup_sha256,migrated_at:timestamp(),legacy_result_count};
    transaction.execute("INSERT INTO migration_history(id,record_json) VALUES (?1,?2)",(&plan.id,serde_json::to_string(&receipt).map_err(|_|sql_error())?)).map_err(|_|sql_error())?;
    transaction.pragma_update(None,"user_version",SCHEMA_VERSION).map_err(|_|sql_error())?;
    transaction.commit().map_err(|_|sql_error())?;Ok(receipt)
}
