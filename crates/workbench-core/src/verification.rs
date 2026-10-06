//! Ellis诊断重评：规则版本与真实测量值比较，不继承旧通过标志。
use std::collections::HashSet;
use serde::Serialize;
use uuid::Uuid;
use crate::{CoreError, CoreResult, storage::{sha256_bytes, timestamp}, types::*};

pub(crate) const METHOD_ID: &str = "ellis-diagnostic-reassessment";
pub(crate) const METHOD_VERSION: u32 = 1;
pub(crate) const HASH_FORMAT: &str = "sha256:rust-typed-json-v1";
pub(crate) const MAX_PAGE_SIZE: usize = 100;
const MAX_TEXT_BYTES: usize = 4096;
const CONSERVATION_LIMIT: f64 = 1e-8;
const TRAJECTORY_LIMIT: f64 = 1e-6;
const QUADRATURE_LIMIT: f64 = 1e-10;

pub(crate) fn typed_hash<T: Serialize>(value: &T) -> CoreResult<String> {
    serde_json::to_vec(value).map(|bytes| sha256_bytes(&bytes)).map_err(|_| invalid("无法序列化验证内容"))
}
pub(crate) fn rule_hash(rule: &VerificationRuleVersion) -> CoreResult<String> {
    let mut payload = rule.clone(); payload.content_hash.clear(); typed_hash(&payload)
}
pub(crate) fn record_hash(record: &VerificationRecord) -> CoreResult<String> {
    let mut payload = record.clone(); payload.content_hash.clear(); typed_hash(&payload)
}
fn invalid(message: &str) -> CoreError { CoreError::new("invalid_verification_rule", message) }
pub(crate) fn text_valid(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= MAX_TEXT_BYTES && !value.chars().any(|c| c.is_control())
}
fn paragraph_valid(value:&str)->bool {
    !value.trim().is_empty() && value.len()<=MAX_TEXT_BYTES && !value.chars().any(|c|c.is_control() && !matches!(c,'\n'|'\r'|'\t'))
}
pub(crate) fn page_valid(offset: usize, limit: usize) -> CoreResult<()> {
    if limit == 0 || limit > MAX_PAGE_SIZE || offset > i64::MAX as usize { return Err(CoreError::new("invalid_pagination", "分页limit必须为1至100，offset必须为有效非负整数")); }
    Ok(())
}

/// 默认规则明确声明证据范围；未保存的内部积分步及求积误差不能补造。
pub(crate) fn default_checks() -> Vec<VerificationRuleCheck> {
    use VerificationMetric::*;
    let definitions = [
        (EnergyError,"能量守恒误差",CONSERVATION_LIMIT,"1","全部光线","引擎汇总及保存样本；内部接受步仅有引擎汇总"),
        (AngularMomentumError,"角动量守恒误差",CONSERVATION_LIMIT,"1","全部光线","按喉尺度a归一；引擎汇总及保存样本"),
        (NullError,"零条件误差",CONSERVATION_LIMIT,"1","全部光线","E=1；引擎汇总及保存样本"),
        (EquatorialError,"赤道面偏离",CONSERVATION_LIMIT,"rad","全部光线","引擎汇总及保存样本"),
        (TurningRadiusError,"解析转向半径误差",TRAJECTORY_LIMIT,"1","abs(b)>a","已保存转向事件根，按a归一"),
        (RadialAnalyticError,"径向解析解误差",TRAJECTORY_LIMIT,"1","b=0","引擎汇总及保存样本，长度/时间按a归一"),
        (AzimuthReferenceError,"独立方位角参考误差",TRAJECTORY_LIMIT,"rad","已穿喉或返回的光线","非径向参考仅重评引擎汇总；径向phi=0可由样本重算"),
        (ThroatTimeError,"径向穿喉时刻误差",TRAJECTORY_LIMIT,"1","b=0","事件时刻与R比较，按a归一"),
        (ExitTimeError,"径向负侧出口时刻误差",TRAJECTORY_LIMIT,"1","b=0","事件时刻与2R比较，按a归一"),
        (CriticalRelationError,"临界分离关系偏离",CONSERVATION_LIMIT,"1","abs(b)=a","仅保存样本；不代表内部接受步独立复核"),
        (CriticalDirectionError,"临界入射方向偏离",CONSERVATION_LIMIT,"1","abs(b)=a","仅保存样本；不代表内部接受步独立复核"),
        (CriticalPositionError,"临界正侧位置偏离",CONSERVATION_LIMIT,"1","abs(b)=a","仅保存样本；位置按a归一"),
        (ReferenceQuadratureError,"参考求积误差预算",QUADRATURE_LIMIT,"rad","b不等于0且已穿喉或返回","v1稳定diagnostics未保存求积误差估计；必须报告证据不足"),
        (PropagationCompletion,"传播终止条件",0.0,"1","全部光线","冻结终止原因与事件；预算耗尽不视为完整传播"),
    ];
    definitions.into_iter().map(|(metric_id,title,threshold,unit,applicability,evidence_scope)| VerificationRuleCheck {
        metric_id,title:title.into(),threshold,unit:unit.into(),applicability:applicability.into(),evidence_scope:evidence_scope.into(),
        basis:"PRD第21章SCI07的对应误差与终止条件；仅重评已保存的Ellis SciPy证据，不表示GYOTO交叉验证或SCI07整体通过".into(),
    }).collect()
}

pub(crate) fn builtin_rule(project_id: &str) -> CoreResult<VerificationRuleVersion> {
    let id = Uuid::new_v4().to_string();
    let mut rule = VerificationRuleVersion { id:id.clone(),project_id:project_id.into(),rule_family_id:id,parent_version_id:None,
        title:"Ellis SciPy诊断重评".into(),created_at:timestamp(),created_by:"workbench_builtin".into(),method_id:METHOD_ID.into(),method_version:METHOD_VERSION,
        change_reason:"建立有明确证据范围的默认规则".into(),builtin:true,checks:default_checks(),content_hash:String::new() };
    rule.content_hash = rule_hash(&rule)?; Ok(rule)
}

pub(crate) fn derive_rule(base: &VerificationRuleVersion, draft: VerificationRuleDraft) -> CoreResult<VerificationRuleVersion> {
    if !text_valid(&draft.title) || !paragraph_valid(&draft.change_reason) || draft.thresholds.len() != base.checks.len() {
        return Err(invalid("规则名称、变更理由必须完整，且必须保留全部稳定检查指标"));
    }
    let mut seen = HashSet::new(); let mut checks = base.checks.clone();
    for threshold in draft.thresholds {
        if !seen.insert(threshold.metric_id) || !threshold.threshold.is_finite() || threshold.threshold < 0.0 || !paragraph_valid(&threshold.basis) {
            return Err(invalid("指标不能重复，阈值须为有限非负数，每项必须有明确依据"));
        }
        let check = checks.iter_mut().find(|check| check.metric_id == threshold.metric_id).ok_or_else(|| invalid("不支持此验证指标"))?;
        if threshold.metric_id == VerificationMetric::PropagationCompletion && threshold.threshold != 0.0 { return Err(invalid("传播终止条件不能通过放宽数值阈值绕过")); }
        check.threshold = threshold.threshold; check.basis = threshold.basis;
    }
    let mut rule = VerificationRuleVersion { id:Uuid::new_v4().to_string(),project_id:base.project_id.clone(),rule_family_id:base.rule_family_id.clone(),parent_version_id:Some(base.id.clone()),
        title:draft.title,created_at:timestamp(),created_by:"local_user".into(),method_id:METHOD_ID.into(),method_version:METHOD_VERSION,
        change_reason:draft.change_reason,builtin:false,checks,content_hash:String::new() };
    rule.content_hash = rule_hash(&rule)?; Ok(rule)
}

pub(crate) fn validate_rule(rule: &VerificationRuleVersion) -> CoreResult<()> {
    let expected = default_checks(); let mut seen = HashSet::new();
    if rule.method_id != METHOD_ID || rule.method_version != METHOD_VERSION || rule.content_hash != rule_hash(rule)?
        || !text_valid(&rule.title) || !paragraph_valid(&rule.change_reason) || rule.checks.len() != expected.len()
        || Uuid::parse_str(&rule.id).is_err() || Uuid::parse_str(&rule.rule_family_id).is_err()
        || chrono::DateTime::parse_from_rfc3339(&rule.created_at).is_err() { return Err(invalid("已保存规则的身份或内容损坏")); }
    for check in &rule.checks {
        let standard = expected.iter().find(|item| item.metric_id == check.metric_id).ok_or_else(|| invalid("未知指标"))?;
        if !seen.insert(check.metric_id) || check.title != standard.title || check.unit != standard.unit || check.applicability != standard.applicability
            || check.evidence_scope != standard.evidence_scope || !check.threshold.is_finite() || check.threshold < 0.0 || !paragraph_valid(&check.basis)
            || (check.metric_id == VerificationMetric::PropagationCompletion && check.threshold != 0.0)
            || (rule.builtin && check != standard) { return Err(invalid("已保存规则的指标或门槛损坏")); }
    }
    Ok(())
}

pub(crate) fn aggregate(checks: &[VerificationCheckResult]) -> VerificationConclusion {
    use VerificationConclusion::*;
    if checks.iter().any(|c| c.conclusion == Failed) { Failed }
    else if checks.iter().any(|c| c.conclusion == MissingArtifact) { MissingArtifact }
    else if checks.iter().any(|c| matches!(c.conclusion, Inconclusive | NotRun)) || checks.is_empty() { Inconclusive }
    else if checks.iter().all(|c| c.conclusion == NotApplicable) { NotApplicable }
    else { Passed }
}

/// 不解析中文旧checks；stable diagnostics、原始样本和事件是唯一测量来源。
pub(crate) fn evaluate(run: &RunRecord, rule: &VerificationRuleVersion) -> Vec<VerificationCheckResult> {
    let Some(result) = &run.result else {
        return rule.checks.iter().map(|rule| outcome(rule, None, None, None, VerificationConclusion::MissingArtifact,
            "result_missing", "运行没有已登记完整产物，未用零值代替", vec![])).collect();
    };
    result.trajectories.iter().enumerate().flat_map(|(index,ray)| {
        rule.checks.iter().map(move |check| evaluate_check(&run.request.config, ray, index, check))
    }).collect()
}

fn outcome(rule: &VerificationRuleCheck, index: Option<usize>, impact: Option<f64>, actual: Option<f64>, conclusion: VerificationConclusion,
    reason: &str, message: &str, paths: Vec<String>) -> VerificationCheckResult {
    VerificationCheckResult {metric_id:rule.metric_id,trajectory_index:index,impact_parameter:impact,title:rule.title.clone(),actual,
        threshold:rule.threshold,unit:rule.unit.clone(),basis:rule.basis.clone(),conclusion,reason_code:reason.into(),message:message.into(),
        evidence_scope:rule.evidence_scope.clone(),evidence_paths:paths}
}

fn evaluate_check(config: &EllisConfig, ray: &Trajectory, index: usize, rule: &VerificationRuleCheck) -> VerificationCheckResult {
    use VerificationConclusion::*; use VerificationMetric::*;
    let root = format!("/trajectories/{index}");
    let emit = |value, status, reason, message, paths| outcome(rule,Some(index),Some(ray.impact_parameter),value,status,reason,message,paths);
    let a = config.throat_radius; let b = ray.impact_parameter;
    let completed = matches!(ray.termination.as_str(),"through"|"returned");
    let applicable = match rule.metric_id {
        TurningRadiusError => b.abs() > a,
        RadialAnalyticError|ThroatTimeError|ExitTimeError => b == 0.0,
        CriticalRelationError|CriticalDirectionError|CriticalPositionError => b.abs() == a,
        ReferenceQuadratureError => b != 0.0,
        _ => true,
    };
    if !applicable { return emit(None,NotApplicable,"condition_not_applicable","冻结输入不满足此指标的适用条件",vec!["/config/impactParameters".into()]); }
    if rule.metric_id == ReferenceQuadratureError {
        return emit(None,Inconclusive,"quadrature_evidence_missing","稳定产物未保存独立求积误差估计；未读取中文旧检查或继承其通过状态",vec![format!("{root}/diagnostics")]);
    }
    if rule.metric_id == PropagationCompletion {
        if !initial_state_consistent(config,ray) || !branch_evidence_consistent(config,ray) {
            return emit(Some(1.0),Failed,"propagation_evidence_conflict","初始切向量、传播分支、事件或末端样本不一致",vec![format!("{root}/samples"),format!("{root}/events"),format!("{root}/termination")]);
        }
        if b.abs()==a {return emit(None,NotApplicable,"critical_finite_scope","临界光线仅检验已保存有限区间的分离支，不要求有限时间完成传播",vec![format!("{root}/termination")]);}
        if ray.termination == "budget_exhausted" { return emit(None,Inconclusive,"propagation_budget_exhausted","有限仿射预算耗尽，传播归类仍未完成",vec![format!("{root}/termination")]); }
        if ray.termination == "solver_failed" { return emit(Some(1.0),Failed,"solver_failed","求解器没有正常完成",vec![format!("{root}/termination")]); }
    }
    if rule.metric_id == AzimuthReferenceError && !completed {
        return emit(None,Inconclusive,"reference_endpoint_missing","未到达完整有限端点，不能重评全轨迹方位角参考",vec![format!("{root}/termination")]);
    }
    let maximum = |f: &dyn Fn(&Sample)->f64| -> Option<f64> {
        let mut result: f64 = 0.0;
        for sample in &ray.samples { let value = f(sample); if !value.is_finite() || value < 0.0 { return None; } result = result.max(value); }
        Some(result)
    };
    let with_report = |reported: f64, recomputed: Option<f64>| recomputed.map(|v| v.max(reported));
    let mut paths = vec![format!("{root}/samples")];
    let value = match rule.metric_id {
        EnergyError => { paths.push(format!("{root}/diagnostics/maxEnergyError")); with_report(ray.diagnostics.max_energy_error, maximum(&|s| (s.kt-1.0).abs())) },
        AngularMomentumError => { paths.push(format!("{root}/diagnostics/maxAngularMomentumError")); with_report(ray.diagnostics.max_angular_momentum_error,maximum(&|s| ((s.l*s.l+a*a)*s.theta.sin().powi(2)*s.k_phi-b).abs()/a)) },
        NullError => { paths.push(format!("{root}/diagnostics/maxNullError")); with_report(ray.diagnostics.max_null_error,maximum(&|s| (-s.kt*s.kt+s.kl*s.kl+(s.l*s.l+a*a)*(s.k_theta*s.k_theta+s.theta.sin().powi(2)*s.k_phi*s.k_phi)).abs())) },
        EquatorialError => { paths.push(format!("{root}/diagnostics/maxEquatorialError")); with_report(ray.diagnostics.max_equatorial_error,maximum(&|s| (s.theta-std::f64::consts::FRAC_PI_2).abs())) },
        RadialAnalyticError => {
            paths.push(format!("{root}/diagnostics/radialAnalyticError"));
            let sample = maximum(&|s| ((s.l-(config.initial_radius-s.affine)).abs()/a).max((s.t-s.affine).abs()/a).max(s.phi.abs())
                .max((s.kl+1.0).abs()).max(a*s.k_theta.abs()).max(a*s.k_phi.abs()).max((s.kt-1.0).abs()));
            match ray.diagnostics.radial_analytic_error { Some(d) => with_report(d,sample),None => sample }
        },
        TurningRadiusError|ThroatTimeError|ExitTimeError => {
            let kind = if rule.metric_id == TurningRadiusError {"turning"} else if rule.metric_id == ThroatTimeError {"throat"} else {"exit_negative"};
            paths = vec![format!("{root}/events")];
            let events:Vec<_> = ray.events.iter().filter(|e| e.kind == kind).collect();
            if events.is_empty() { return emit(None,Inconclusive,"event_evidence_missing","所需事件根尚未保存，不能由离散样本最小值补造",paths); }
            Some(events.iter().map(|e| if rule.metric_id == TurningRadiusError { (e.radius-(b*b-a*a).sqrt()).abs()/a }
                else { (e.affine-config.initial_radius*if rule.metric_id==ThroatTimeError {1.0}else{2.0}).abs()/a }).fold(0.0,f64::max))
        },
        AzimuthReferenceError => { paths = vec![format!("{root}/diagnostics/azimuthReferenceError")];
            if b == 0.0 { paths.push(format!("{root}/samples")); maximum(&|s| s.phi.abs()).map(|v| v.max(ray.diagnostics.azimuth_reference_error.unwrap_or(0.0))) }
            else { ray.diagnostics.azimuth_reference_error } },
        CriticalRelationError => maximum(&|s| (s.kl+s.l/s.l.hypot(a)).abs()),
        CriticalDirectionError => maximum(&|s| s.kl.max(0.0)),
        CriticalPositionError => maximum(&|s| (-s.l).max(0.0)/a),
        PropagationCompletion => { paths = vec![format!("{root}/termination"),format!("{root}/events")]; Some(0.0) },
        ReferenceQuadratureError => None,
    };
    let Some(actual) = value.filter(|v| v.is_finite() && *v >= 0.0) else {
        return emit(None,Inconclusive,"measurement_unavailable","缺少有限、可解释的测量证据",paths);
    };
    if actual <= rule.threshold { emit(Some(actual),Passed,"within_threshold","实际测量满足所选规则；范围以证据说明为准",paths) }
    else { emit(Some(actual),Failed,"threshold_exceeded","实际测量超过所选规则门槛",paths) }
}

/// 守恒量只约束切向量的平方；必须另外验证入射方向和真实初态。
fn initial_state_consistent(config:&EllisConfig,ray:&Trajectory)->bool {
    let Some(first)=ray.samples.first() else{return false;};let a=config.throat_radius;let r=config.initial_radius;let b=ray.impact_parameter;
    let area=r*r+a*a;let radial=-(1.0-b*b/area).sqrt();
    (first.l-r).abs()/a<=TRAJECTORY_LIMIT && first.t.abs()/a<=TRAJECTORY_LIMIT && first.phi.abs()<=TRAJECTORY_LIMIT
        && (first.theta-std::f64::consts::FRAC_PI_2).abs()<=CONSERVATION_LIMIT && (first.kt-1.0).abs()<=CONSERVATION_LIMIT
        && (first.kl-radial).abs()<=CONSERVATION_LIMIT && a*first.k_theta.abs()<=CONSERVATION_LIMIT && a*(first.k_phi-b/area).abs()<=CONSERVATION_LIMIT
}

/// 终态要与物理分支、唯一终末事件、末端位置和方向同时一致。
fn branch_evidence_consistent(config:&EllisConfig,ray:&Trajectory)->bool {
    let a=config.throat_radius;let b=ray.impact_parameter;let Some(last)=ray.samples.last()else{return false;};
    let count=|kind:&str|ray.events.iter().filter(|e|e.kind==kind).count();
    if b.abs()==a {return ray.termination=="budget_exhausted" && ray.events.is_empty();}
    if !matches!(ray.termination.as_str(),"through"|"returned"){return true;}
    let through=b.abs()<a;
    let expected_termination=if through{"through"}else{"returned"};let expected_kind=if through{"exit_negative"}else{"return_positive"};
    if ray.termination!=expected_termination || count(expected_kind)!=1 || count("throat")!=usize::from(through)
        || count("turning")!=usize::from(!through) || count(if through{"return_positive"}else{"exit_negative"})!=0 {return false;}
    let target=if through{-config.initial_radius}else{config.initial_radius};
    let terminal=ray.events.iter().find(|e|e.kind==expected_kind).expect("数量已经验证");
    if (last.l-target).abs()/a>TRAJECTORY_LIMIT || (terminal.radius-last.l).abs()/a>TRAJECTORY_LIMIT
        || terminal.affine!=last.affine || (through && last.kl>=0.0) || (!through && last.kl<=0.0){return false;}
    if ray.events.iter().any(|e|e.kind=="throat" && e.radius.abs()/a>TRAJECTORY_LIMIT){return false;}
    let turning_time=ray.events.iter().find(|e|e.kind=="turning").map(|e|e.affine);
    ray.samples.iter().all(|s|if through || turning_time.is_some_and(|t|s.affine<t){s.kl<=CONSERVATION_LIMIT}else{s.kl>=-CONSERVATION_LIMIT})
}
