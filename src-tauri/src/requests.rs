//! 桌面边界拒绝未知字段，业务校验继续由独立核心层执行。

use serde::Deserialize;
use std::path::PathBuf;
use workbench_core::types::EllisConfig;
use workbench_core::types::{ExecuteVerification, VerificationRuleDraft};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateProject { pub parent_directory: PathBuf, pub name: String }

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OpenProject { pub directory: PathBuf }

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectIdentity { pub project_id: String }

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SaveModel { pub project_id: String, pub label: String, pub config: EllisConfig }

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProbeEnvironment { pub python_executable: PathBuf }

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PrepareRun { pub project_id: String, pub model_version_id: String, pub python_executable: PathBuf }

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StartRun { pub project_id: String, pub preflight_id: String }

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RunIdentity { pub project_id: String, pub run_id: String }

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExportRun { pub project_id: String, pub run_id: String, pub destination_directory: PathBuf }

// 验证查询使用有界分页；规则草稿与执行请求继续由核心强类型拒绝未知字段。
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ListVerificationRules { pub project_id: String, pub offset: usize, pub limit: usize }

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SaveVerificationRule { pub project_id: String, pub draft: VerificationRuleDraft }

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RunVerificationIdentity { pub project_id: String, pub run_id: String, pub rule_version_id: String }

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExecuteVerificationRequest { pub project_id: String, pub request: ExecuteVerification }

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ListVerificationRecords { pub project_id: String, pub run_id: String, pub offset: usize, pub limit: usize }

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VerificationRecordIdentity { pub project_id: String, pub record_id: String }

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApplyProjectMigration { pub directory: PathBuf, pub plan_id: String }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_payload_accepts_the_documented_camel_case_contract() {
        let input: PrepareRun = serde_json::from_str(r#"{"projectId":"p","modelVersionId":"m","pythonExecutable":"python"}"#).unwrap();
        assert_eq!(input.project_id, "p");
        assert_eq!(input.model_version_id, "m");
        assert_eq!(input.python_executable, PathBuf::from("python"));
    }

    #[test]
    fn execution_payload_cannot_smuggle_an_arbitrary_command_or_duplicate_identity() {
        assert!(serde_json::from_str::<StartRun>(r#"{"projectId":"p","preflightId":"f","command":"arbitrary"}"#).is_err());
        assert!(serde_json::from_str::<RunIdentity>(r#"{"projectId":"p","runId":"one","runId":"two"}"#).is_err());
        assert!(serde_json::from_str::<ProjectIdentity>(r#"{"projectId":true}"#).is_err());
    }

    #[test]
    fn verification_and_migration_payloads_reject_hidden_execution_and_ambiguous_identity() {
        // 两层请求均严格解码；外来验证请求不能夹带命令、产物哈希或迁移目标。
        let valid = r#"{"projectId":"p","request":{"runId":"r","ruleVersionId":"v","clientRequestId":"c","previousRecordId":null}}"#;
        let decoded: ExecuteVerificationRequest = serde_json::from_str(valid).unwrap();
        assert_eq!(decoded.request.client_request_id, "c");
        assert!(serde_json::from_str::<ExecuteVerificationRequest>(r#"{"projectId":"p","request":{"runId":"r","ruleVersionId":"v","clientRequestId":"c","previousRecordId":null,"command":"python"}}"#).is_err());
        assert!(serde_json::from_str::<ListVerificationRules>(r#"{"projectId":"p","offset":-1,"limit":20}"#).is_err());
        assert!(serde_json::from_str::<ApplyProjectMigration>(r#"{"directory":"a","planId":"x","planId":"y"}"#).is_err());
        assert!(serde_json::from_str::<ApplyProjectMigration>(r#"{"directory":"a","planId":"x","backupPath":"unreviewed"}"#).is_err());
    }
}
