import { z } from 'zod';
import { errorSchema } from './contracts';

// B22 使用独立版本和来源；运行时引擎检查保持原始内容，不在这里改写。
export const VERIFICATION_TEXT_MAX_BYTES = 4096;
export const PROPAGATION_COMPLETION_THRESHOLD = 0;
// 与核心的 UTF-8 字节上限和控制字符规则保持一致，校验不能改写冻结文本。
const utf8 = new TextEncoder();
const preservedText = z.string().refine((value) => value.trim().length > 0, '内容不能为空。')
  .refine((value) => utf8.encode(value).length <= VERIFICATION_TEXT_MAX_BYTES, '内容不能超过 4096 个 UTF-8 字节。');
export const verificationTextSchema = preservedText.refine((value) => !/[\p{Cc}]/u.test(value), '单行内容不能包含控制字符。');
export const verificationParagraphSchema = preservedText.refine((value) => !/[\p{Cc}]/u.test(value.replace(/[\n\r\t]/gu, '')), '段落只能包含换行、回车和制表控制字符。');
const text = verificationTextSchema;
const hash = z.string().regex(/^[a-f\d]{64}$/iu);
const finite = z.number().finite();
export const VERIFICATION_PAGE_SIZE = 20;
export const VERIFICATION_PAGE_LIMIT = 100;
export const metricSchema = z.enum([
  'energy_error', 'angular_momentum_error', 'null_error', 'equatorial_error', 'turning_radius_error',
  'radial_analytic_error', 'azimuth_reference_error', 'throat_time_error', 'exit_time_error',
  'critical_relation_error', 'critical_direction_error', 'critical_position_error', 'reference_quadrature_error', 'propagation_completion',
]);
export const conclusionSchema = z.enum(['not_run', 'passed', 'failed', 'missing_artifact', 'not_applicable', 'inconclusive']);
const validThreshold = (check: { metricId: string; threshold: number }) => check.metricId !== 'propagation_completion' || check.threshold === PROPAGATION_COMPLETION_THRESHOLD;
export const verificationRuleCheckSchema = z.strictObject({
  metricId: metricSchema, title: text, threshold: finite.nonnegative(), unit: text, basis: verificationParagraphSchema, applicability: text, evidenceScope: text,
}).refine(validThreshold, '传播终止条件阈值必须为零。');
export const verificationRuleSchema = z.strictObject({
  id: text, projectId: text, ruleFamilyId: text, parentVersionId: text.nullable(), title: text, createdAt: text,
  createdBy: text, methodId: text, methodVersion: z.number().int().positive(), changeReason: verificationParagraphSchema, builtin: z.boolean(),
  checks: z.array(verificationRuleCheckSchema).min(1), contentHash: hash,
}).refine((rule) => new Set(rule.checks.map((check) => check.metricId)).size === rule.checks.length, '规则指标不得重复。');
export const verificationDraftSchema = z.strictObject({
  baseVersionId: text, title: text, changeReason: verificationParagraphSchema,
  thresholds: z.array(z.strictObject({ metricId: metricSchema, threshold: finite.nonnegative(), basis: verificationParagraphSchema })
    .refine(validThreshold, '传播终止条件阈值必须为零。')).min(1),
}).refine((draft) => new Set(draft.thresholds.map((check) => check.metricId)).size === draft.thresholds.length, '规则指标不得重复。');
export const verificationSourceSchema = z.strictObject({
  requestHash: hash, environmentHash: hash, resultHash: hash.nullable(), hashFormat: text,
  resultOrigin: z.enum(['captured_at_completion', 'observed_at_migration', 'no_result']),
}).refine((source) => (source.resultHash === null) === (source.resultOrigin === 'no_result'), '产物身份与来源必须一致。');
export const verificationCheckSchema = z.strictObject({
  metricId: metricSchema, trajectoryIndex: z.number().int().nonnegative().nullable(), impactParameter: finite.nullable(),
  title: text, actual: finite.nonnegative().nullable(), threshold: finite.nonnegative(), unit: text, basis: verificationParagraphSchema,
  conclusion: conclusionSchema, reasonCode: text, message: text, evidenceScope: text, evidencePaths: z.array(text),
}).refine(validThreshold, '传播终止条件阈值必须为零。')
  .refine((check) => check.conclusion !== 'passed' || (check.actual !== null && check.actual <= check.threshold && check.evidencePaths.length > 0),
  '通过必须有实际值、完整依据路径且不超过阈值。');
export const verificationRecordSchema = z.strictObject({
  id: text, projectId: text, requestId: text, clientRequestId: text, runId: text, ruleVersionId: text,
  previousRecordId: text.nullable(), startedAt: text, finishedAt: text, executedBy: text, methodId: text, methodVersion: z.number().int().positive(),
  executionStatus: z.enum(['completed', 'failed', 'interrupted']), conclusion: conclusionSchema,
  source: verificationSourceSchema, checks: z.array(verificationCheckSchema), error: errorSchema.nullable(), contentHash: hash,
}).refine((record) => record.conclusion !== 'passed' || (record.executionStatus === 'completed' && record.error === null
  && record.source.resultHash !== null && record.checks.some((check) => check.conclusion === 'passed')
  && record.checks.every((check) => check.conclusion === 'passed' || check.conclusion === 'not_applicable')),
  '独立验证通过必须由有产物的已完成检查支持。');
export const verificationStateSchema = z.strictObject({
  runId: text, ruleVersionId: text, conclusion: conclusionSchema, latestRecord: verificationRecordSchema.nullable(),
  recordCount: z.number().int().nonnegative(), pendingRequestId: text.nullable(),
}).refine((state) => !state.latestRecord || (state.latestRecord.runId === state.runId && state.latestRecord.ruleVersionId === state.ruleVersionId),
  '最近记录必须属于所选运行和规则。')
  .refine((state) => state.conclusion === (state.pendingRequestId ? 'inconclusive' : state.latestRecord?.conclusion ?? 'not_run'), '状态必须对应实际历史或未完成请求。')
  .refine((state) => (state.latestRecord === null) === (state.recordCount === 0), '记录数量与最近记录必须一致。');
export const verificationRulePageSchema = z.strictObject({ rules: z.array(verificationRuleSchema).max(VERIFICATION_PAGE_LIMIT), total: z.number().int().nonnegative(), nextOffset: z.number().int().nonnegative().nullable() });
export const verificationRecordPageSchema = z.strictObject({ records: z.array(verificationRecordSchema).max(VERIFICATION_PAGE_LIMIT), total: z.number().int().nonnegative(), nextOffset: z.number().int().nonnegative().nullable() });
export const verificationExecutionSchema = z.strictObject({ runId: text, ruleVersionId: text, clientRequestId: text, previousRecordId: text.nullable() });
export const migrationPlanSchema = z.strictObject({
  id: text, directory: text, projectId: text, projectName: text, fromVersion: z.literal(1), toVersion: z.literal(2),
  createdAt: text, sourceFingerprint: hash, backupPath: text, changes: z.array(text), warnings: z.array(text), requiresConfirmation: z.literal(true),
});
export const migrationReceiptSchema = z.strictObject({
  planId: text, directory: text, projectId: text, fromVersion: z.literal(1), toVersion: z.literal(2), backupPath: text,
  backupSha256: hash, migratedAt: text, legacyResultCount: z.number().int().nonnegative(),
});
export type VerificationRuleVersion = z.infer<typeof verificationRuleSchema>;
export type VerificationRuleDraft = z.infer<typeof verificationDraftSchema>;
export type VerificationRecord = z.infer<typeof verificationRecordSchema>;
export type RunVerificationState = z.infer<typeof verificationStateSchema>;
export type VerificationRulePage = z.infer<typeof verificationRulePageSchema>;
export type VerificationRecordPage = z.infer<typeof verificationRecordPageSchema>;
export type VerificationExecution = z.infer<typeof verificationExecutionSchema>;
export type ProjectMigrationPlan = z.infer<typeof migrationPlanSchema>;
export type ProjectMigrationReceipt = z.infer<typeof migrationReceiptSchema>;
export const CONCLUSION_LABELS: Record<z.infer<typeof conclusionSchema>, string> = {
  not_run: '尚未执行独立验证', passed: '独立验证通过', failed: '独立验证未通过', missing_artifact: '缺少产物', not_applicable: '不适用', inconclusive: '证据不足',
};
