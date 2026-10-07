import { theme, type ThemeConfig } from 'antd';
import { DESIGN_COLORS, DESIGN_LAYOUT, DESIGN_MOTION, DESIGN_TYPE } from './system';

// 组件库主题在界面装配时加载；纯配色、相机和图形模块无需加载完整组件库。
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
