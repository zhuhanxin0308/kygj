import { DESIGN_COLORS } from './system';

// 全局导航沿用v3的分面三角标记；矢量绘制保持不同缩放下的清晰度。
export function GravityMark() {
  return <svg className="brand-mark" viewBox="0 0 36 36" aria-hidden="true" focusable="false">
    <path d="M18 3 2 31h11l5-9-5-8Z" fill={DESIGN_COLORS.primary} />
    <path d="m18 3 6 10-6 9-5-8Z" fill={DESIGN_COLORS.secondary} />
    <path d="m24 13 10 18H13l5-9 5 9h11Z" fill={DESIGN_COLORS.secondary} opacity=".7" />
  </svg>;
}
