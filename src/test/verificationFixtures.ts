import type { ProjectMigrationPlan, VerificationRecord, VerificationRuleVersion, RunVerificationState } from '../domain/verification';
import { project, run } from './fixtures';

// 只用于测试协议与交互，生产数据必须来自宿主持久化结果。
export const verificationRule: VerificationRuleVersion = {
  id: 'rule-1', projectId: project.project.id, ruleFamilyId: 'family-1', parentVersionId: null,
  title: 'Ellis SciPy诊断重评', createdAt: run.createdAt, createdBy: '内置规则', methodId: 'ellis_diagnostics',
  methodVersion: 1, changeReason: '预先冻结的检查', builtin: true, contentHash: 'd'.repeat(64),
  checks: [{ metricId: 'energy_error', title: '能量误差', threshold: 1e-8, unit: '无量纲', basis: '研究方案完整依据', applicability: '已保存诊断', evidenceScope: '引擎诊断重评' }],
};
export const verificationRecord: VerificationRecord = {
  id: 'verification-1', projectId: project.project.id, requestId: 'verification-request-1', clientRequestId: 'client-1',
  runId: run.id, ruleVersionId: verificationRule.id, previousRecordId: null,
  startedAt: run.createdAt, finishedAt: run.finishedAt!, executedBy: '本地研究者', methodId: verificationRule.methodId,
  methodVersion: verificationRule.methodVersion, executionStatus: 'completed', conclusion: 'passed',
  source: { requestHash: 'a'.repeat(64), environmentHash: 'b'.repeat(64), resultHash: 'c'.repeat(64), hashFormat: 'sha256-json-v1', resultOrigin: 'captured_at_completion' },
  checks: [{ metricId: 'energy_error', trajectoryIndex: 0, impactParameter: 0, title: '能量误差', actual: 0,
    threshold: 1e-8, unit: '无量纲', basis: '研究方案完整依据', conclusion: 'passed', reasonCode: 'within_threshold',
    message: '实际误差在阈值内', evidenceScope: '引擎诊断重评', evidencePaths: ['trajectories[0].diagnostics.maxEnergyError'] }],
  error: null, contentHash: 'e'.repeat(64),
};
export const verificationState: RunVerificationState = {
  runId: run.id, ruleVersionId: verificationRule.id, conclusion: 'not_run', latestRecord: null, recordCount: 0, pendingRequestId: null,
};
export const migrationPlan: ProjectMigrationPlan = {
  id: 'migration-1', directory: project.project.path, projectId: project.project.id, projectName: project.project.name,
  fromVersion: 1, toVersion: 2, createdAt: run.createdAt, sourceFingerprint: 'a'.repeat(64), backupPath: 'C:/Research/backup.sqlite',
  changes: ['添加独立验证记录'], warnings: ['旧产物哈希仅表示迁移时所见内容。'], requiresConfirmation: true,
};
