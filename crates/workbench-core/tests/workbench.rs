//! 独立业务入口到真实已安装Python引擎的端到端测试，不启动桌面或开发服务。

use std::{path::{Path, PathBuf}, thread, time::{Duration, Instant}};
use workbench_core::{Workbench, storage::ProjectStore, types::*};

fn python() -> PathBuf {
    if let Some(path) = std::env::var_os("WORKBENCH_TEST_PYTHON") { return path.into(); }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().parent().unwrap();
    #[cfg(windows)] let relative = ".venv/Scripts/python.exe";
    #[cfg(not(windows))] let relative = ".venv/bin/python";
    root.join(relative)
}
fn config() -> EllisConfig { EllisConfig { throat_radius:1.0,initial_radius:10.0,impact_parameters:vec![0.0],max_affine_parameter:25.0,sample_count:101,relative_tolerance:1e-9,absolute_tolerance:1e-11 } }

fn await_terminal(workbench: &Workbench, project: &str, run: &str) -> RunRecord {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let record = workbench.get_run(project, run).unwrap();
        if record.state.terminal() || record.state == RunState::Unknown { return record; }
        assert!(Instant::now() < deadline, "真实子进程未在测试期限内结束");
        thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn complete_real_workflow_preserves_identity_result_and_export() {
    let directory = tempfile::tempdir().unwrap(); let workbench = Workbench::new();
    let state = workbench.create_project(directory.path(), "真实径向光线").unwrap();
    let project = &state.project.id;
    let model = workbench.save_model(project, "径向基准", config()).unwrap();
    let environment = workbench.probe_environment(&python()).unwrap(); assert_eq!(environment.engine_version, "0.1.0");
    let report = workbench.prepare_run(project, &model.id, &python()).unwrap(); assert_eq!(report.status, PreflightStatus::Ready);
    assert_eq!(report.execution_limits.max_wall_time_seconds, 300);
    let run = workbench.start_run(project, &report.id).unwrap();
    assert!(workbench.start_run(project, &report.id).is_err());
    let finished = await_terminal(&workbench, project, &run.id);
    assert_eq!(finished.state, RunState::Completed, "{:?}", finished.error);
    assert_eq!(finished.validation_status, ValidationStatus::Passed);
    assert_eq!(finished.result.as_ref().unwrap().request_id, run.request.request_id);
    assert_eq!(workbench.cancel_run(project, &run.id).unwrap(), finished);
    let export = workbench.export_run(project, &run.id, directory.path()).unwrap();
    let bytes = std::fs::read(export.path).unwrap();
    assert_eq!(export.sha256, workbench_core::storage::sha256_bytes(&bytes));
    assert_eq!(workbench.get_project(project).unwrap().runs.len(), 1);
    workbench.shutdown().unwrap();
    let reopened = Workbench::new().open_project(Path::new(&state.project.path)).unwrap();
    assert_eq!(reopened.project.id, *project); assert_eq!(reopened.runs[0].state, RunState::Completed);
}

#[test]
fn cancellation_is_requested_before_becoming_terminal() {
    let directory = tempfile::tempdir().unwrap(); let workbench = Workbench::new();
    let state = workbench.create_project(directory.path(), "取消真实任务").unwrap(); let project = &state.project.id;
    let mut input = config(); input.impact_parameters = vec![1.0; 50]; input.sample_count = 1000; input.max_affine_parameter = 100_000.0;
    let model = workbench.save_model(project, "临界光线批次", input).unwrap();
    let report = workbench.prepare_run(project, &model.id, &python()).unwrap();
    let run = workbench.start_run(project, &report.id).unwrap();
    let requested = workbench.cancel_run(project, &run.id).unwrap(); assert_eq!(requested.state, RunState::Cancelling);
    let ended = await_terminal(&workbench, project, &run.id); assert_eq!(ended.state, RunState::Cancelled);
    assert!(ended.finished_at.is_some()); assert!(ended.result.is_none()); assert_eq!(ended.validation_status, ValidationStatus::NotRun);
    workbench.shutdown().unwrap();
}

#[test]
fn reopening_unmanaged_running_record_marks_unknown_without_reexecution() {
    let directory = tempfile::tempdir().unwrap(); let store = ProjectStore::create(directory.path(), "中断恢复").unwrap();
    let model = store.save_model("模型", config()).unwrap();
    let env = EnvironmentInfo { python_executable:python().to_string_lossy().into(),engine_version:"0.1.0".into(),python_version:"3".into(),numpy_version:"2".into(),scipy_version:"1".into(),engine_source_hash:"a".repeat(64) };
    let report = store.prepare(&model.id, env.clone(), ExecutionLimits::default(), vec![]).unwrap();
    let run = store.consume_preflight(&report.id, &env).unwrap();
    store.transition(&run.id, RunState::Running, None, None).unwrap();
    let workbench = Workbench::new(); let state = workbench.open_project(store.directory()).unwrap();
    assert_eq!(state.runs[0].state, RunState::Unknown); assert!(state.runs[0].result.is_none());
    assert!(workbench.start_run(&state.project.id, &report.id).is_err());
    assert!(workbench.get_project("unopened-project").is_err());
    workbench.shutdown().unwrap();
}

#[test]
fn real_output_overflow_persists_failed_run_and_exportable_error() {
    // 此上限可容纳真实describe响应，但不能容纳101个真实轨迹样本。
    const TEST_RESPONSE_LIMIT: usize = 1024;
    let directory = tempfile::tempdir().unwrap();
    let limits = workbench_core::limits::ProcessLimits { max_output_bytes: TEST_RESPONSE_LIMIT, ..Default::default() };
    let workbench = Workbench::with_limits(limits).unwrap();
    let state = workbench.create_project(directory.path(), "真实输出越界").unwrap();
    let project = &state.project.id;
    let model = workbench.save_model(project, "受限输出测试", config()).unwrap();
    let report = workbench.prepare_run(project, &model.id, &python()).unwrap();
    assert_eq!(report.status, PreflightStatus::Ready);
    assert_eq!(report.execution_limits.max_output_bytes, TEST_RESPONSE_LIMIT);
    let run = workbench.start_run(project, &report.id).unwrap();
    let finished = await_terminal(&workbench, project, &run.id);
    assert_eq!(finished.state, RunState::Failed);
    assert_eq!(finished.validation_status, ValidationStatus::NotRun);
    assert!(finished.result.is_none());
    assert!(finished.finished_at.is_some());
    assert_eq!(finished.error.as_ref().unwrap().code, "output_limit_exceeded");
    let exported = workbench.export_run(project, &run.id, directory.path()).unwrap();
    let bytes = std::fs::read(&exported.path).unwrap();
    let data: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(data["run"]["state"], "failed");
    assert_eq!(data["run"]["validationStatus"], "not_run");
    assert!(data["run"]["result"].is_null());
    assert_eq!(data["run"]["error"]["code"], "output_limit_exceeded");
    assert_eq!(exported.sha256, workbench_core::storage::sha256_bytes(&bytes));
    workbench.shutdown().unwrap();
}

#[test]
fn real_nonradial_and_critical_results_reassess_saved_evidence_without_inventing_quadrature() {
    const PAGE_SIZE: usize = 20;
    const AFFINE_BUDGET: f64 = 40.0;
    const RELATIVE_TOLERANCE: f64 = 1e-11;
    const ABSOLUTE_TOLERANCE: f64 = 1e-13;
    let directory = tempfile::tempdir().unwrap();
    let workbench = Workbench::new();
    let project = workbench.create_project(directory.path(), "真实非径向与临界复评").unwrap().project;
    let mut input = config();
    // 同时覆盖两种旋转方向的穿喉、临界渐近和返回支，全部样本来自实际已安装引擎。
    input.impact_parameters = vec![0.5, -0.5, 1.0, -1.0, 2.0, -2.0];
    input.max_affine_parameter = AFFINE_BUDGET;
    input.relative_tolerance = RELATIVE_TOLERANCE;
    input.absolute_tolerance = ABSOLUTE_TOLERANCE;
    let model = workbench.save_model(&project.id, "六支光线", input).unwrap();
    let report = workbench.prepare_run(&project.id, &model.id, &python()).unwrap();
    let run = workbench.start_run(&project.id, &report.id).unwrap();
    let complete = await_terminal(&workbench, &project.id, &run.id);
    assert_eq!(complete.state, RunState::Completed, "{:?}", complete.error);
    let rule = workbench.list_verification_rules(&project.id, 0, PAGE_SIZE).unwrap().rules.remove(0);
    let verified = workbench.execute_verification(&project.id, ExecuteVerification {
        run_id:run.id.clone(), rule_version_id:rule.id.clone(), client_request_id:"真实六支复评".into(), previous_record_id:None,
    }).unwrap();
    // 与引擎现有40预算临界回归一致：分离支数值漂移必须压过缺少求积证据的“不确定”。
    assert_eq!(verified.conclusion, VerificationConclusion::Failed);
    let trajectories = &complete.result.as_ref().unwrap().trajectories;
    for (index, trajectory) in trajectories.iter().enumerate() {
        let check = |metric| verified.checks.iter().find(|item| item.trajectory_index == Some(index) && item.metric_id == metric).unwrap();
        let quadrature = check(VerificationMetric::ReferenceQuadratureError);
        assert_eq!(quadrature.conclusion, VerificationConclusion::Inconclusive);
        assert_eq!(quadrature.reason_code, "quadrature_evidence_missing");
        assert!(quadrature.actual.is_none());
        for metric in [VerificationMetric::EnergyError, VerificationMetric::AngularMomentumError,
            VerificationMetric::NullError, VerificationMetric::EquatorialError] {
            assert_eq!(check(metric).conclusion, VerificationConclusion::Passed, "b={}，指标={metric:?}", trajectory.impact_parameter);
        }
        if trajectory.impact_parameter.abs() == model.config.throat_radius {
            assert_eq!(trajectory.termination, "budget_exhausted");
            assert_eq!(check(VerificationMetric::PropagationCompletion).conclusion, VerificationConclusion::NotApplicable);
            assert!([VerificationMetric::CriticalRelationError, VerificationMetric::CriticalDirectionError, VerificationMetric::CriticalPositionError]
                .iter().any(|metric|check(*metric).conclusion == VerificationConclusion::Failed));
        } else {
            assert_eq!(check(VerificationMetric::PropagationCompletion).conclusion, VerificationConclusion::Passed);
            assert_eq!(check(VerificationMetric::AzimuthReferenceError).conclusion, VerificationConclusion::Passed);
            if trajectory.impact_parameter.abs() > model.config.throat_radius {
                assert_eq!(trajectory.termination, "returned");
                assert_eq!(check(VerificationMetric::TurningRadiusError).conclusion, VerificationConclusion::Passed);
            } else { assert_eq!(trajectory.termination, "through"); }
        }
    }
    // 详情、历史和汇总应读取同一不可变验证记录，复评不得修改原运行。
    assert_eq!(workbench.get_verification_record(&project.id, &verified.id).unwrap(), verified);
    assert_eq!(workbench.list_verification_records(&project.id, &run.id, 0, PAGE_SIZE).unwrap().records, vec![verified.clone()]);
    assert_eq!(workbench.get_run_verification_state(&project.id, &run.id, &rule.id).unwrap().latest_record, Some(verified));
    assert_eq!(workbench.get_run(&project.id, &run.id).unwrap(), complete);
    workbench.shutdown().unwrap();
}

#[test]
fn real_short_critical_branch_passes_finite_scope_checks_without_claiming_propagation_completion() {
    // 引擎test_short_critical_trajectory_has_independent_branch_checks使用同一有限预算。
    const FINITE_CRITICAL_BUDGET: f64 = 1.0;
    let directory = tempfile::tempdir().unwrap();
    let workbench = Workbench::new();
    let project = workbench.create_project(directory.path(), "真实临界有限区间").unwrap().project;
    let mut input = config();
    input.impact_parameters = vec![1.0, -1.0];
    input.max_affine_parameter = FINITE_CRITICAL_BUDGET;
    let model = workbench.save_model(&project.id, "正反旋转临界有限区间", input).unwrap();
    let report = workbench.prepare_run(&project.id, &model.id, &python()).unwrap();
    let run = workbench.start_run(&project.id, &report.id).unwrap();
    let complete = await_terminal(&workbench, &project.id, &run.id);
    assert_eq!(complete.state, RunState::Completed, "{:?}", complete.error);
    let rule = workbench.list_verification_rules(&project.id, 0, 1).unwrap().rules.remove(0);
    let verified = workbench.execute_verification(&project.id, ExecuteVerification {
        run_id:run.id.clone(),rule_version_id:rule.id,client_request_id:"临界有限支复评".into(),previous_record_id:None,
    }).unwrap();
    assert_eq!(verified.conclusion, VerificationConclusion::Inconclusive);
    for (index, ray) in complete.result.as_ref().unwrap().trajectories.iter().enumerate() {
        assert_eq!(ray.termination, "budget_exhausted"); assert!(ray.events.is_empty());
        let check = |metric| verified.checks.iter().find(|item|item.trajectory_index == Some(index) && item.metric_id == metric).unwrap();
        for metric in [VerificationMetric::CriticalRelationError, VerificationMetric::CriticalDirectionError, VerificationMetric::CriticalPositionError] {
            assert_eq!(check(metric).conclusion, VerificationConclusion::Passed);
        }
        assert_eq!(check(VerificationMetric::PropagationCompletion).reason_code, "critical_finite_scope");
        assert_eq!(check(VerificationMetric::PropagationCompletion).conclusion, VerificationConclusion::NotApplicable);
        assert_eq!(check(VerificationMetric::AzimuthReferenceError).reason_code, "reference_endpoint_missing");
        assert_eq!(check(VerificationMetric::ReferenceQuadratureError).reason_code, "quadrature_evidence_missing");
    }
    assert_eq!(workbench.get_run(&project.id, &run.id).unwrap(), complete);
    workbench.shutdown().unwrap();
}

#[test]
fn real_scientific_samples_keep_their_bits_and_hash_after_json_roundtrip() {
    use std::sync::{Arc, atomic::AtomicBool};
    use workbench_core::{limits::ProcessLimits, process, storage::sha256_bytes};
    let directory = tempfile::tempdir().unwrap();
    let limits = ProcessLimits::default();
    let environment = process::probe_environment(&python(), &limits).unwrap();
    let mut input = config();
    input.impact_parameters = vec![0.5];
    let request = TraceRequest::new("真实科学数值往返".into(), input);
    let original = process::execute_engine(directory.path(), &request, &environment, &limits, Arc::new(AtomicBool::new(false))).unwrap();
    let encoded = serde_json::to_vec(&original).unwrap();
    let decoded: TraceResult = serde_json::from_slice(&encoded).unwrap();
    for (ray, restored_ray) in original.trajectories.iter().zip(&decoded.trajectories) {
        for (index, (sample, restored)) in ray.samples.iter().zip(&restored_ray.samples).enumerate() {
            // 科学产物的SHA-256以精确数值为基础，允许范围内的浮点数不能被JSON解析悄悄舍入。
            for (field, value, persisted) in [
                ("affine", sample.affine, restored.affine), ("t", sample.t, restored.t),
                ("l", sample.l, restored.l), ("theta", sample.theta, restored.theta),
                ("phi", sample.phi, restored.phi), ("kt", sample.kt, restored.kt),
                ("kl", sample.kl, restored.kl), ("kTheta", sample.k_theta, restored.k_theta),
                ("kPhi", sample.k_phi, restored.k_phi),
            ] {
                assert_eq!(value.to_bits(), persisted.to_bits(), "样本{index}字段{field}从{value:?}变成{persisted:?}");
            }
        }
    }
    assert_eq!(sha256_bytes(&encoded), sha256_bytes(&serde_json::to_vec(&decoded).unwrap()));
}
