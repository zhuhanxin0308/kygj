import { z } from 'zod';

// 数值与资源边界与 engine-v1 契约一致，前端仅提前反馈，宿主仍独立验证。
export const INPUT_LIMITS = { minSamples: 2, maxSamples: 100000, maxTotalSamples: 100000, maxRays: 256, minRelativeTolerance: 100 * Number.EPSILON } as const;
export const MAX_PROJECT_NAME_BYTES = 255;
export const configSchema = z.strictObject({
  throatRadius: z.number().finite().positive(), initialRadius: z.number().finite().positive(),
  impactParameters: z.array(z.number().finite()).min(1).max(INPUT_LIMITS.maxRays),
  maxAffineParameter: z.number().finite().positive(),
  sampleCount: z.number().int().min(INPUT_LIMITS.minSamples).max(INPUT_LIMITS.maxSamples),
  relativeTolerance: z.number().finite().min(INPUT_LIMITS.minRelativeTolerance).lt(1), absoluteTolerance: z.number().finite().positive(),
}).refine((config) => config.sampleCount * config.impactParameters.length <= INPUT_LIMITS.maxTotalSamples, {
  message: '每批光线的总采样点数不能超过资源限额。', path: ['sampleCount'],
}).refine((config) => {
  // 科学引擎使用平方形式的度规分量；hypot 有限不代表后续平方和仍有限。
  const radiusSquared = config.initialRadius * config.initialRadius + config.throatRadius * config.throatRadius;
  return Number.isFinite(radiusSquared) && radiusSquared > 0
    && config.impactParameters.every((impact) => Number.isFinite(impact * impact)
      && Number.isFinite(impact / radiusSquared) && Number.isFinite(1 - (impact * impact) / radiusSquared)
      && 1 - (impact * impact) / radiusSquared > 0);
}, {
  message: '初态度规与切向量运算超出有限浮点范围，请使用可计算的几何尺度。',
});
export type EllisConfig = z.infer<typeof configSchema>;
export const DEFAULT_CONFIG: EllisConfig = {
  throatRadius: 1, initialRadius: 10, impactParameters: [0, 0.5, 2],
  maxAffineParameter: 40, sampleCount: 1001, relativeTolerance: 1e-10, absoluteTolerance: 1e-12,
};

export function parseImpactParameters(input: string): number[] {
  const parts = input.replaceAll('，', ',').split(',');
  if (parts.some((part) => part.trim() === '' || !Number.isFinite(Number(part)))) {
    throw new Error('冲量参数须为用逗号分隔的有限数字，不能包含空项。');
  }
  return parts.map(Number);
}

export function validateProjectName(name: string): string | null {
  const stem = name.split('.')[0].trimEnd();
  if (!name || name.trim() !== name || new TextEncoder().encode(name).length > MAX_PROJECT_NAME_BYTES
    || /[<>:"/\\|?*\p{Cc}]/u.test(name) || /[. ]$/u.test(name)
    || /^(con|prn|aux|nul|com[1-9¹²³]|lpt[1-9¹²³])$/iu.test(stem) || name === '..') {
    return '项目名称须为合法单个目录名，不能包含路径、保留名称或首尾空格。';
  }
  return null;
}

// 固定字段序列保证比较不依赖对象键的插入顺序。
export function configsEqual(left: EllisConfig, right: EllisConfig): boolean {
  return left.throatRadius === right.throatRadius && left.initialRadius === right.initialRadius
    && left.maxAffineParameter === right.maxAffineParameter && left.sampleCount === right.sampleCount
    && left.relativeTolerance === right.relativeTolerance && left.absoluteTolerance === right.absoluteTolerance
    && left.impactParameters.length === right.impactParameters.length
    && left.impactParameters.every((value, index) => value === right.impactParameters[index]);
}
