//! 固定Python模块的独立进程监督：异步管道、有界捕获、取消及超时均等待真实退出。

use std::{io::{Read, Write}, path::{Path, PathBuf}, process::{Command, ExitStatus, Stdio}, sync::{Arc, atomic::{AtomicBool, Ordering}, mpsc}, thread, time::{Duration, Instant}};
use uuid::Uuid;
use crate::{CoreError, CoreResult, limits::*, protocol, types::*};

const STREAM_DRAIN_TIMEOUT: Duration = Duration::from_secs(2);
const FIXED_ENGINE_ARGUMENTS: &[&str] = &["-I", "-X", "utf8", "-m", "gravity_engine"];

#[derive(Debug)]
struct ProcessOutput { stdout: Vec<u8>, stderr: Vec<u8>, stderr_truncated: bool, status: ExitStatus }

pub fn canonical_python(path: &Path) -> CoreResult<PathBuf> {
    // POSIX虚拟环境的python通常是符号链接，执行时必须保留入口路径才能找到pyvenv.cfg。
    let entry = if path.is_absolute() { path.to_path_buf() } else {
        std::env::current_dir().map_err(|_| CoreError::new("invalid_python", "无法确定Python入口的绝对路径"))?.join(path)
    };
    let resolved = entry.canonicalize().map_err(|_| CoreError::new("invalid_python", "指定的Python可执行文件不存在或无法访问"))?;
    let filename = entry.file_name().and_then(|n| n.to_str()).unwrap_or("").to_ascii_lowercase();
    let basename = filename.strip_suffix(".exe").unwrap_or(&filename);
    let allowed = basename.strip_prefix("python").is_some_and(|suffix| suffix.chars().all(|c| c.is_ascii_digit() || c == '.'));
    if !resolved.is_file() || !allowed { return Err(CoreError::new("invalid_python", "只能选择Python可执行文件，不能提交Shell或任意命令")); }
    Ok(entry)
}

pub fn probe_environment(python: &Path, limits: &ProcessLimits) -> CoreResult<EnvironmentInfo> {
    let executable = canonical_python(python)?;
    let request_id = Uuid::new_v4().to_string();
    let request = serde_json::to_vec(&serde_json::json!({"protocolVersion":PROTOCOL_VERSION,"requestId":request_id,"action":"describe"}))
        .map_err(|_| CoreError::new("invalid_request", "无法构建环境探测请求"))?;
    let probe_limits = ProcessLimits { max_wall_time: limits.max_wall_time.min(DESCRIBE_TIMEOUT), ..limits.clone() };
    let directory = executable.parent().ok_or_else(|| CoreError::new("invalid_python", "Python目录无效"))?;
    let output = exchange(&executable, FIXED_ENGINE_ARGUMENTS, directory, request, &probe_limits, Arc::new(AtomicBool::new(false)))?;
    let info = protocol::decode_capabilities(&output.stdout, &request_id, &executable.to_string_lossy())?;
    if !output.status.success() { return Err(CoreError::new("process_exit_error", "环境探测进程异常退出，未登记为就绪")); }
    Ok(info)
}

pub fn execute_engine(directory: &Path, request: &TraceRequest, environment: &EnvironmentInfo, limits: &ProcessLimits, cancelled: Arc<AtomicBool>) -> CoreResult<TraceResult> {
    request.config.validate()?;
    let executable = canonical_python(Path::new(&environment.python_executable))?;
    let input = serde_json::to_vec(request).map_err(|_| CoreError::new("invalid_request", "无法序列化冻结的计算请求"))?;
    let output = exchange(&executable, FIXED_ENGINE_ARGUMENTS, directory, input, limits, cancelled)?;
    let result = protocol::decode_result(&output.stdout, request, environment)?;
    if !output.status.success() { return Err(CoreError::new("process_exit_error", "计算进程异常退出，不能登记为完成")); }
    // 标准错误是有界诊断，不会被当作协议或自动送入AI上下文。
    let _diagnostic_summary = (output.stderr.len(), output.stderr_truncated);
    Ok(result)
}

/// 原始进程交换仅为内部实现与测试使用，桌面IPC不会暴露参数数组或任意程序入口。
fn exchange(executable: &Path, args: &[&str], directory: &Path, input: Vec<u8>, limits: &ProcessLimits, cancelled: Arc<AtomicBool>) -> CoreResult<ProcessOutput> {
    if input.len().saturating_add(1) > MAX_REQUEST_BYTES { return Err(CoreError::new("input_limit_exceeded", "请求超过1 MiB协议上限")); }
    if limits.max_wall_time.is_zero() || limits.poll_interval.is_zero() || limits.max_output_bytes == 0
        || limits.max_output_bytes > MAX_RESPONSE_BYTES || limits.max_stderr_bytes > MAX_STDERR_BYTES {
        return Err(CoreError::new("invalid_process_limits", "进程资源限制无效"));
    }
    if cancelled.load(Ordering::SeqCst) { return Err(CoreError::new("cancelled", "任务在启动前已取消")); }
    let mut command = Command::new(executable);
    command.args(args).current_dir(directory).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped())
        .env_remove("PYTHONPATH").env_remove("PYTHONHOME");
    #[cfg(windows)] {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    let mut child = command.spawn().map_err(|_| CoreError::new("process_spawn_failed", "无法启动已选择的Python计算进程"))?;
    let started = Instant::now();
    let overflow = Arc::new(AtomicBool::new(false));
    let stdout = capture(child.stdout.take().expect("已配置stdout管道"), limits.max_output_bytes, Some(Arc::clone(&overflow)));
    let stderr = capture(child.stderr.take().expect("已配置stderr管道"), limits.max_stderr_bytes, None);
    let mut stdin = child.stdin.take().expect("已配置stdin管道");
    let (writer_tx, writer_rx) = mpsc::channel();
    thread::spawn(move || {
        let outcome = stdin.write_all(&input).and_then(|_| stdin.write_all(b"\n")).and_then(|_| stdin.flush());
        drop(stdin);
        let _ = writer_tx.send(outcome.is_ok());
    });
    let mut failure = None;
    let status = loop {
        let reason = if cancelled.load(Ordering::SeqCst) { Some(CoreError::new("cancelled", "计算进程已收到取消要求")) }
            else if overflow.load(Ordering::SeqCst) { Some(CoreError::new("output_limit_exceeded", "标准输出超过已声明的协议上限")) }
            else if started.elapsed() >= limits.max_wall_time { Some(CoreError::new("process_timeout", "计算超过已声明的墙钟时间限制")) }
            else { None };
        if let Some(reason) = reason {
            // kill可能与自然退出竞争；无论如何都要wait确认，不能提前返回cancelled。
            let _ = child.kill();
            let exited = child.wait().map_err(|_| CoreError::new("process_state_unknown", "无法确认计算进程已经退出"))?;
            failure = Some(reason);
            break exited;
        }
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => thread::sleep(limits.poll_interval),
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(CoreError::new("process_state_unknown", "无法可靠读取计算进程状态"));
            }
        }
    };
    // 进程退出后仍需排空管道；读取线程的等待也有上限，避免UI后台任务永久悬挂。
    let out = stdout.recv_timeout(STREAM_DRAIN_TIMEOUT).map_err(|_| CoreError::new("stream_timeout", "进程已退出，但输出管道尚未关闭"))??;
    let err = stderr.recv_timeout(STREAM_DRAIN_TIMEOUT).map_err(|_| CoreError::new("stream_timeout", "进程已退出，但诊断管道尚未关闭"))??;
    if let Some(error) = failure { return Err(error); }
    if out.truncated { return Err(CoreError::new("output_limit_exceeded", "标准输出超过已声明的协议上限")); }
    if !writer_rx.recv_timeout(STREAM_DRAIN_TIMEOUT).unwrap_or(false) { return Err(CoreError::new("process_input_failed", "未能完整发送冻结请求")); }
    Ok(ProcessOutput { stdout: out.bytes, stderr: err.bytes, stderr_truncated: err.truncated, status })
}

struct Capture { bytes: Vec<u8>, truncated: bool }

fn capture<R: Read + Send + 'static>(mut reader: R, limit: usize, overflow: Option<Arc<AtomicBool>>) -> mpsc::Receiver<CoreResult<Capture>> {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let mut bytes = Vec::with_capacity(limit.min(STREAM_CHUNK_BYTES));
        let mut buffer = [0_u8; STREAM_CHUNK_BYTES];
        let mut truncated = false;
        let result = loop {
            match reader.read(&mut buffer) {
                Ok(0) => break Ok(Capture { bytes, truncated }),
                Ok(count) => {
                    let retained = count.min(limit.saturating_sub(bytes.len()));
                    bytes.extend_from_slice(&buffer[..retained]);
                    if retained < count {
                        truncated = true;
                        if let Some(flag) = &overflow { flag.store(true, Ordering::SeqCst); }
                    }
                }
                Err(_) => break Err(CoreError::new("process_stream_error", "读取进程协议或诊断失败")),
            }
        };
        let _ = sender.send(result);
    });
    receiver
}

#[cfg(test)]
#[path = "process_tests.rs"]
mod tests;
