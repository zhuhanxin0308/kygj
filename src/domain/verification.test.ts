import { describe, expect, it } from 'vitest';
import { verificationDraftSchema, verificationRecordSchema, verificationRuleSchema, verificationStateSchema } from './verification';
import { verificationRecord, verificationRule, verificationState } from '../test/verificationFixtures';

// 独立检查与执行完成分开；缺证据、篡改布尔结论和身份漂移不能进入界面。
describe('独立验证接收契约', () => {
  it('接受冻结规则、真实记录及未执行状态', () => {
    expect(verificationRuleSchema.parse(verificationRule)).toEqual(verificationRule);
    expect(verificationRecordSchema.parse(verificationRecord)).toEqual(verificationRecord);
    expect(verificationStateSchema.parse(verificationState)).toEqual(verificationState);
  });
  it('拒绝空检查通过、无产物通过和超阈值通过', () => {
    expect(verificationRecordSchema.safeParse({ ...verificationRecord, checks: [] }).success).toBe(false);
    expect(verificationRecordSchema.safeParse({ ...verificationRecord, source: { ...verificationRecord.source, resultHash: null, resultOrigin: 'no_result' } }).success).toBe(false);
    const broken = structuredClone(verificationRecord); broken.checks[0].actual = 1;
    expect(verificationRecordSchema.safeParse(broken).success).toBe(false);
    broken.checks[0].actual = null;
    expect(verificationRecordSchema.safeParse(broken).success).toBe(false);
  });
  it('失败执行不能通过，状态不能冒用其他运行或规则的历史', () => {
    expect(verificationRecordSchema.safeParse({ ...verificationRecord, executionStatus: 'failed' }).success).toBe(false);
    expect(verificationStateSchema.safeParse({ ...verificationState, conclusion: 'passed' }).success).toBe(false);
    expect(verificationStateSchema.safeParse({ ...verificationState, latestRecord: verificationRecord, conclusion: 'passed', recordCount: 1, runId: 'other' }).success).toBe(false);
    expect(verificationStateSchema.safeParse({ ...verificationState, latestRecord: verificationRecord, conclusion: 'passed', recordCount: 1, ruleVersionId: 'other' }).success).toBe(false);
  });
  it('依据与变更理由必须填写，指标不能重复且阈值必须有限非负', () => {
    const draft = { baseVersionId: verificationRule.id, title: '修订', changeReason: '按研究方案冻结', thresholds: [{ metricId: 'energy_error', threshold: 1e-8, basis: '研究方案第 2 节' }] };
    expect(verificationDraftSchema.safeParse(draft).success).toBe(true);
    expect(verificationDraftSchema.safeParse({ ...draft, changeReason: ' ' }).success).toBe(false);
    expect(verificationDraftSchema.safeParse({ ...draft, thresholds: [...draft.thresholds, ...draft.thresholds] }).success).toBe(false);
    expect(verificationDraftSchema.safeParse({ ...draft, thresholds: [{ ...draft.thresholds[0], threshold: Infinity }] }).success).toBe(false);
    expect(verificationDraftSchema.safeParse({ ...draft, thresholds: [{ ...draft.thresholds[0], basis: '' }] }).success).toBe(false);
  });
  it('待执行请求与旧记录并存时只能显示进行中，适用检查全通过才能为绿', () => {
    expect(verificationStateSchema.safeParse({ ...verificationState, latestRecord: verificationRecord, recordCount: 1, pendingRequestId: 'pending', conclusion: 'inconclusive' }).success).toBe(true);
    expect(verificationStateSchema.safeParse({ ...verificationState, latestRecord: verificationRecord, recordCount: 1, pendingRequestId: 'pending', conclusion: 'passed' }).success).toBe(false);
    const withInapplicable = { ...verificationRecord, checks: [...verificationRecord.checks, { ...verificationRecord.checks[0], metricId: 'turning_radius_error', actual: null, conclusion: 'not_applicable' }] };
    expect(verificationRecordSchema.safeParse(withInapplicable).success).toBe(true);
    expect(verificationRecordSchema.safeParse({ ...withInapplicable, checks: [withInapplicable.checks[1]] }).success).toBe(false);
    expect(verificationRecordSchema.safeParse({ ...verificationRecord, checks: [{ ...verificationRecord.checks[0], evidencePaths: [] }] }).success).toBe(false);
  });
  it('传播终止条件不能通过放宽阈值规避，误差值不能为负', () => {
    const draft = { baseVersionId: verificationRule.id, title: '修订', changeReason: '方案说明', thresholds: [{ metricId: 'propagation_completion', threshold: 1, basis: '研究方案' }] };
    expect(verificationDraftSchema.safeParse(draft).success).toBe(false);
    expect(verificationRuleSchema.safeParse({ ...verificationRule, checks: [{ ...verificationRule.checks[0], metricId: 'propagation_completion', threshold: 1 }] }).success).toBe(false);
    expect(verificationRecordSchema.safeParse({ ...verificationRecord, checks: [{ ...verificationRecord.checks[0], actual: -1 }] }).success).toBe(false);
  });
  it('历史依据逐字保留，允许多段依据但拒绝控制符和过长输入', () => {
    const basis = '  研究依据第一段\n第二段  ';
    const historical = { ...verificationRule, checks: [{ ...verificationRule.checks[0], basis }] };
    expect(verificationRuleSchema.parse(historical).checks[0].basis).toBe(basis);
    const draft = { baseVersionId: verificationRule.id, title: '新规则', changeReason: '方案更新\n保留条件', thresholds: [{ metricId: 'energy_error', threshold: 0, basis }] };
    expect(verificationDraftSchema.safeParse(draft).success).toBe(true);
    expect(verificationDraftSchema.safeParse({ ...draft, title: '标题\n第二行' }).success).toBe(false);
    expect(verificationDraftSchema.safeParse({ ...draft, changeReason: '理由\0损坏' }).success).toBe(false);
    expect(verificationDraftSchema.safeParse({ ...draft, changeReason: '中'.repeat(4096) }).success).toBe(false);
  });
});
