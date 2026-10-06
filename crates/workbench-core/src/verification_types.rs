//! B22独立验证的版本化数据契约；与原始引擎结果、运行状态完全分离。
use serde::{Deserialize, Serialize};
use crate::CoreError;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all="snake_case")]
pub enum VerificationMetric { EnergyError, AngularMomentumError, NullError, EquatorialError,
    TurningRadiusError, RadialAnalyticError, AzimuthReferenceError, ThroatTimeError, ExitTimeError,
    CriticalRelationError, CriticalDirectionError, CriticalPositionError, ReferenceQuadratureError, PropagationCompletion }

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all="snake_case")]
pub enum VerificationConclusion { NotRun, Passed, Failed, MissingArtifact, NotApplicable, Inconclusive }

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all="snake_case")]
pub enum VerificationExecutionStatus { Completed, Failed, Interrupted }

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all="snake_case")]
pub enum ResultIdentityOrigin { CapturedAtCompletion, ObservedAtMigration, NoResult }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
pub struct VerificationRuleCheck { pub metric_id: VerificationMetric, pub title: String, pub threshold: f64,
    pub unit: String, pub basis: String, pub applicability: String, pub evidence_scope: String }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
pub struct VerificationRuleVersion { pub id: String, pub project_id: String, pub rule_family_id: String,
    pub parent_version_id: Option<String>, pub title: String, pub created_at: String, pub created_by: String,
    pub method_id: String, pub method_version: u32, pub change_reason: String, pub builtin: bool,
    pub checks: Vec<VerificationRuleCheck>, pub content_hash: String }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
pub struct VerificationThreshold { pub metric_id: VerificationMetric, pub threshold: f64, pub basis: String }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
pub struct VerificationRuleDraft { pub base_version_id: String, pub title: String,
    pub change_reason: String, pub thresholds: Vec<VerificationThreshold> }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
pub struct ExecuteVerification { pub run_id: String, pub rule_version_id: String,
    pub client_request_id: String, pub previous_record_id: Option<String> }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
pub struct VerificationSourceIdentity { pub request_hash: String, pub environment_hash: String,
    pub result_hash: Option<String>, pub hash_format: String, pub result_origin: ResultIdentityOrigin }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
pub struct VerificationCheckResult { pub metric_id: VerificationMetric, pub trajectory_index: Option<usize>,
    pub impact_parameter: Option<f64>, pub title: String, pub actual: Option<f64>, pub threshold: f64,
    pub unit: String, pub basis: String, pub conclusion: VerificationConclusion, pub reason_code: String,
    pub message: String, pub evidence_scope: String, pub evidence_paths: Vec<String> }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
pub struct VerificationRecord { pub id: String, pub project_id: String, pub request_id: String,
    pub client_request_id: String, pub run_id: String, pub rule_version_id: String, pub previous_record_id: Option<String>,
    pub started_at: String, pub finished_at: String, pub executed_by: String, pub method_id: String,
    pub method_version: u32, pub execution_status: VerificationExecutionStatus, pub conclusion: VerificationConclusion,
    pub source: VerificationSourceIdentity, pub checks: Vec<VerificationCheckResult>, pub error: Option<CoreError>, pub content_hash: String }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
pub struct RunVerificationState { pub run_id: String, pub rule_version_id: String,
    pub conclusion: VerificationConclusion, pub latest_record: Option<VerificationRecord>,
    pub record_count: usize, pub pending_request_id: Option<String> }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
pub struct VerificationRulePage { pub rules: Vec<VerificationRuleVersion>, pub total: usize, pub next_offset: Option<usize> }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
pub struct VerificationRecordPage { pub records: Vec<VerificationRecord>, pub total: usize, pub next_offset: Option<usize> }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
pub struct ProjectMigrationPlan { pub id: String, pub directory: String, pub project_id: String,
    pub project_name: String, pub from_version: u32, pub to_version: u32, pub created_at: String,
    pub source_fingerprint: String, pub backup_path: String, pub changes: Vec<String>,
    pub warnings: Vec<String>, pub requires_confirmation: bool }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
pub struct ProjectMigrationReceipt { pub plan_id: String, pub directory: String, pub project_id: String,
    pub from_version: u32, pub to_version: u32, pub backup_path: String, pub backup_sha256: String,
    pub migrated_at: String, pub legacy_result_count: usize }
