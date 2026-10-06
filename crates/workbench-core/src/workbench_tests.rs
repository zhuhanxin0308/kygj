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
