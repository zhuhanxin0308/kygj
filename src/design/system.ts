import type { CSSProperties } from 'react';
import { theme, type ThemeConfig } from 'antd';

// v3唯一视觉基准：outputs/界面设计稿-科技版/全局设计规范.json。
// 科研数值、验证状态和嵌入几何仍来自真实数据，设计图的示例值不属于主题。
export const DESIGN_COLORS = {
  background: '#090E17', surface: '#111A29', raised: '#17243A', primary: '#3EDCFF', secondary: '#578CFF',
  warning: '#FFB65C', error: '#FF6B7A', success: '#36D6B0', text: '#EDF4FF', muted: '#A6B7CF', border: '#263A51',
} as const;
export const DESIGN_MOTION = { hover: 120, selection: 160, drawer: 220, workspace: 240, camera: 360 } as const;
export const DESIGN_LAYOUT = { navigation: 64, topbar: 48, resource: 240, inspector: 320, gap: 8, radius: 6, status: 40 } as const;
export const DESIGN_TYPE = {
  body: 'Inter, "Segoe UI", "Microsoft YaHei", sans-serif',
  numeric: '"Cascadia Code", Consolas, monospace',
  equation: 'Cambria, "Times New Roman", serif',
} as const;

// Ant Design与自绘科学场景共享语义色；组件仍使用各自可访问性实现。
export const WORKBENCH_THEME: ThemeConfig = {
  algorithm: theme.darkAlgorithm,
  token: {
    colorPrimary: DESIGN_COLORS.primary, colorInfo: DESIGN_COLORS.primary,
    colorSuccess: DESIGN_COLORS.success, colorWarning: DESIGN_COLORS.warning, colorError: DESIGN_COLORS.error,
    colorBgBase: DESIGN_COLORS.background, colorBgContainer: DESIGN_COLORS.surface, colorBgElevated: DESIGN_COLORS.raised,
    colorText: DESIGN_COLORS.text, colorTextSecondary: DESIGN_COLORS.muted, colorBorder: DESIGN_COLORS.border,
    borderRadius: DESIGN_LAYOUT.radius, borderRadiusSM: DESIGN_LAYOUT.radius, borderRadiusLG: DESIGN_LAYOUT.radius,
    fontFamily: DESIGN_TYPE.body, fontSize: 14, fontSizeSM: 12, controlHeight: 32,
    motionDurationFast: `${DESIGN_MOTION.hover}ms`, motionDurationMid: `${DESIGN_MOTION.selection}ms`,
    motionDurationSlow: `${DESIGN_MOTION.drawer}ms`,
  },
};

export const DESIGN_CSS_VARIABLES = Object.fromEntries([
  ...Object.entries(DESIGN_COLORS).map(([name, value]) => [`--color-${name}`, value]),
  ...Object.entries(DESIGN_MOTION).map(([name, value]) => [`--motion-${name}`, `${value}ms`]),
  ...Object.entries(DESIGN_LAYOUT).map(([name, value]) => [`--layout-${name}`, `${value}px`]),
  ['--font-body', DESIGN_TYPE.body], ['--font-numeric', DESIGN_TYPE.numeric], ['--font-equation', DESIGN_TYPE.equation],
]) as CSSProperties;
