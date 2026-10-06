//! 使用真实Python子进程制造协议边界与阻塞情形；夹具不注册为生产引擎。

use super::*;

fn python() -> PathBuf {
    if let Some(path) = std::env::var_os("WORKBENCH_TEST_PYTHON") { return PathBuf::from(path); }
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().parent().unwrap();
    #[cfg(windows)] let relative = ".venv/Scripts/python.exe";
    #[cfg(not(windows))] let relative = ".venv/bin/python";
    repository.join(relative)
}

fn run_script(script: &str, input: Vec<u8>, limits: ProcessLimits, cancel: Arc<AtomicBool>) -> CoreResult<ProcessOutput> {
    let directory = tempfile::tempdir().unwrap();
    exchange(&python(), &["-I", "-c", script], directory.path(), input, &limits, cancel)
}

#[test]
fn real_process_captures_output_and_bounds_diagnostics() {
    let limits = ProcessLimits { max_stderr_bytes: 32, ..ProcessLimits::default() };
    let output = run_script("import sys; sys.stdin.readline(); sys.stderr.write('x'*1000); print('ok')", b"{}".to_vec(), limits, Arc::new(AtomicBool::new(false))).unwrap();
    assert!(output.status.success()); assert_eq!(String::from_utf8(output.stdout).unwrap().trim_end_matches(['\r', '\n']), "ok");
    assert_eq!(output.stderr.len(), 32); assert!(output.stderr_truncated);
}

#[test]
fn response_overflow_terminates_real_process() {
    let limits = ProcessLimits { max_output_bytes: 64, ..ProcessLimits::default() };
    let error = run_script("import sys,time; sys.stdout.write('x'*10000); sys.stdout.flush(); time.sleep(10)", b"{}".to_vec(), limits, Arc::new(AtomicBool::new(false))).unwrap_err();
    assert_eq!(error.code, "output_limit_exceeded");
}

#[test]
fn timeout_includes_blocked_stdin_and_waits_for_process_exit() {
    let limits = ProcessLimits { max_wall_time: Duration::from_millis(150), ..ProcessLimits::default() };
    let start = Instant::now();
    let error = run_script("import time; time.sleep(10)", vec![b'x'; MAX_REQUEST_BYTES - 1], limits, Arc::new(AtomicBool::new(false))).unwrap_err();
    assert_eq!(error.code, "process_timeout"); assert!(start.elapsed() < Duration::from_secs(3));
}

#[test]
fn cancellation_is_observed_before_spawn_and_during_execution() {
    let cancelled = Arc::new(AtomicBool::new(true));
    assert_eq!(run_script("print('must not execute')", b"{}".to_vec(), ProcessLimits::default(), cancelled).unwrap_err().code, "cancelled");
    let cancelled = Arc::new(AtomicBool::new(false)); let signal = Arc::clone(&cancelled);
    let sender = thread::spawn(move || { thread::sleep(Duration::from_millis(150)); signal.store(true, Ordering::SeqCst); });
    let error = run_script("import time; time.sleep(10)", b"{}".to_vec(), ProcessLimits::default(), cancelled).unwrap_err();
    sender.join().unwrap(); assert_eq!(error.code, "cancelled");
}

#[test]
fn oversized_request_and_invalid_python_path_are_rejected() {
    let error = run_script("print('no')", vec![b'x'; MAX_REQUEST_BYTES + 1], ProcessLimits::default(), Arc::new(AtomicBool::new(false))).unwrap_err();
    assert_eq!(error.code, "input_limit_exceeded");
    assert!(canonical_python(Path::new("powershell.exe")).is_err());
}

#[test]
fn installed_engine_describes_real_environment() {
    let info = probe_environment(&python(), &ProcessLimits::default()).unwrap();
    assert_eq!(info.engine_version, "0.1.0"); assert_eq!(info.engine_source_hash.len(), 64);
    assert!(!info.python_version.is_empty()); assert!(!info.scipy_version.is_empty());
}

#[test]
fn transient_state_read_failure_can_still_confirm_exit() {
    // 只注入内核查询结果，验证一次查询错误不等于永久无法确认退出。
    #[cfg(windows)] use std::os::windows::process::ExitStatusExt;
    #[cfg(unix)] use std::os::unix::process::ExitStatusExt;
    let mut queries = 0;
    let status = poll_confirmed_exit(|| {
        queries += 1;
        if queries == 1 { Err(std::io::Error::other("注入状态查询异常")) }
        else { Ok(Some(ExitStatus::from_raw(0))) }
    }, Duration::from_millis(100), Duration::from_millis(1)).unwrap();
    assert!(status.success());
    assert_eq!(queries, 2);
}

#[test]
fn termination_confirmation_has_a_budget_for_live_or_unreadable_processes() {
    // 领域故障注入，不声称在实际操作系统上制造了kill或wait故障。
    for unreadable in [false, true] {
        let started = Instant::now();
        let error = poll_confirmed_exit(|| {
            if unreadable { Err(std::io::Error::other("注入持续状态查询异常")) }
            else { Ok(None) }
        }, Duration::from_millis(20), Duration::from_millis(1)).unwrap_err();
        assert_eq!(error.code, "process_state_unknown");
        assert!(started.elapsed() < Duration::from_secs(1), "不能退回无界wait");
    }
}

#[cfg(unix)]
#[test]
fn virtual_environment_symbolic_entry_is_not_resolved_to_base_python() {
    use std::os::unix::fs::symlink;
    let directory = tempfile::tempdir().unwrap();
    let entry = directory.path().join("python");
    symlink(python().canonicalize().unwrap(), &entry).unwrap();
    assert_eq!(canonical_python(&entry).unwrap(), entry);
}
