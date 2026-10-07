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
  it.each([false, true])('稀疏线框只沿相邻经纬边且无重复或三角对角线（剖切：%s）', (cut) => {
    const mesh = makeSurface(1, 10, cut);
    expect(mesh.wireIndices).toBeDefined();
    const stride = DISPLAY_LIMITS.angularSegments + 1;
    const edges = new Set<string>();
    const latitudeRows = new Set<number>();
    const longitudeColumns = new Set<number>();
    for (let index = 0; index < mesh.wireIndices.length; index += 2) {
      const start = mesh.wireIndices[index];
      const end = mesh.wireIndices[index + 1];
      const startRow = Math.floor(start / stride); const endRow = Math.floor(end / stride);
      const startColumn = start % stride; const endColumn = end % stride;
      expect(start).toBeGreaterThanOrEqual(0);
      expect(end).toBeLessThan(mesh.positions.length / 3);
      const alongLatitude = startRow === endRow && endColumn - startColumn === 1;
      const alongLongitude = startColumn === endColumn && endRow - startRow === 1;
      expect(alongLatitude || alongLongitude).toBe(true);
      if (alongLatitude) latitudeRows.add(startRow);
      if (alongLongitude) longitudeColumns.add(startColumn);
      const key = `${start}:${end}`;
      expect(edges.has(key)).toBe(false); edges.add(key);
    }
    // 线框降低线条密度，但三角曲面的分辨率保持不变。
    expect(latitudeRows.size).toBeLessThan(DISPLAY_LIMITS.radialSegments + 1);
    expect(longitudeColumns.size).toBeLessThan(DISPLAY_LIMITS.angularSegments + 1);
    expect(mesh.indices).toHaveLength(DISPLAY_LIMITS.radialSegments * DISPLAY_LIMITS.angularSegments * 6);
  });
  it.each([false, true])('稀疏线框覆盖双侧口沿、喉部与完整剖切边界（剖切：%s）', (cut) => {
    const mesh = makeSurface(1, 10, cut);
    expect(mesh.wireIndices).toBeDefined();
    const { radialSegments, angularSegments } = DISPLAY_LIMITS;
    const stride = angularSegments + 1;
    const edges = new Set(Array.from({ length: mesh.wireIndices.length / 2 }, (_, index) => `${mesh.wireIndices[index * 2]}:${mesh.wireIndices[index * 2 + 1]}`));
    for (const row of [0, radialSegments / 2, radialSegments]) {
      for (let column = 0; column < angularSegments; column += 1) {
        expect(edges.has(`${row * stride + column}:${row * stride + column + 1}`)).toBe(true);
      }
    }
    for (let row = 0; row < radialSegments; row += 1) {
      expect(edges.has(`${row * stride}:${(row + 1) * stride}`)).toBe(true);
      // 完整曲面的0与2π缝线重合，仅画一次；剖切曲面的两条开口边均保留。
      expect(edges.has(`${row * stride + angularSegments}:${(row + 1) * stride + angularSegments}`)).toBe(cut);
    }
    const mouth = mesh.positions.slice(0, 3);
    const seam = mesh.positions.slice(angularSegments * 3, angularSegments * 3 + 3);
    expect(seam[0]).toBeCloseTo(mouth[0]);
    expect(seam[1]).toBeCloseTo(cut ? -mouth[1] : mouth[1]);
    expect(seam[2]).toBeCloseTo(0);
  });
  it('线框引用同一真实嵌入顶点，归一化不改变曲面比例和拓扑', () => {
    const physical = makeSurface(2, 10, false);
    const normalized = makeSurface(2, 10, false, 10);
    expect(physical.wireIndices).toBeDefined();
    expect(normalized.wireIndices).toEqual(physical.wireIndices);
    expect(normalized.indices).toEqual(physical.indices);
    expect(normalized.positions).toEqual(physical.positions.map((coordinate) => coordinate / 10));
    expect(physical.positions).toHaveLength((DISPLAY_LIMITS.radialSegments + 1) * (DISPLAY_LIMITS.angularSegments + 1) * 3);
    const throatVertex = DISPLAY_LIMITS.radialSegments / 2 * (DISPLAY_LIMITS.angularSegments + 1);
    expect(physical.positions.slice(throatVertex * 3, throatVertex * 3 + 3)).toEqual([0, 2, 0]);
    expect(physical.positions[0]).toBeCloseTo(-2 * Math.asinh(5));
    expect(physical.positions[1]).toBeCloseTo(Math.sqrt(104));
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
