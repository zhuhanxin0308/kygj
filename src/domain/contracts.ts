import { z } from 'zod';
import { configSchema, configsEqual, INPUT_LIMITS } from './model.ts';

// 接收端拒绝非有限数、错误身份和结构漂移，避免损坏产物被绘成可信结果。
const finite = z.number().finite();
const identifier = z.string().min(1);
const hash = z.string().regex(/^[a-f\d]{64}$/iu);
const timestamp = z.string().min(1);
export const errorSchema = z.strictObject({ code: identifier, message: identifier });
export const environmentSchema = z.strictObject({
  pythonExecutable: identifier, engineVersion: identifier, pythonVersion: identifier,
  numpyVersion: identifier, scipyVersion: identifier, engineSourceHash: hash,
});
export const sampleSchema = z.strictObject({
  affine: finite.nonnegative(), t: finite, l: finite, theta: finite, phi: finite,
  kt: finite, kl: finite, kTheta: finite, kPhi: finite,
});
export const trajectorySchema = z.strictObject({
  impactParameter: finite,
  termination: z.enum(['through', 'returned', 'budget_exhausted', 'solver_failed']),
  samples: z.array(sampleSchema).min(1).max(INPUT_LIMITS.maxSamples),
  events: z.array(z.strictObject({
    kind: z.enum(['throat', 'turning', 'exit_negative', 'return_positive']), affine: finite.nonnegative(), radius: finite,
  })),
  diagnostics: z.strictObject({
    maxEnergyError: finite.nonnegative(), maxAngularMomentumError: finite.nonnegative(),
    maxNullError: finite.nonnegative(), maxEquatorialError: finite.nonnegative(),
    turningRadiusError: finite.nonnegative().nullable(), radialAnalyticError: finite.nonnegative().nullable(),
    azimuthReferenceError: finite.nonnegative().nullable(),
  }),
  validation: z.strictObject({
    status: z.enum(['passed', 'failed', 'inconclusive']),
    checks: z.array(z.strictObject({ name: identifier, actual: finite, threshold: finite, passed: z.boolean() })
      .refine((check) => check.passed === (check.actual <= check.threshold), { message: '检查布尔值必须与实际值和阈值一致。' })),
  }).refine((validation) => validation.status !== 'passed'
    || (validation.checks.length > 0 && validation.checks.every((check) => check.passed)), {
    message: '检查通过必须具有非空且全部通过的检查记录。',
  }),
}).refine((trajectory) => trajectory.samples.every((sample, index, samples) => index === 0 || sample.affine > samples[index - 1].affine), {
  message: '轨迹仿射参数必须严格递增。',
}).refine((trajectory) => trajectory.validation.status !== 'passed'
  || (trajectory.termination === 'through' || trajectory.termination === 'returned'), {
  message: '预算耗尽或求解失败的轨迹不能宣称验证通过。',
});
export const engineResultSchema = z.strictObject({
  protocolVersion: z.literal(1), requestId: identifier, type: z.literal('result'), config: configSchema,
  trajectories: z.array(trajectorySchema).min(1).max(INPUT_LIMITS.maxRays),
  environment: z.strictObject({ pythonVersion: identifier, numpyVersion: identifier, scipyVersion: identifier }),
}).refine((result) => result.trajectories.length === result.config.impactParameters.length
  && result.trajectories.every((trajectory, index) => trajectory.impactParameter === result.config.impactParameters[index]), {
  message: '轨迹数量和冲量参数必须与冻结配置相同。',
});
export const modelVersionSchema = z.strictObject({
  id: identifier, projectId: identifier, label: identifier, createdAt: timestamp, config: configSchema, contentHash: hash,
});
export const runRecordSchema = z.strictObject({
  id: identifier, projectId: identifier, modelVersionId: identifier,
  createdAt: timestamp, startedAt: timestamp.nullable(), finishedAt: timestamp.nullable(),
  state: z.enum(['queued', 'running', 'completed', 'failed', 'cancelling', 'cancelled', 'unknown']),
  validationStatus: z.enum(['not_run', 'passed', 'failed', 'inconclusive']),
  request: z.strictObject({ protocolVersion: z.literal(1), requestId: identifier, action: z.literal('traceEllis'), config: configSchema }),
  environment: environmentSchema, result: engineResultSchema.nullable(), error: errorSchema.nullable(),
}).refine((record) => !record.result || (record.result.requestId === record.request.requestId
  && configsEqual(record.result.config, record.request.config)), { message: '产物必须匹配冻结请求的身份与配置。' })
  .refine((record) => {
    // 顶层状态只能忠实汇总实际轨迹，不得把未判定或失败轨迹变为绿色通过。
    if (!record.result) return record.validationStatus === 'not_run';
    const statuses = record.result.trajectories.map((trajectory) => trajectory.validation.status);
    const aggregate = statuses.includes('failed') ? 'failed' : statuses.includes('inconclusive') ? 'inconclusive' : 'passed';
    return record.validationStatus === aggregate;
  }, { message: '运行验证状态必须与实际轨迹的聚合状态一致。' });
export const projectStateSchema = z.strictObject({
  project: z.strictObject({ id: identifier, name: identifier, path: identifier, createdAt: timestamp, schemaVersion: z.literal(1) }),
  models: z.array(modelVersionSchema), runs: z.array(runRecordSchema),
}).refine((state) => state.models.every((model) => model.projectId === state.project.id)
  && state.runs.every((run) => run.projectId === state.project.id), { message: '对象必须属于当前项目。' });
export const preflightSchema = z.strictObject({
  id: identifier, projectId: identifier, modelVersionId: identifier, config: configSchema,
  environment: environmentSchema, createdAt: timestamp, status: z.enum(['ready', 'blocked']), issues: z.array(errorSchema),
  executionLimits: z.strictObject({ maxWallTimeSeconds: finite.positive(), maxOutputBytes: finite.positive(), maxTotalSamples: finite.positive() }),
});
export const exportSchema = z.strictObject({ path: identifier, sha256: hash });
export type Sample = z.infer<typeof sampleSchema>;
export type Trajectory = z.infer<typeof trajectorySchema>;
export type EnvironmentInfo = z.infer<typeof environmentSchema>;
export type EngineResult = z.infer<typeof engineResultSchema>;
export type ModelVersion = z.infer<typeof modelVersionSchema>;
export type RunRecord = z.infer<typeof runRecordSchema>;
export type ProjectState = z.infer<typeof projectStateSchema>;
export type PreflightReport = z.infer<typeof preflightSchema>;
export type ExportRecord = z.infer<typeof exportSchema>;
