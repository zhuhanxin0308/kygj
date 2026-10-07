//! 宿主的共享业务入口；路径注册、预检授权和执行事实由单一管理者协调。

use std::{collections::{HashMap, HashSet}, path::Path, sync::{Arc, Mutex, MutexGuard, atomic::{AtomicBool, Ordering}}, thread::{self, JoinHandle}, time::Instant};
use uuid::Uuid;
use crate::{CoreError, CoreResult, limits::{ProcessLimits, MAX_RESPONSE_BYTES, MAX_STDERR_BYTES, MAX_TOTAL_SAMPLES, SHUTDOWN_CONFIRMATION_TIMEOUT}, process, storage::ProjectStore, types::*};

type Key = (String, String);

struct Inner {
    projects: Mutex<HashMap<String, ProjectStore>>,
    preflights: Mutex<HashMap<Key, PreflightReport>>,
    controls: Mutex<HashMap<Key, Arc<AtomicBool>>>,
    workers: Mutex<Vec<(Key, JoinHandle<()>)>>,
    probes: Mutex<HashMap<String, Arc<AtomicBool>>>,
    unconfirmed_exits: Mutex<HashSet<Key>>,
    operation: Mutex<()>,
    shutdown_gate: Mutex<()>,
    stopping: AtomicBool,
    limits: ProcessLimits,
    migrations: Mutex<HashMap<String, ProjectMigrationPlan>>,
}

/// Clone只共享管理状态，不复制运行或生成第二个执行端。
#[derive(Clone)]
pub struct Workbench { inner: Arc<Inner> }

fn lock<T>(mutex: &Mutex<T>) -> CoreResult<MutexGuard<'_, T>> {
    mutex.lock().map_err(|_| CoreError::new("host_state_error", "宿主状态不可用，请保留项目后重新打开"))
}

impl Default for Workbench { fn default() -> Self { Self::new() } }

impl Workbench {
    pub fn new() -> Self { Self::with_limits(ProcessLimits::default()).expect("默认进程限制必须有效") }

    pub fn with_limits(limits: ProcessLimits) -> CoreResult<Self> {
        if limits.max_wall_time.as_secs() == 0 || limits.poll_interval.is_zero() || limits.termination_timeout.is_zero() || limits.max_output_bytes == 0
            || limits.max_output_bytes > MAX_RESPONSE_BYTES || limits.max_stderr_bytes > MAX_STDERR_BYTES {
            return Err(CoreError::new("invalid_process_limits", "宿主进程限制无效，墙钟时间至少为一秒"));
        }
        Ok(Self { inner: Arc::new(Inner { projects: Mutex::new(HashMap::new()), preflights: Mutex::new(HashMap::new()),
            controls: Mutex::new(HashMap::new()), workers: Mutex::new(Vec::new()), probes: Mutex::new(HashMap::new()), unconfirmed_exits: Mutex::new(HashSet::new()), operation: Mutex::new(()), shutdown_gate: Mutex::new(()),
            stopping: AtomicBool::new(false), limits, migrations:Mutex::new(HashMap::new()) }) })
    }

    fn ensure_running(&self) -> CoreResult<()> {
        if self.inner.stopping.load(Ordering::SeqCst) { return Err(CoreError::new("host_stopping", "宿主正在退出，不再接受新执行操作")); }
        Ok(())
    }

    fn project(&self, project_id: &str) -> CoreResult<ProjectStore> {
        lock(&self.inner.projects)?.get(project_id).cloned().ok_or_else(|| CoreError::new("project_not_open", "项目尚未由当前宿主打开"))
    }

    pub fn create_project(&self, parent: &Path, name: &str) -> CoreResult<ProjectState> {
        let _operation = lock(&self.inner.operation)?;
        self.ensure_running()?;
        let store = ProjectStore::create(parent, name)?;
        let state = store.state()?;
        lock(&self.inner.projects)?.insert(state.project.id.clone(), store);
        Ok(state)
    }

    pub fn open_project(&self, directory: &Path) -> CoreResult<ProjectState> {
        let _operation = lock(&self.inner.operation)?;
        self.ensure_running()?;
        let store = ProjectStore::open(directory)?;
        let state = store.state()?;
        if let Some(existing) = lock(&self.inner.projects)?.get(&state.project.id)
            && existing.directory() != store.directory() { return Err(CoreError::new("project_identity_conflict", "同一项目身份已在其他目录打开，不能混用")); }
        store.recover_verification_requests()?;
        // 没有当前进程句柄的旧记录只标为未知，绝不自动重启或假装已经失败。
        let controls = lock(&self.inner.controls)?;
        for run in state.runs {
            if !run.state.terminal() && run.state != RunState::Unknown
                && !controls.contains_key(&(run.project_id.clone(), run.id.clone())) {
                store.transition(&run.id, RunState::Unknown, None, Some(CoreError::new("process_state_unknown", "原进程不在当前宿主管理范围，状态待核实")))?;
            }
        }
        drop(controls);
        lock(&self.inner.projects)?.insert(state.project.id, store.clone());
        store.state()
    }

    pub fn get_project(&self, project_id: &str) -> CoreResult<ProjectState> { self.project(project_id)?.state() }

    pub fn save_model(&self, project_id: &str, label: &str, config: EllisConfig) -> CoreResult<ModelVersion> {
        // 与关闭决定共用门禁，不能在停止接受写入后追加模型版本。
        let _operation = lock(&self.inner.operation)?;
        self.ensure_running()?;
        self.project(project_id)?.save_model(label, config)
    }

    pub fn probe_environment(&self, python: &Path) -> CoreResult<EnvironmentInfo> {
        let (id, cancelled) = self.begin_probe()?;
        let outcome = process::probe_environment_controlled(python, &self.inner.limits, cancelled);
        self.finish_probe(&id, &outcome)?;
        outcome
    }

    fn begin_probe(&self) -> CoreResult<(String, Arc<AtomicBool>)> {
        let _operation = lock(&self.inner.operation)?;
        self.ensure_running()?;
        let id = Uuid::new_v4().to_string();
        let cancelled = Arc::new(AtomicBool::new(false));
        lock(&self.inner.probes)?.insert(id.clone(), Arc::clone(&cancelled));
        Ok((id, cancelled))
    }

    fn finish_probe(&self, id: &str, outcome: &CoreResult<EnvironmentInfo>) -> CoreResult<()> {
        if outcome.as_ref().is_err_and(|error| error.code == "process_state_unknown") {
            lock(&self.inner.unconfirmed_exits)?.insert(("environment-probe".into(), id.into()));
        }
        lock(&self.inner.probes)?.remove(id);
        Ok(())
    }

    pub fn prepare_run(&self, project_id: &str, model_version_id: &str, python: &Path) -> CoreResult<PreflightReport> {
        self.ensure_running()?;
        let store = self.project(project_id)?;
        store.model(model_version_id)?.config.validate()?;
        let environment = self.probe_environment(python)?;
        let _operation = lock(&self.inner.operation)?;
        self.ensure_running()?;
        let report = store.prepare(model_version_id, environment, self.execution_limits(), vec![])?;
        // 本会话授权副本防止外来项目中的旧预检绕过“选择环境”动作。
        lock(&self.inner.preflights)?.insert((project_id.into(), report.id.clone()), report.clone());
        Ok(report)
    }

    fn execution_limits(&self) -> ExecutionLimits {
        ExecutionLimits { max_wall_time_seconds: self.inner.limits.max_wall_time.as_secs(), max_output_bytes: self.inner.limits.max_output_bytes, max_total_samples: MAX_TOTAL_SAMPLES }
    }

    pub fn start_run(&self, project_id: &str, preflight_id: &str) -> CoreResult<RunRecord> {
        self.ensure_running()?;
        let store = self.project(project_id)?;
        let key = (project_id.to_owned(), preflight_id.to_owned());
        let report = lock(&self.inner.preflights)?.get(&key).cloned().ok_or_else(|| CoreError::new("preflight_stale", "当前会话尚未确认此预检，或该预检已经提交，请重新预检"))?;
        if report.status != PreflightStatus::Ready { return Err(CoreError::new("preflight_blocked", "预检存在阻断项，不能提交")); }
        if store.preflight(preflight_id)? != report || report.execution_limits != self.execution_limits() { return Err(CoreError::new("preflight_stale", "预检记录或资源限制已改变，请重新预检")); }
        // 探测在操作锁外执行，避免等待Python导入时阻塞其他任务的取消。
        let current = self.probe_environment(Path::new(&report.environment.python_executable))?;
        let _operation = lock(&self.inner.operation)?;
        self.ensure_running()?;
        let run = store.consume_preflight(preflight_id, &current)?;
        lock(&self.inner.preflights)?.remove(&key);
        let directory = match store.run_directory(&run.id) {
            Ok(directory) => directory,
            Err(error) => { store.transition(&run.id, RunState::Failed, None, Some(error.clone()))?; return Err(error); }
        };
        let cancelled = Arc::new(AtomicBool::new(false));
        let run_key = (project_id.to_owned(), run.id.clone());
        lock(&self.inner.controls)?.insert(run_key.clone(), Arc::clone(&cancelled));
        let inner = Arc::clone(&self.inner);
        let worker_store = store.clone();
        let worker_run = run.clone();
        let worker_key = run_key.clone();
        let worker = thread::Builder::new().name(format!("gravity-run-{}", run.id)).spawn(move || {
            execute_background(inner, worker_store, worker_run, worker_key, directory, cancelled);
        });
        match worker {
            Ok(worker) => lock(&self.inner.workers)?.push((run_key, worker)),
            Err(_) => {
                lock(&self.inner.controls)?.remove(&run_key);
                let error = CoreError::new("worker_start_failed", "无法创建计算监督线程，未启动计算");
                store.transition(&run.id, RunState::Failed, None, Some(error.clone()))?;
                return Err(error);
            }
        }
        Ok(run)
    }

    pub fn get_run(&self, project_id: &str, run_id: &str) -> CoreResult<RunRecord> { self.project(project_id)?.run(run_id) }

    pub fn cancel_run(&self, project_id: &str, run_id: &str) -> CoreResult<RunRecord> {
        let _operation = lock(&self.inner.operation)?;
        let store = self.project(project_id)?;
        let run = store.run(run_id)?;
        if run.state.terminal() || run.state == RunState::Cancelling { return Ok(run); }
        if run.state == RunState::Unknown { return Err(CoreError::new("process_state_unknown", "当前无法确认原进程身份，不能伪报取消成功")); }
        let control = lock(&self.inner.controls)?.get(&(project_id.into(), run_id.into())).cloned();
        let Some(control) = control else {
            return store.transition(run_id, RunState::Unknown, None, Some(CoreError::new("process_state_unknown", "计算进程身份缺失，状态待核实")));
        };
        let requested = store.transition(run_id, RunState::Cancelling, None, None)?;
        control.store(true, Ordering::SeqCst);
        Ok(requested)
    }

    pub fn export_run(&self, project_id: &str, run_id: &str, destination: &Path) -> CoreResult<ExportedRun> { self.project(project_id)?.export_run(run_id, destination) }

    /// 独立验证接口只追加账本，所有对象先经当前已打开项目身份校验。
    pub fn list_verification_rules(&self, project_id:&str, offset:usize, limit:usize)->CoreResult<VerificationRulePage>{self.project(project_id)?.list_verification_rules(offset,limit)}
    pub fn save_verification_rule_version(&self, project_id:&str, draft:VerificationRuleDraft)->CoreResult<VerificationRuleVersion>{
        // 规则追加与退出决策共用操作门，确保关闭确认后不会再提交研究对象版本。
        let _operation=lock(&self.inner.operation)?;
        self.ensure_running()?;self.project(project_id)?.save_verification_rule_version(draft)
    }
    pub fn get_run_verification_state(&self, project_id:&str, run_id:&str, rule_version_id:&str)->CoreResult<RunVerificationState>{self.project(project_id)?.get_run_verification_state(run_id,rule_version_id)}
    pub fn execute_verification(&self, project_id:&str, request:ExecuteVerification)->CoreResult<VerificationRecord>{let _operation=lock(&self.inner.operation)?;self.ensure_running()?;self.project(project_id)?.execute_verification(request)}
    pub fn list_verification_records(&self, project_id:&str, run_id:&str, offset:usize, limit:usize)->CoreResult<VerificationRecordPage>{self.project(project_id)?.list_verification_records(run_id,offset,limit)}
    pub fn get_verification_record(&self, project_id:&str, record_id:&str)->CoreResult<VerificationRecord>{self.project(project_id)?.get_verification_record(record_id)}

    /// 预览只读，完整计划由宿主会话保存；前端不能自行构造授权计划。
    pub fn prepare_project_migration(&self, directory:&Path)->CoreResult<ProjectMigrationPlan>{
        self.ensure_running()?;let plan=crate::migration::prepare(directory)?;
        lock(&self.inner.migrations)?.insert(plan.id.clone(),plan.clone());Ok(plan)
    }
    pub fn apply_project_migration(&self, directory:&Path, plan_id:&str)->CoreResult<ProjectMigrationReceipt>{
        let _operation=lock(&self.inner.operation)?;self.ensure_running()?;
        let plan=lock(&self.inner.migrations)?.remove(plan_id).ok_or_else(||CoreError::new("migration_plan_stale","当前会话没有此迁移计划，或计划已经使用，请重新预览"))?;
        crate::migration::apply(directory,&plan)
    }

    /// 退出时先持久化取消意图，再等待所有监督线程确认真实子进程结束。
    pub fn shutdown(&self) -> CoreResult<()> {
        // 同时关闭必须串行确认，不能把另一调用临时接管的线程误认为已经退出。
        let _shutdown = lock(&self.inner.shutdown_gate)?;
        {
            let _operation = lock(&self.inner.operation)?;
            self.inner.stopping.store(true, Ordering::SeqCst);
            for cancelled in lock(&self.inner.probes)?.values() { cancelled.store(true, Ordering::SeqCst); }
            for cancelled in lock(&self.inner.controls)?.values() { cancelled.store(true, Ordering::SeqCst); }
            for ((project, run_id), flag) in lock(&self.inner.controls)?.iter() {
                let store = self.project(project)?;
                let run = store.run(run_id)?;
                if matches!(run.state, RunState::Queued | RunState::Running) { store.transition(run_id, RunState::Cancelling, None, None)?; }
                flag.store(true, Ordering::SeqCst);
            }
        }
        let started = Instant::now();
        let mut pending = std::mem::take(&mut *lock(&self.inner.workers)?);
        loop {
            let mut still_running = Vec::new();
            for (key, worker) in pending {
                if !worker.is_finished() { still_running.push((key, worker)); }
                else if worker.join().is_err() { lock(&self.inner.unconfirmed_exits)?.insert(key); }
            }
            pending = still_running;
            if pending.is_empty() && lock(&self.inner.probes)?.is_empty() { break; }
            if started.elapsed() >= SHUTDOWN_CONFIRMATION_TIMEOUT {
                lock(&self.inner.workers)?.extend(pending);
                return Err(CoreError::new("shutdown_pending", "仍有受管执行或环境探测等待退出确认，窗口必须保留"));
            }
            thread::sleep(self.inner.limits.poll_interval);
        }
        let unconfirmed = lock(&self.inner.unconfirmed_exits)?;
        if !unconfirmed.is_empty() {
            return Err(CoreError::new("process_state_unknown", format!("仍有{}个受管进程无法确认退出，不能安全关闭", unconfirmed.len())));
        }
        Ok(())
    }
}

fn execute_background(inner: Arc<Inner>, store: ProjectStore, run: RunRecord, key: Key, directory: std::path::PathBuf, cancelled: Arc<AtomicBool>) {
    let outcome = (|| -> CoreResult<()> {
        {
            let _operation = lock(&inner.operation)?;
            let current = store.run(&run.id)?;
            if current.state == RunState::Cancelling {
                store.transition(&run.id, RunState::Cancelled, None, None)?;
                return Ok(());
            }
            store.transition(&run.id, RunState::Running, None, None)?;
        }
        let result = process::execute_engine(&directory, &run.request, &run.environment, &inner.limits, Arc::clone(&cancelled));
        finish_execution(&inner, &store, &key, result)?;
        Ok(())
    })();
    if let Err(error) = outcome {
        // 存储失效不能伪造完成；若记录仍可用，只登记需要核实的状态。
        if let Ok(record) = store.run(&run.id)
            && matches!(record.state, RunState::Queued | RunState::Running | RunState::Cancelling) {
            let _ = store.transition(&run.id, RunState::Unknown, None, Some(error));
        }
    }
    if let Ok(mut controls) = inner.controls.lock() { controls.remove(&key); }
}

/// 将执行端事实转换为领域状态，单独测试异常退出确认而不伪造生产引擎。
fn finish_execution(inner: &Inner, store: &ProjectStore, key: &Key, result: CoreResult<TraceResult>) -> CoreResult<()> {
    let _operation = lock(&inner.operation)?;
    if result.as_ref().is_err_and(|error| error.code == "process_state_unknown") {
        // 必须先保留执行端事实，不能依赖之后可能失败的数据库写入。
        lock(&inner.unconfirmed_exits)?.insert(key.clone());
    }
    let current = store.run(&key.1)?;
    if current.state == RunState::Cancelling {
        if result.as_ref().is_err_and(|error| error.code == "process_state_unknown") {
            store.transition(&key.1, RunState::Unknown, None, result.err())?;
        } else { store.transition(&key.1, RunState::Cancelled, None, None)?; }
    } else {
        match result {
            Ok(result) => { store.transition(&key.1, RunState::Completed, Some(result), None)?; }
            Err(error) => {
                let next = if error.code == "process_state_unknown" { RunState::Unknown } else { RunState::Failed };
                store.transition(&key.1, next, None, Some(error))?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "workbench_tests.rs"]
mod tests;
