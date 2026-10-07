import { describe, expect, it } from 'vitest';
import { rayColor, RAY_COLORS } from './palette';

// 光线颜色绑定真实参数身份，重新排序或新增光线不能改变原有参数的视觉含义。
describe('光线参数配色', () => {
  it('固定基准光线使用蓝、青、橙配色', () => {
    expect([rayColor(0), rayColor(0.5), rayColor(2)]).toEqual(RAY_COLORS.slice(0, 3));
    expect(rayColor(0)).toBe('#578CFF');
    expect(rayColor(0.5)).toBe('#3EDCFF');
    expect(rayColor(2)).toBe('#FFB65C');
  });
  it('任意参数按值稳定映射，排序及数值表示不影响配色', () => {
    const before = new Map([0, 0.5, 2, 3.25, -0.5].map((impact) => [impact, rayColor(impact)]));
    for (const impact of [3.25, 2, 0, -0.5, 0.5]) expect(rayColor(impact)).toBe(before.get(impact));
    expect(rayColor(-0)).toBe(rayColor(0));
    expect(rayColor(3.25)).toMatch(/^#[0-9A-F]{6}$/);
  });
});
