import { DESIGN_COLORS } from '../design/system';

// 标准光线绑定参数身份；数组仍导出供既有颜色图例兼容使用。
export const RAY_COLORS = [DESIGN_COLORS.secondary, DESIGN_COLORS.primary, DESIGN_COLORS.warning, DESIGN_COLORS.success, DESIGN_COLORS.error];
const REFERENCE_COLORS = new Map([[0, RAY_COLORS[0]], [0.5, RAY_COLORS[1]], [2, RAY_COLORS[2]]]);
const HASH_BASE = 31;

// 非标准参数使用稳定哈希，颜色不能因插入或排序而漂移；界面同时显示 b 值避免仅凭颜色识别。
export function rayColor(impactParameter: number): string {
  const reference = REFERENCE_COLORS.get(impactParameter);
  if (reference) return reference;
  const key = String(impactParameter);
  let hash = 0;
  for (const character of key) hash = (Math.imul(hash, HASH_BASE) + character.charCodeAt(0)) >>> 0;
  return RAY_COLORS[hash % RAY_COLORS.length];
}
