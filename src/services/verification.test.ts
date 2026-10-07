import { describe, expect, it, vi } from 'vitest';
import { createDesktopClient } from './desktop';
import { migrationPlan, verificationRecord, verificationRule, verificationState } from '../test/verificationFixtures';

// 校验列表中每条对象与请求来源，不能只检查最外层结构。
describe('独立验证与迁移 IPC', () => {
  it('按冻结接口传递规则、运行、幂等请求与分页', async () => {
    const transport = vi.fn(); const client = createDesktopClient(transport, () => true);
    transport.mockResolvedValueOnce({ rules: [verificationRule], total: 1, nextOffset: null });
    await client.listVerificationRules(verificationRule.projectId, 0, 20);
    expect(transport).toHaveBeenLastCalledWith('list_verification_rules', { request: { projectId: verificationRule.projectId, offset: 0, limit: 20 } });
    transport.mockResolvedValueOnce(verificationState);
    await client.getRunVerificationState(verificationRule.projectId, verificationRecord.runId, verificationRule.id);
    transport.mockResolvedValueOnce(verificationRecord);
    const request = { runId: verificationRecord.runId, ruleVersionId: verificationRule.id, clientRequestId: verificationRecord.clientRequestId, previousRecordId: null };
    await client.executeVerification(verificationRule.projectId, request);
    expect(transport).toHaveBeenLastCalledWith('execute_verification', { request: { projectId: verificationRule.projectId, request } });
    transport.mockResolvedValueOnce({ records: [verificationRecord], total: 1, nextOffset: null });
    await client.listVerificationRecords(verificationRule.projectId, verificationRecord.runId, 0, 20);
    transport.mockResolvedValueOnce(verificationRecord);
    await client.getVerificationRecord(verificationRule.projectId, verificationRecord.id);
  });
  it('拒绝跨项目列表、跨运行历史、规则错配和幂等身份错配', async () => {
    const transport = vi.fn(); const client = createDesktopClient(transport, () => true);
    transport.mockResolvedValueOnce({ rules: [verificationRule], total: 1, nextOffset: null });
    await expect(client.listVerificationRules('other', 0, 20)).rejects.toMatchObject({ code: 'invalid_response' });
    transport.mockResolvedValueOnce({ records: [verificationRecord], total: 1, nextOffset: null });
    await expect(client.listVerificationRecords(verificationRule.projectId, 'other', 0, 20)).rejects.toMatchObject({ code: 'invalid_response' });
    transport.mockResolvedValueOnce(verificationState);
    await expect(client.getRunVerificationState(verificationRule.projectId, verificationRecord.runId, 'other')).rejects.toMatchObject({ code: 'invalid_response' });
    transport.mockResolvedValueOnce(verificationRecord);
    await expect(client.executeVerification(verificationRule.projectId, { runId: verificationRecord.runId, ruleVersionId: verificationRule.id, clientRequestId: 'other', previousRecordId: null })).rejects.toMatchObject({ code: 'invalid_response' });
    transport.mockResolvedValueOnce(verificationRecord);
    await expect(client.getVerificationRecord(verificationRule.projectId, 'other')).rejects.toMatchObject({ code: 'invalid_response' });
  });
  it('无效草稿和分页在 IPC 前拒绝，迁移确认回复必须匹配计划', async () => {
    const transport = vi.fn(); const client = createDesktopClient(transport, () => true);
    await expect(client.listVerificationRules('p', -1, 20)).rejects.toBeDefined();
    await expect(client.listVerificationRecords('p', 'r', 0, 101)).rejects.toBeDefined();
    await expect(client.saveVerificationRuleVersion('p', { baseVersionId: 'r', title: '规则', changeReason: '', thresholds: [] })).rejects.toBeDefined();
    expect(transport).not.toHaveBeenCalled();
    transport.mockResolvedValueOnce(migrationPlan);
    expect(await client.prepareProjectMigration(migrationPlan.directory)).toEqual(migrationPlan);
    transport.mockResolvedValueOnce({ planId: 'other', directory: migrationPlan.directory, projectId: migrationPlan.projectId, fromVersion: 1, toVersion: 2, backupPath: migrationPlan.backupPath, backupSha256: 'a'.repeat(64), migratedAt: migrationPlan.createdAt, legacyResultCount: 1 });
    await expect(client.applyProjectMigration(migrationPlan.directory, migrationPlan.id)).rejects.toMatchObject({ code: 'invalid_response' });
  });
  it('规则派生回复必须完整匹配父版本、阈值、依据和变更理由', async () => {
    const transport = vi.fn(); const client = createDesktopClient(transport, () => true);
    const draft = { baseVersionId: verificationRule.id, title: '新规则', changeReason: '根据新方案', thresholds: verificationRule.checks.map(({ metricId, threshold, basis }) => ({ metricId, threshold, basis })) };
    const newRule = { ...verificationRule, id: 'rule-2', parentVersionId: verificationRule.id, builtin: false, title: draft.title, changeReason: draft.changeReason };
    transport.mockResolvedValueOnce(newRule);
    expect(await client.saveVerificationRuleVersion(verificationRule.projectId, draft)).toEqual(newRule);
    expect(transport).toHaveBeenLastCalledWith('save_verification_rule_version', { request: { projectId: verificationRule.projectId, draft } });
    for (const changed of [{ ...newRule, parentVersionId: null }, { ...newRule, builtin: true }, { ...newRule, title: '其他标题' }, { ...newRule, checks: [{ ...newRule.checks[0], threshold: 0.5 }] }]) {
      transport.mockResolvedValueOnce(changed);
      await expect(client.saveVerificationRuleVersion(verificationRule.projectId, draft)).rejects.toMatchObject({ code: 'invalid_response' });
    }
  });
  it('分页游标不能回退或超过页大小，最近历史必须属于当前项目', async () => {
    const transport = vi.fn(); const client = createDesktopClient(transport, () => true);
    transport.mockResolvedValueOnce({ rules: [verificationRule], total: 2, nextOffset: 0 });
    await expect(client.listVerificationRules(verificationRule.projectId, 0, 20)).rejects.toMatchObject({ code: 'invalid_response' });
    transport.mockResolvedValueOnce({ rules: [verificationRule], total: 2, nextOffset: 1 });
    expect((await client.listVerificationRules(verificationRule.projectId, 0, 1)).nextOffset).toBe(1);
    transport.mockResolvedValueOnce({ records: [verificationRecord, { ...verificationRecord, id: 'second' }], total: 2, nextOffset: null });
    await expect(client.listVerificationRecords(verificationRule.projectId, verificationRecord.runId, 0, 1)).rejects.toMatchObject({ code: 'invalid_response' });
    transport.mockResolvedValueOnce({ ...verificationState, latestRecord: { ...verificationRecord, projectId: 'other' }, recordCount: 1, conclusion: 'passed' });
    await expect(client.getRunVerificationState(verificationRule.projectId, verificationRecord.runId, verificationRule.id)).rejects.toMatchObject({ code: 'invalid_response' });
  });
  it('尚有历史时不能省略下一页游标，累计项数不能超过总数', async () => {
    const transport = vi.fn(); const client = createDesktopClient(transport, () => true);
    transport.mockResolvedValueOnce({ rules: [verificationRule], total: 2, nextOffset: null });
    await expect(client.listVerificationRules(verificationRule.projectId, 0, 20)).rejects.toMatchObject({ code: 'invalid_response' });
    transport.mockResolvedValueOnce({ records: [verificationRecord], total: 1, nextOffset: null });
    await expect(client.listVerificationRecords(verificationRule.projectId, verificationRecord.runId, 1, 20)).rejects.toMatchObject({ code: 'invalid_response' });
  });
  it('派生版本逐字核对依据与理由，不接受被裁剪的冻结内容', async () => {
    const transport = vi.fn(); const client = createDesktopClient(transport, () => true);
    const draft = { baseVersionId: verificationRule.id, title: '  新规则  ', changeReason: '  第一段\n第二段  ', thresholds: verificationRule.checks.map(({ metricId, threshold }) => ({ metricId, threshold, basis: '  研究方案\r\n\t原始依据  ' })) };
    const newRule = { ...verificationRule, id: 'rule-2', parentVersionId: verificationRule.id, builtin: false, title: draft.title, changeReason: draft.changeReason, checks: verificationRule.checks.map((check) => ({ ...check, basis: draft.thresholds[0].basis })) };
    transport.mockResolvedValueOnce(newRule);
    expect(await client.saveVerificationRuleVersion(verificationRule.projectId, draft)).toEqual(newRule);
    expect(transport).toHaveBeenLastCalledWith('save_verification_rule_version', { request: { projectId: verificationRule.projectId, draft } });
    transport.mockResolvedValueOnce({ ...newRule, checks: newRule.checks.map((check) => ({ ...check, basis: check.basis.trim() })) });
    await expect(client.saveVerificationRuleVersion(verificationRule.projectId, draft)).rejects.toMatchObject({ code: 'invalid_response' });
  });
  it('空页和最后一页不生成游标，有剩余项时空页不能使分页停滞', async () => {
    const transport = vi.fn(); const client = createDesktopClient(transport, () => true);
    transport.mockResolvedValueOnce({ rules: [], total: 0, nextOffset: null });
    expect((await client.listVerificationRules(verificationRule.projectId, 0, 20)).rules).toEqual([]);
    transport.mockResolvedValueOnce({ records: [verificationRecord], total: 2, nextOffset: null });
    expect((await client.listVerificationRecords(verificationRule.projectId, verificationRecord.runId, 1, 20)).total).toBe(2);
    transport.mockResolvedValueOnce({ rules: [], total: 2, nextOffset: 0 });
    await expect(client.listVerificationRules(verificationRule.projectId, 0, 20)).rejects.toMatchObject({ code: 'invalid_response' });
    transport.mockResolvedValueOnce({ rules: [verificationRule], total: 1, nextOffset: 1 });
    await expect(client.listVerificationRules(verificationRule.projectId, 0, 20)).rejects.toMatchObject({ code: 'invalid_response' });
  });
});
