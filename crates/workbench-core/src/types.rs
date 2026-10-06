//! 与固定桌面和科学进程契约对应的强类型对象，未知字段一律拒绝。

use serde::{Deserialize, Serialize};
use crate::{CoreError, CoreResult, limits::*};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EllisConfig {
    pub throat_radius: f64,
    pub initial_radius: f64,
    pub impact_parameters: Vec<f64>,
    pub max_affine_parameter: f64,
    pub sample_count: usize,
    pub relative_tolerance: f64,
    pub absolute_tolerance: f64,
}

impl EllisConfig {
    /// 初态表达式也必须能以有限浮点数求值，不能只检查输入本身。
    pub fn validate(&self) -> CoreResult<()> {
        let positive = [self.throat_radius, self.initial_radius, self.max_affine_parameter, self.absolute_tolerance];
        let denominator = self.initial_radius * self.initial_radius + self.throat_radius * self.throat_radius;
        let count_ok = (MIN_SAMPLES..=MAX_TOTAL_SAMPLES).contains(&self.sample_count)
            && (1..=MAX_RAYS).contains(&self.impact_parameters.len())
            && self.sample_count.checked_mul(self.impact_parameters.len()).is_some_and(|n| n <= MAX_TOTAL_SAMPLES);
        let tolerance_ok = self.relative_tolerance.is_finite()
            && self.relative_tolerance >= MIN_RELATIVE_TOLERANCE && self.relative_tolerance < 1.0;
        let rays_ok = denominator.is_finite() && denominator > 0.0
            && self.impact_parameters.iter().all(|b| {
                let square = b * b;
                let radial = 1.0 - square / denominator;
                let angular_velocity = b / denominator;
                b.is_finite() && square.is_finite() && radial.is_finite() && radial > 0.0 && angular_velocity.is_finite()
            });
        if !positive.iter().all(|n| n.is_finite() && *n > 0.0) || !count_ok || !tolerance_ok || !rays_ok {
            return Err(CoreError::new("invalid_config", "参数必须有限且满足正尺度、合法初态、容差和总采样数限制"));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EnvironmentInfo {
    pub python_executable: String,
    pub engine_version: String,
    pub python_version: String,
    pub numpy_version: String,
    pub scipy_version: String,
    pub engine_source_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExecutionLimits {
    pub max_wall_time_seconds: u64,
    pub max_output_bytes: usize,
    pub max_total_samples: usize,
}

impl Default for ExecutionLimits {
    fn default() -> Self {
        Self { max_wall_time_seconds: DEFAULT_WALL_TIME_SECONDS, max_output_bytes: MAX_RESPONSE_BYTES, max_total_samples: MAX_TOTAL_SAMPLES }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TraceRequest {
    pub protocol_version: u32,
    pub request_id: String,
    pub action: String,
    pub config: EllisConfig,
}

impl TraceRequest {
    pub fn new(request_id: String, config: EllisConfig) -> Self {
        Self { protocol_version: PROTOCOL_VERSION, request_id, action: "traceEllis".into(), config }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectSummary {
    pub id: String,
    pub name: String,
    pub path: String,
    pub created_at: String,
    pub schema_version: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModelVersion {
    pub id: String,
    pub project_id: String,
    pub label: String,
    pub created_at: String,
    pub config: EllisConfig,
    pub content_hash: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RunState { Queued, Running, Completed, Failed, Cancelling, Cancelled, Unknown }

impl RunState {
    pub fn terminal(self) -> bool { matches!(self, Self::Completed | Self::Failed | Self::Cancelled) }
    /// 状态转换是执行事实约束，终态不会因随后取消或重试而被覆写。
    pub fn permits(self, next: Self) -> bool {
        matches!((self, next),
            (Self::Queued, Self::Running | Self::Failed | Self::Cancelling | Self::Unknown)
            | (Self::Running, Self::Completed | Self::Failed | Self::Cancelling | Self::Unknown)
            | (Self::Cancelling, Self::Cancelled | Self::Unknown))
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ValidationStatus { NotRun, Passed, Failed, Inconclusive }

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PreflightStatus { Ready, Blocked }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PreflightReport {
    pub id: String,
    pub project_id: String,
    pub model_version_id: String,
    pub config: EllisConfig,
    pub environment: EnvironmentInfo,
    pub created_at: String,
    pub status: PreflightStatus,
    pub issues: Vec<CoreError>,
    pub execution_limits: ExecutionLimits,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RunRecord {
    pub id: String,
    pub project_id: String,
    pub model_version_id: String,
    pub created_at: String,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub state: RunState,
    pub validation_status: ValidationStatus,
    pub request: TraceRequest,
    pub environment: EnvironmentInfo,
    pub result: Option<TraceResult>,
    pub error: Option<CoreError>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectState { pub project: ProjectSummary, pub models: Vec<ModelVersion>, pub runs: Vec<RunRecord> }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExportedRun { pub path: String, pub sha256: String }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeVersions { pub python_version: String, pub numpy_version: String, pub scipy_version: String }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TraceResult {
    pub protocol_version: u32,
    pub request_id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub config: EllisConfig,
    pub trajectories: Vec<Trajectory>,
    pub environment: RuntimeVersions,
}

impl TraceResult {
    pub fn validation_status(&self) -> ValidationStatus {
        if self.trajectories.iter().any(|t| t.validation.status == ValidationStatus::Failed) { ValidationStatus::Failed }
        else if self.trajectories.iter().any(|t| t.validation.status != ValidationStatus::Passed) { ValidationStatus::Inconclusive }
        else { ValidationStatus::Passed }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Trajectory {
    pub impact_parameter: f64,
    pub termination: String,
    pub samples: Vec<Sample>,
    pub events: Vec<Event>,
    pub diagnostics: Diagnostics,
    pub validation: TrajectoryValidation,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Sample {
    pub affine: f64, pub t: f64, pub l: f64, pub theta: f64, pub phi: f64,
    pub kt: f64, pub kl: f64, pub k_theta: f64, pub k_phi: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Event { pub kind: String, pub affine: f64, pub radius: f64 }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Diagnostics {
    pub max_energy_error: f64, pub max_angular_momentum_error: f64,
    pub max_null_error: f64, pub max_equatorial_error: f64,
    pub turning_radius_error: Option<f64>, pub radial_analytic_error: Option<f64>, pub azimuth_reference_error: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TrajectoryValidation { pub status: ValidationStatus, pub checks: Vec<ValidationCheck> }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ValidationCheck { pub name: String, pub actual: f64, pub threshold: f64, pub passed: bool }
