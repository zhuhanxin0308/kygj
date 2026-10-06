import { describe, expect, it } from 'vitest';
import { projectStateSchema, runRecordSchema, engineResultSchema } from './contracts';
import { project, run, result } from '../test/fixtures';

// 运行结果进入界面前必须校验身份、冻结配置和有限采样。
describe('跨语言返回契约', () => {
  it('接受完整项目和真实结果结构', () => {
    expect(projectStateSchema.parse(project)).toEqual(project);
  });
  it('拒绝错误状态与损坏数值', () => {
    expect(runRecordSchema.safeParse({ ...run, state: 'success' }).success).toBe(false);
    const broken = structuredClone(result);
    broken.trajectories[0].samples[0].l = Infinity;
    expect(engineResultSchema.safeParse(broken).success).toBe(false);
  });
  it('拒绝关联标识不匹配和冻结配置漂移', () => {
    expect(runRecordSchema.safeParse({ ...run, result: { ...result, requestId: 'other' } }).success).toBe(false);
    expect(runRecordSchema.safeParse({ ...run, result: { ...result, config: { ...result.config, throatRadius: 2 } } }).success).toBe(false);
  });
  it('拒绝样本时序倒置与轨迹数量不符', () => {
    const reversed = structuredClone(result);
    reversed.trajectories[0].samples.reverse();
    expect(engineResultSchema.safeParse(reversed).success).toBe(false);
    expect(engineResultSchema.safeParse({ ...result, trajectories: [] }).success).toBe(false);
  });
  it.each(['budget_exhausted', 'solver_failed'] as const)('未完成轨迹 %s 不能宣称检查通过', (termination) => {
    const value = structuredClone(result);
    value.trajectories[0].termination = termination;
    expect(engineResultSchema.safeParse(value).success).toBe(false);
  });
  it('空检查和与实际阈值不一致的通过布尔值被拒绝', () => {
    const empty = structuredClone(result); empty.trajectories[0].validation.checks = [];
    expect(engineResultSchema.safeParse(empty).success).toBe(false);
    const dishonest = structuredClone(result); dishonest.trajectories[0].validation.checks[0].actual = 1;
    expect(engineResultSchema.safeParse(dishonest).success).toBe(false);
    const falseNegative = structuredClone(result); falseNegative.trajectories[0].validation.checks[0].passed = false;
    expect(engineResultSchema.safeParse(falseNegative).success).toBe(false);
  });
  it('预算耗尽保留失败检查和证据不足状态', () => {
    const value = structuredClone(result);
    value.trajectories[0].termination = 'budget_exhausted';
    value.trajectories[0].validation = { status: 'inconclusive', checks: [{ name: '能量', actual: 1, threshold: 1e-8, passed: false }] };
    expect(engineResultSchema.safeParse(value).success).toBe(true);
  });
  it('运行层不能覆盖轨迹聚合验证状态，无产物不能标记通过', () => {
    const inconclusive = structuredClone(run);
    inconclusive.result!.trajectories[0].termination = 'budget_exhausted';
    inconclusive.result!.trajectories[0].validation.status = 'inconclusive';
    expect(runRecordSchema.safeParse(inconclusive).success).toBe(false);
    expect(runRecordSchema.safeParse({ ...inconclusive, validationStatus: 'inconclusive' }).success).toBe(true);
    const failed = structuredClone(run);
    failed.result!.trajectories[0].validation = { status: 'failed', checks: [{ name: '能量', actual: 1, threshold: 1e-8, passed: false }] };
    expect(runRecordSchema.safeParse(failed).success).toBe(false);
    expect(runRecordSchema.safeParse({ ...failed, validationStatus: 'failed' }).success).toBe(true);
    expect(runRecordSchema.safeParse({ ...run, result: null }).success).toBe(false);
    expect(runRecordSchema.safeParse({ ...run, state: 'running', result: null, validationStatus: 'not_run' }).success).toBe(true);
  });
});
