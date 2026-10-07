//! 退出确认的领域状态注入；不冒充操作系统异常的实机故障注入。

use super::*;

fn managed_run_fixture() -> (tempfile::TempDir, Workbench, ProjectStore, Key) {
    let directory = tempfile::tempdir().unwrap();
    let workbench = Workbench::new();
    let state = workbench.create_project(directory.path(), "退出确认测试").unwrap();
    let store = workbench.project(&state.project.id).unwrap();
    let config = EllisConfig { throat_radius:1.0,initial_radius:10.0,impact_parameters:vec![0.0],max_affine_parameter:1.0,sample_count:2,relative_tolerance:1e-9,absolute_tolerance:1e-11 };
    let model = store.save_model("状态测试模型", config).unwrap();
    let environment = EnvironmentInfo { python_executable:"测试专用领域身份".into(),engine_version:"0.1.0".into(),python_version:"3".into(),numpy_version:"2".into(),scipy_version:"1".into(),engine_source_hash:"a".repeat(64) };
    let report = store.prepare(&model.id, environment.clone(), ExecutionLimits::default(), vec![]).unwrap();
    let run = store.consume_preflight(&report.id, &environment).unwrap();
    store.transition(&run.id, RunState::Running, None, None).unwrap();
    let key = (state.project.id, run.id);
    (directory, workbench, store, key)
}

#[test]
fn unconfirmed_managed_exit_blocks_shutdown_even_after_supervisor_returns() {
    let (_directory, workbench, store, key) = managed_run_fixture();
    finish_execution(&workbench.inner, &store, &key, Err(CoreError::new("process_state_unknown", "注入退出无法确认的执行事实"))).unwrap();
    assert_eq!(store.run(&key.1).unwrap().state, RunState::Unknown);
    assert!(workbench.inner.controls.lock().unwrap().is_empty());
    assert!(workbench.shutdown().is_err(), "监督线程结束不能代替子进程退出确认");
    assert!(workbench.shutdown().is_err(), "再次关闭也不能遗忘尚未确认的事实");
}

#[test]
fn state_read_error_with_confirmed_exit_does_not_permanently_block_shutdown() {
    let (_directory, workbench, store, key) = managed_run_fixture();
    finish_execution(&workbench.inner, &store, &key, Err(CoreError::new("process_state_read_failed", "注入状态查询失败但实际退出已确认"))).unwrap();
    assert_eq!(store.run(&key.1).unwrap().state, RunState::Failed);
    assert!(workbench.shutdown().is_ok());
}

#[test]
fn shutdown_cancels_and_waits_for_registered_environment_probe() {
    // 探测登记和结束使用生产状态入口；取消后结束的操作系统路径另有真实进程测试。
    let workbench = Workbench::new();
    let (id, cancelled) = workbench.begin_probe().unwrap();
    let closer = workbench.clone();
    let (sender, receiver) = std::sync::mpsc::channel();
    let shutdown = thread::spawn(move || { sender.send(closer.shutdown()).unwrap(); });
    let deadline = Instant::now() + std::time::Duration::from_secs(2);
    while !cancelled.load(Ordering::SeqCst) {
        assert!(Instant::now() < deadline, "关闭必须向已登记的探测请求取消");
        thread::sleep(std::time::Duration::from_millis(1));
    }
    assert!(matches!(receiver.try_recv(), Err(std::sync::mpsc::TryRecvError::Empty)), "收到取消请求不能代替真实退出确认");
    workbench.finish_probe(&id, &Err(CoreError::new("cancelled", "注入探测进程已退出"))).unwrap();
    assert!(receiver.recv_timeout(std::time::Duration::from_secs(2)).unwrap().is_ok());
    shutdown.join().unwrap();
    assert_eq!(workbench.begin_probe().unwrap_err().code, "host_stopping");
}

#[test]
fn unconfirmed_probe_exit_is_retained_after_probe_caller_returns() {
    let workbench = Workbench::new();
    let (id, _) = workbench.begin_probe().unwrap();
    workbench.finish_probe(&id, &Err(CoreError::new("process_state_unknown", "注入探测进程退出无法确认"))).unwrap();
    assert!(workbench.inner.probes.lock().unwrap().is_empty());
    assert_eq!(workbench.shutdown().unwrap_err().code, "process_state_unknown");
    assert_eq!(workbench.shutdown().unwrap_err().code, "process_state_unknown");
}

#[test]
fn concurrent_shutdown_cannot_skip_worker_owned_by_first_closer() {
    use std::{sync::mpsc, time::Duration};
    const TEST_DEADLINE: Duration = Duration::from_secs(2);
    const PREMATURE_CLOSE_WINDOW: Duration = Duration::from_millis(100);
    const TEST_POLL_INTERVAL: Duration = Duration::from_millis(1);
    let (_directory, workbench, store, key) = managed_run_fixture();
    let (confirm_exit, await_exit) = mpsc::channel();
    let inner = Arc::clone(&workbench.inner);
    let worker_store = store.clone();
    let worker_key = key.clone();
    // 用明确的领域信号延迟退出确认；这不是操作系统故障或生产引擎替身。
    let worker = thread::spawn(move || {
        await_exit.recv().unwrap();
        finish_execution(&inner, &worker_store, &worker_key,
            Err(CoreError::new("cancelled", "注入已确认的受管进程退出"))).unwrap();
        inner.controls.lock().unwrap().remove(&worker_key);
    });
    workbench.inner.controls.lock().unwrap().insert(key.clone(), Arc::new(AtomicBool::new(false)));
    workbench.inner.workers.lock().unwrap().push((key.clone(), worker));
    let first_closer = workbench.clone();
    let first = thread::spawn(move || first_closer.shutdown());
    let deadline = Instant::now() + TEST_DEADLINE;
    while !workbench.inner.workers.lock().unwrap().is_empty() {
        assert!(Instant::now() < deadline, "第一个关闭请求必须接管监督线程并等待");
        thread::sleep(TEST_POLL_INTERVAL);
    }
    assert_eq!(store.run(&key.1).unwrap().state, RunState::Cancelling);
    let (second_ready, await_second) = mpsc::channel();
    let (second_result, await_result) = mpsc::channel();
    let second_closer = workbench.clone();
    let second = thread::spawn(move || {
        second_ready.send(()).unwrap();
        second_result.send(second_closer.shutdown()).unwrap();
    });
    await_second.recv_timeout(TEST_DEADLINE).unwrap();
    let premature = await_result.recv_timeout(PREMATURE_CLOSE_WINDOW);
    // 无论断言是否成立都先释放并回收线程，避免失败用例遗留后台工作。
    confirm_exit.send(()).unwrap();
    assert!(first.join().unwrap().is_ok());
    second.join().unwrap();
    assert!(matches!(premature, Err(mpsc::RecvTimeoutError::Timeout)),
        "第一个关闭仍等待退出确认时，第二个关闭不得提前完成：{premature:?}");
    assert!(await_result.recv_timeout(TEST_DEADLINE).unwrap().is_ok());
    assert_eq!(store.run(&key.1).unwrap().state, RunState::Cancelled);
}

#[test]
fn rule_version_commit_participates_in_the_host_shutdown_operation_gate() {
    use std::{sync::mpsc, time::Duration};
    const PREMATURE_COMMIT_WINDOW: Duration = Duration::from_millis(100);
    const TEST_DEADLINE: Duration = Duration::from_secs(2);
    let directory = tempfile::tempdir().unwrap();
    let workbench = Workbench::new();
    let project = workbench.create_project(directory.path(), "关闭与规则提交").unwrap();
    let rule = workbench.list_verification_rules(&project.project.id, 0, 20).unwrap().rules.remove(0);
    let draft = VerificationRuleDraft {base_version_id:rule.id,title:"关闭前规则".into(),change_reason:"保留研究者修订".into(),thresholds:rule.checks.iter().map(|check|VerificationThreshold {metric_id:check.metric_id,threshold:check.threshold,basis:check.basis.clone()}).collect()};
    // 持有与shutdown相同的操作门：新规则不能越过正在进行的关闭决定写入数据库。
    let gate = workbench.inner.operation.lock().unwrap();
    let writer = workbench.clone(); let project_id = project.project.id.clone();
    let (ready, await_ready) = mpsc::channel(); let (finished, await_finished) = mpsc::channel();
    let worker = thread::spawn(move || {ready.send(()).unwrap();finished.send(writer.save_verification_rule_version(&project_id, draft)).unwrap();});
    await_ready.recv_timeout(TEST_DEADLINE).unwrap();
    let premature = await_finished.recv_timeout(PREMATURE_COMMIT_WINDOW);
    // 模拟关闭在持锁状态下停止接受写入，然后释放等待者；无论结果均回收线程。
    workbench.inner.stopping.store(true, Ordering::SeqCst); drop(gate); worker.join().unwrap();
    assert!(matches!(premature, Err(mpsc::RecvTimeoutError::Timeout)), "规则提交不能绕过退出操作门：{premature:?}");
    assert_eq!(await_finished.recv_timeout(TEST_DEADLINE).unwrap().unwrap_err().code, "host_stopping");
    assert_eq!(workbench.list_verification_rules(&project.project.id, 0, 20).unwrap().total, 1);
}

#[test]
fn model_version_commit_cannot_bypass_the_host_shutdown_operation_gate() {
    use std::{sync::mpsc, time::Duration};
    const PREMATURE_COMMIT_WINDOW: Duration = Duration::from_millis(100);
    const TEST_DEADLINE: Duration = Duration::from_secs(2);
    let directory = tempfile::tempdir().unwrap();
    let workbench = Workbench::new();
    let project = workbench.create_project(directory.path(), "关闭与模型提交").unwrap();
    let config = EllisConfig { throat_radius:1.0,initial_radius:10.0,impact_parameters:vec![0.0],max_affine_parameter:20.0,sample_count:3,relative_tolerance:1e-10,absolute_tolerance:1e-12 };
    // 模型版本和规则版本一样是持久研究对象，关闭决定必须排除所有后续写入。
    let gate = workbench.inner.operation.lock().unwrap();
    let writer = workbench.clone(); let project_id = project.project.id.clone();
    let (ready, await_ready) = mpsc::channel(); let (finished, await_finished) = mpsc::channel();
    let worker = thread::spawn(move || {ready.send(()).unwrap();finished.send(writer.save_model(&project_id, "关闭前模型", config)).unwrap();});
    await_ready.recv_timeout(TEST_DEADLINE).unwrap();
    let premature = await_finished.recv_timeout(PREMATURE_COMMIT_WINDOW);
    // 先释放并回收线程再断言，失败用例也不得留下阻塞线程。
    workbench.inner.stopping.store(true, Ordering::SeqCst); drop(gate); worker.join().unwrap();
    assert!(matches!(premature, Err(mpsc::RecvTimeoutError::Timeout)), "模型提交不能绕过退出操作门：{premature:?}");
    assert_eq!(await_finished.recv_timeout(TEST_DEADLINE).unwrap().unwrap_err().code, "host_stopping");
    assert!(workbench.get_project(&project.project.id).unwrap().models.is_empty());
}
