import { describe, expect, it } from 'vitest';
import { DEFAULT_CONFIG, configSchema, parseImpactParameters, validateProjectName } from './model';

// 配置测试锁定科学输入边界，不能通过表单吞掉错误值。
describe('Ellis 科学输入', () => {
  it('允许完整默认值与负冲量参数', () => {
    expect(configSchema.parse(DEFAULT_CONFIG)).toEqual(DEFAULT_CONFIG);
    expect(parseImpactParameters('0, -0.5，2')).toEqual([0, -0.5, 2]);
  });
  it.each([
    { throatRadius: 0 }, { throatRadius: Infinity }, { initialRadius: -1 },
    { impactParameters: [] }, { impactParameters: [NaN] }, { impactParameters: [11] },
    { sampleCount: 1 }, { sampleCount: 100001 }, { sampleCount: 2.5 },
    { maxAffineParameter: 0 }, { relativeTolerance: 0 }, { absoluteTolerance: -1 },
    { impactParameters: Array(257).fill(0) }, { unexpected: true },
    { relativeTolerance: 1 }, { relativeTolerance: Number.EPSILON }, { sampleCount: 100000 },
  ])('拒绝不合法参数 %j', (patch) => {
    expect(configSchema.safeParse({ ...DEFAULT_CONFIG, ...patch }).success).toBe(false);
  });
  it.each(['', '0,,1', '0,abc', 'Infinity', '0, '])('不允许空项与非数字 %s', (input) => {
    expect(() => parseImpactParameters(input)).toThrow();
  });
  it.each(['../项目', 'CON', 'LPT1.txt', 'a/b', 'a\\b', '项目.', ' 项目', ''])('阻止危险项目目录名 %s', (name) => {
    expect(validateProjectName(name)).not.toBeNull();
  });
  it('保留合法中文目录名', () => expect(validateProjectName('Ellis 光传播研究')).toBeNull());
  it('临界初态严格使用契约的平方除法顺序，不新增平方根舍入门槛', () => {
    expect(configSchema.safeParse({ ...DEFAULT_CONFIG, throatRadius: 0.1, initialRadius: 1, impactParameters: [1.004987562112089] }).success).toBe(true);
  });
  it.each(['COM¹', 'LPT³.txt', 'CON .txt', '中'.repeat(86), '控制\u007f字符'])('目录名与宿主UTF-8和Windows规范一致 %s', (name) => {
    expect(validateProjectName(name)).not.toBeNull();
  });
});
