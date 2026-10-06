import { describe, expect, it } from 'vitest';
import { embeddingPoint, makeSurface, sampleAtAffine, residualSeries, timeBounds, advanceTimeline, displaySamples, DISPLAY_LIMITS } from './geometry';

// 解析几何与时间轴均使用显式参考值，测试夹具从不进入生产数据。
const samples = [
  { affine: 0, t: 0, l: 2, theta: Math.PI / 2, phi: 0, kt: 1, kl: -1, kTheta: 0, kPhi: 0 },
  { affine: 2, t: 2, l: 0, theta: Math.PI / 2, phi: 0, kt: 1, kl: -1, kTheta: 0, kPhi: 0 },
];
describe('嵌入与真实样本联动', () => {
  it('喉部与双侧嵌入满足解析曲面', () => {
    expect(embeddingPoint(0, 0, 2)).toEqual([0, 2, 0]);
    const positive = embeddingPoint(3, Math.PI / 2, 2);
    const negative = embeddingPoint(-3, Math.PI / 2, 2);
    expect(positive[0]).toBeCloseTo(2 * Math.asinh(1.5));
    expect(negative[0]).toBeCloseTo(-positive[0]);
    expect(positive[2]).toBeCloseTo(Math.sqrt(13));
  });
  it('曲面网格包含两侧且剖切仅限制角范围', () => {
    const full = makeSurface(1, 10, false);
    const cut = makeSurface(1, 10, true);
    expect(full.positions.length).toBe(cut.positions.length);
    expect(full.indices.length).toBeGreaterThan(0);
    expect(Math.min(...full.positions)).toBeLessThan(0);
    expect(full.positions.every(Number.isFinite)).toBe(true);
  });
  it('以真实采样插值并钳制到边界', () => {
    expect(sampleAtAffine(samples, 1)?.l).toBeCloseTo(1);
    expect(sampleAtAffine(samples, -1)).toEqual(samples[0]);
    expect(sampleAtAffine(samples, 4)).toEqual(samples[1]);
    expect(sampleAtAffine([], 1)).toBeNull();
  });
  it('从度规直接计算守恒残差，不注入非零伪值', () => {
    expect(residualSeries(samples, 1, 0).map((entry) => entry.nullError)).toEqual([0, 0]);
    const perturbed = { ...samples[0], kt: 2, kPhi: 1 };
    expect(residualSeries([perturbed], 1, 0)[0]).toEqual({ affine: 0, energyError: 1, angularMomentumError: 5, nullError: 2 });
  });
  it('空轨迹与完整播放终点有明确语义', () => {
    expect(timeBounds([])).toEqual([0, 0]);
    expect(timeBounds([samples])).toEqual([0, 2]);
    expect(advanceTimeline(1, 0.5, 2, 2)).toEqual({ affine: 2, playing: false });
    expect(advanceTimeline(0, 0.5, 1, 2)).toEqual({ affine: 0.5, playing: true });
  });
  it('显示抽样保持终点、遵守图形预算且不改动原数据', () => {
    const original = Array.from({ length: DISPLAY_LIMITS.maxLineSamples * 3 }, (_, index) => ({ ...samples[0], affine: index, l: index }));
    const displayed = displaySamples(original);
    expect(displayed.length).toBeLessThanOrEqual(DISPLAY_LIMITS.maxLineSamples);
    expect(displayed.at(-1)).toBe(original.at(-1));
    expect(original).toHaveLength(DISPLAY_LIMITS.maxLineSamples * 3);
    expect(displaySamples(original, 10.5).at(-1)?.affine).toBe(10.5);
    expect(displaySamples([])).toEqual([]);
  });
  it('二分搜索在多点轨迹的左右分支都取得相邻样本', () => {
    const many = [0, 1, 2, 3, 4].map((affine) => ({ ...samples[0], affine, l: 10 - affine }));
    expect(sampleAtAffine(many, 0.5)?.l).toBe(9.5);
    expect(sampleAtAffine(many, 3.5)?.l).toBe(6.5);
  });
  it('极端合法比值不会让嵌入轴溢出，显示网格归一化后保持有限', () => {
    expect(embeddingPoint(1e150, 0, 1e-200).every(Number.isFinite)).toBe(true);
    const mesh = makeSurface(1e150, 1e150, false, 1e150);
    expect(mesh.positions.every((value) => Number.isFinite(Math.fround(value)))).toBe(true);
    expect(Math.max(...mesh.positions.map(Math.abs))).toBeLessThanOrEqual(Math.SQRT2 + 1e-12);
  });
});
