//! 严格验证进程响应的身份、结构和有限数值；不从图形或退出码推断科学通过。

use serde::Deserialize;
use crate::{CoreError, CoreResult, limits::PROTOCOL_VERSION, types::*};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Capabilities {
    protocol_version: u32,
    request_id: String,
    #[serde(rename = "type")]
    kind: String,
    engine_version: String,
    engine_source_hash: String,
    actions: Vec<String>,
    environment: RuntimeVersions,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ErrorEnvelope {
    protocol_version: u32,
    request_id: String,
    #[serde(rename = "type")]
    kind: String,
    error: EngineError,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EngineError { code: String, message: String, details: serde_json::Value }

fn malformed() -> CoreError { CoreError::new("invalid_engine_response", "科学引擎返回了无效或不匹配的协议响应") }

/// 错误也需要匹配本次身份；对外只传稳定说明，不转发引擎内部详情。
fn reject_engine_error(bytes: &[u8], request_id: &str) -> CoreResult<()> {
    let value: serde_json::Value = serde_json::from_slice(bytes).map_err(|_| malformed())?;
    if value.get("type").and_then(|v| v.as_str()) != Some("error") { return Ok(()); }
    let envelope: ErrorEnvelope = serde_json::from_slice(bytes).map_err(|_| malformed())?;
    if envelope.protocol_version != PROTOCOL_VERSION || envelope.request_id != request_id || envelope.kind != "error"
        || envelope.error.message.trim().is_empty() || !envelope.error.details.is_object() { return Err(malformed()); }
    let message = match envelope.error.code.as_str() {
        "invalid_request" => "科学引擎拒绝了输入参数",
        "unsupported_version" => "科学引擎不支持当前协议版本",
        "unsupported_action" => "科学引擎不支持所需计算能力",
        "computation_failed" => "计算未能产生合法的有限轨迹",
        "internal_error" => "科学引擎发生内部错误",
        _ => return Err(malformed()),
    };
    Err(CoreError::new(&envelope.error.code, message))
}

pub fn decode_capabilities(bytes: &[u8], request_id: &str, executable: &str) -> CoreResult<EnvironmentInfo> {
    reject_engine_error(bytes, request_id)?;
    let response: Capabilities = serde_json::from_slice(bytes).map_err(|_| malformed())?;
    let hash_ok = response.engine_source_hash.len() == 64
        && response.engine_source_hash.bytes().all(|c| c.is_ascii_hexdigit());
    if response.protocol_version != PROTOCOL_VERSION || response.request_id != request_id
        || response.kind != "capabilities" || !hash_ok || response.engine_version.is_empty()
        || !response.actions.iter().any(|a| a == "traceEllis")
        || !versions_present(&response.environment) { return Err(malformed()); }
    Ok(EnvironmentInfo { python_executable: executable.into(), engine_version: response.engine_version,
        python_version: response.environment.python_version, numpy_version: response.environment.numpy_version,
        scipy_version: response.environment.scipy_version, engine_source_hash: response.engine_source_hash })
}

fn versions_present(v: &RuntimeVersions) -> bool {
    [&v.python_version, &v.numpy_version, &v.scipy_version].iter().all(|s| !s.trim().is_empty() && !s.chars().any(char::is_control))
}

pub fn decode_result(bytes: &[u8], request: &TraceRequest, environment: &EnvironmentInfo) -> CoreResult<TraceResult> {
    reject_engine_error(bytes, &request.request_id)?;
    let result: TraceResult = serde_json::from_slice(bytes).map_err(|_| malformed())?;
    validate_result(&result, request, environment)?;
    Ok(result)
}

pub fn validate_result(result: &TraceResult, request: &TraceRequest, environment: &EnvironmentInfo) -> CoreResult<()> {
    request.config.validate()?;
    let same_config = serde_json::to_vec(&result.config).map_err(|_| malformed())?
        == serde_json::to_vec(&request.config).map_err(|_| malformed())?;
    let env = &result.environment;
    if request.protocol_version != PROTOCOL_VERSION || request.action != "traceEllis"
        || result.protocol_version != PROTOCOL_VERSION || result.request_id != request.request_id
        || request.request_id.is_empty() || result.kind != "result" || !same_config
        || env.python_version != environment.python_version || env.numpy_version != environment.numpy_version
        || env.scipy_version != environment.scipy_version
        || result.trajectories.len() != request.config.impact_parameters.len() { return Err(malformed()); }
    for (trajectory, impact) in result.trajectories.iter().zip(&request.config.impact_parameters) {
        if trajectory.impact_parameter != *impact || trajectory.samples.len() != request.config.sample_count
            || !matches!(trajectory.termination.as_str(), "through" | "returned" | "budget_exhausted" | "solver_failed") {
            return Err(malformed());
        }
        let mut previous = None;
        for sample in &trajectory.samples {
            let values = [sample.affine, sample.t, sample.l, sample.theta, sample.phi, sample.kt, sample.kl, sample.k_theta, sample.k_phi];
            if !values.iter().all(|v| v.is_finite()) || sample.affine < 0.0
                || sample.affine > request.config.max_affine_parameter
                || previous.is_some_and(|p| sample.affine <= p) { return Err(malformed()); }
            previous = Some(sample.affine);
        }
        if trajectory.samples.first().is_none_or(|sample| sample.affine != 0.0) { return Err(malformed()); }
        let final_affine = trajectory.samples.last().ok_or_else(malformed)?.affine;
        let mut previous_event = None;
        for event in &trajectory.events {
            if !matches!(event.kind.as_str(), "throat" | "turning" | "exit_negative" | "return_positive")
                || !event.affine.is_finite() || !event.radius.is_finite() || event.affine < 0.0
                || event.affine > final_affine
                || previous_event.is_some_and(|previous| event.affine < previous) { return Err(malformed()); }
            previous_event = Some(event.affine);
        }
        let terminal_event = trajectory.events.last();
        match trajectory.termination.as_str() {
            "through" if terminal_event.is_none_or(|e| e.kind != "exit_negative" || e.affine != final_affine || e.radius >= 0.0) => return Err(malformed()),
            "returned" if terminal_event.is_none_or(|e| e.kind != "return_positive" || e.affine != final_affine || e.radius <= 0.0) => return Err(malformed()),
            "budget_exhausted" if final_affine != request.config.max_affine_parameter
                || trajectory.events.iter().any(|e| matches!(e.kind.as_str(), "exit_negative" | "return_positive")) => return Err(malformed()),
            _ => {}
        }
        let d = &trajectory.diagnostics;
        let errors = [Some(d.max_energy_error), Some(d.max_angular_momentum_error), Some(d.max_null_error), Some(d.max_equatorial_error), d.turning_radius_error, d.radial_analytic_error, d.azimuth_reference_error];
        if !errors.into_iter().flatten().all(|v| v.is_finite() && v >= 0.0) { return Err(malformed()); }
        let validation = &trajectory.validation;
        if validation.status == ValidationStatus::NotRun { return Err(malformed()); }
        for check in &validation.checks {
            if check.name.is_empty() || !check.actual.is_finite() || !check.threshold.is_finite()
                || check.actual < 0.0 || check.threshold < 0.0 || check.passed != (check.actual <= check.threshold) { return Err(malformed()); }
        }
        // 预算耗尽仍可具有失败检查；只阻止无证据的“通过”，不篡改未判定状态。
        if validation.status == ValidationStatus::Passed
            && (validation.checks.is_empty() || validation.checks.iter().any(|c| !c.passed)
                || matches!(trajectory.termination.as_str(), "budget_exhausted" | "solver_failed")) { return Err(malformed()); }
    }
    Ok(())
}
