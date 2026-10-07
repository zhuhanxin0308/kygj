import { ExclamationCircleOutlined } from '@ant-design/icons';
import { Alert, Tag, type AlertProps } from 'antd';
import type { ReactNode } from 'react';
import { DESIGN_COLORS } from '../design/system';

// 提示使用统一感叹号图标；错误仍保留alert语义，颜色与状态文案共同表达含义。
export function DesignNotice(props: AlertProps) {
  return <Alert {...props} showIcon icon={<ExclamationCircleOutlined />} />;
}

const TONE_COLORS = {
  success: DESIGN_COLORS.success, error: DESIGN_COLORS.error, warning: DESIGN_COLORS.warning,
  info: DESIGN_COLORS.primary, neutral: DESIGN_COLORS.muted,
} as const;

// 不使用AntD预设green/gold等独立色板，避免同一状态在不同模块含义漂移。
export function SemanticTag({ tone = 'neutral', children }: { tone?: keyof typeof TONE_COLORS; children: ReactNode }) {
  const color = TONE_COLORS[tone];
  return <Tag className="semantic-tag" style={{ color, borderColor: color, background: `color-mix(in srgb, ${color} 12%, transparent)` }}>{children}</Tag>;
}
