//! 数值输入和跨进程响应均按冻结契约检查，不能将程序退出成功当成科学验证通过。

use workbench_core::{protocol, types::*};

fn config() -> EllisConfig {
    EllisConfig { throat_radius: 1.0, initial_radius: 10.0, impact_parameters: vec![0.0], max_affine_parameter: 1.0, sample_count: 2, relative_tolerance: 1e-9, absolute_tolerance: 1e-11 }
}

#[test]
fn configuration_rejects_nonfinite_and_resource_overflow() {
    let base = config();
    assert!(base.validate().is_ok());
    let mut c = base.clone(); c.throat_radius = f64::NAN; assert!(c.validate().is_err());
    c = base.clone(); c.relative_tolerance = f64::EPSILON; assert!(c.validate().is_err());
    c = base.clone(); c.relative_tolerance = 1.0; assert!(c.validate().is_err());
    c = base.clone(); c.absolute_tolerance = 0.0; assert!(c.validate().is_err());
    c = base.clone(); c.sample_count = 1; assert!(c.validate().is_err());
    c = base.clone(); c.sample_count = 100_000; c.impact_parameters = vec![0.0, 0.5]; assert!(c.validate().is_err());
    c = base.clone(); c.impact_parameters.clear(); assert!(c.validate().is_err());
    c = base.clone(); c.impact_parameters = vec![20.0]; assert!(c.validate().is_err());
    c = base.clone(); c.initial_radius = f64::MAX; assert!(c.validate().is_err());
    c = base; c.impact_parameters = vec![-0.5]; assert!(c.validate().is_ok());
}

#[test]
fn initial_radial_boundary_uses_the_shared_cross_language_operation_order() {
    let mut c = config(); c.throat_radius = 0.1; c.initial_radius = 1.0;
    c.impact_parameters = vec![1.004987562112089];
    assert!(c.validate().is_ok());
    c.impact_parameters = vec![1.0049875621120892];
    assert!(c.validate().is_err());
}

fn request() -> TraceRequest { TraceRequest::new("request-1".into(), config()) }

fn response() -> serde_json::Value {
    serde_json::json!({
        "protocolVersion":1,"requestId":"request-1","type":"result","config":config(),
        "environment":{"pythonVersion":"3.12.0","numpyVersion":"2.0","scipyVersion":"1.14"},
        "trajectories":[{"impactParameter":0.0,"termination":"budget_exhausted",
        "samples":[
            {"affine":0.0,"t":0.0,"l":10.0,"theta":1.5707963267948966,"phi":0.0,"kt":1.0,"kl":-1.0,"kTheta":0.0,"kPhi":0.0},
            {"affine":1.0,"t":1.0,"l":9.0,"theta":1.5707963267948966,"phi":0.0,"kt":1.0,"kl":-1.0,"kTheta":0.0,"kPhi":0.0}],
        "events":[],"diagnostics":{"maxEnergyError":0.0,"maxAngularMomentumError":0.0,"maxNullError":0.0,"maxEquatorialError":0.0,"turningRadiusError":null,"radialAnalyticError":null,"azimuthReferenceError":null},
        "validation":{"status":"inconclusive","checks":[{"name":"energy","actual":0.0,"threshold":1e-8,"passed":true}]}}]
    })
}

fn environment() -> EnvironmentInfo {
    EnvironmentInfo { python_executable:"python".into(), engine_version:"0.1.0".into(), python_version:"3.12.0".into(), numpy_version:"2.0".into(), scipy_version:"1.14".into(), engine_source_hash:"a".repeat(64) }
}

#[test]
fn strict_response_accepts_complete_matching_result() {
    let bytes = serde_json::to_vec(&response()).unwrap();
    let result = protocol::decode_result(&bytes, &request(), &environment()).unwrap();
    assert_eq!(result.validation_status(), ValidationStatus::Inconclusive);
}

#[test]
fn rejects_mismatched_request_config_environment_and_unknown_fields() {
    for change in ["requestId", "config", "environment", "unknown"] {
        let mut value = response();
        match change {
            "requestId" => value["requestId"] = "another".into(),
            "config" => value["config"]["throatRadius"] = 2.0.into(),
            "environment" => value["environment"]["numpyVersion"] = "changed".into(),
            _ => value["shell"] = "must-not-run".into(),
        }
        assert!(protocol::decode_result(&serde_json::to_vec(&value).unwrap(), &request(), &environment()).is_err(), "未拒绝{change}");
    }
}

#[test]
fn rejects_extra_messages_nonmonotonic_samples_and_false_validation() {
    let mut text = serde_json::to_vec(&response()).unwrap(); text.extend_from_slice(b"\n{}\n");
    assert!(protocol::decode_result(&text, &request(), &environment()).is_err());
    let mut value = response(); value["trajectories"][0]["samples"][1]["affine"] = 0.0.into();
    assert!(protocol::decode_result(&serde_json::to_vec(&value).unwrap(), &request(), &environment()).is_err());
    value = response(); value["trajectories"][0]["validation"]["status"] = "passed".into(); value["trajectories"][0]["validation"]["checks"] = serde_json::json!([]);
    assert!(protocol::decode_result(&serde_json::to_vec(&value).unwrap(), &request(), &environment()).is_err());
}

#[test]
fn incomplete_terminations_cannot_claim_passed_even_with_true_checks() {
    for termination in ["budget_exhausted", "solver_failed"] {
        let mut value = response(); value["trajectories"][0]["termination"] = termination.into();
        value["trajectories"][0]["validation"]["status"] = "passed".into();
        assert!(protocol::decode_result(&serde_json::to_vec(&value).unwrap(), &request(), &environment()).is_err());
    }
}

#[test]
fn budget_exhausted_can_remain_inconclusive_with_failed_diagnostic() {
    let mut value = response();
    value["trajectories"][0]["validation"]["checks"][0]["actual"] = 2e-8.into();
    value["trajectories"][0]["validation"]["checks"][0]["passed"] = false.into();
    let result = protocol::decode_result(&serde_json::to_vec(&value).unwrap(), &request(), &environment()).unwrap();
    assert_eq!(result.validation_status(), ValidationStatus::Inconclusive);
}

#[test]
fn rejects_events_beyond_actual_samples_and_missing_terminal_events() {
    for mutation in ["future", "missing_exit", "missing_return", "short_budget", "unordered"] {
        let mut value = response();
        match mutation {
            "future" => {
                value["trajectories"][0]["termination"] = "solver_failed".into();
                value["trajectories"][0]["samples"][1]["affine"] = 0.5.into();
                value["trajectories"][0]["events"] = serde_json::json!([{"kind":"throat","affine":0.75,"radius":0.0}]);
            }
            "missing_exit" => value["trajectories"][0]["termination"] = "through".into(),
            "missing_return" => value["trajectories"][0]["termination"] = "returned".into(),
            "short_budget" => value["trajectories"][0]["samples"][1]["affine"] = 0.5.into(),
            _ => value["trajectories"][0]["events"] = serde_json::json!([{"kind":"throat","affine":0.75,"radius":0.0},{"kind":"turning","affine":0.25,"radius":0.1}]),
        }
        assert!(protocol::decode_result(&serde_json::to_vec(&value).unwrap(), &request(), &environment()).is_err(), "未拒绝{mutation}");
    }
}

#[test]
fn duplicate_error_identity_or_code_is_not_silently_normalized() {
    for bytes in [
        br#"{"protocolVersion":1,"requestId":"wrong","requestId":"request-1","type":"error","error":{"code":"invalid_request","message":"bad","details":{}}}"#.as_slice(),
        br#"{"protocolVersion":1,"requestId":"request-1","type":"error","error":{"code":"internal_error","code":"invalid_request","message":"bad","details":{}}}"#.as_slice(),
    ] {
        assert_eq!(protocol::decode_result(bytes, &request(), &environment()).unwrap_err().code, "invalid_engine_response");
    }
}
