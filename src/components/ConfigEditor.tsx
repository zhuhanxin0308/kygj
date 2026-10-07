import { Collapse, Form, Input, InputNumber, Typography } from 'antd';
import { DesignNotice as Alert } from './DesignNotice';
import { useEffect, useState } from 'react';
import { INPUT_LIMITS, parseImpactParameters, type EllisConfig } from '../domain/model';

interface Props { config: EllisConfig; onChange(patch: Partial<EllisConfig>): void; onValidityChange?(valid: boolean): void; disabled?: boolean }
// 常用物理参数常显，积分设置渐进展开；单位和约定始终可见。
export function ConfigEditor({ config, onChange, onValidityChange, disabled = false }: Props) {
  const [impacts, setImpacts] = useState(config.impactParameters.join(', '));
  const [error, setError] = useState<string | null>(null);
  useEffect(() => { setImpacts(config.impactParameters.join(', ')); setError(null); onValidityChange?.(true); }, [config.impactParameters, onValidityChange]);
  const number = (key: Exclude<keyof EllisConfig, 'impactParameters'>, label: string, minimum?: number, maximum?: number) => (
    <Form.Item label={label} htmlFor={`config-${key}`}>
      <InputNumber id={`config-${key}`} aria-label={label} value={config[key]} disabled={disabled}
        min={minimum} max={maximum} controls onChange={(value) => onChange({ [key]: value ?? Number.NaN })} style={{ width: '100%' }} />
    </Form.Item>
  );
  const commitImpacts = () => {
    try { const values = parseImpactParameters(impacts); setError(null); onValidityChange?.(true); onChange({ impactParameters: values }); }
    catch { setError('冲量参数须为逗号分隔的有限数字，不能包含空项。'); onValidityChange?.(false); }
  };
  return <Form layout="vertical" size="small" className="config-form">
    <div className="equation-card" aria-label="Ellis 度规">ds² = −dt² + dl² + (l² + a²)dΩ²</div>
    <Typography.Paragraph type="secondary">G = c = 1 · 度规 (−+++) · E = 1<br />a、l₀、b、λ 使用同一几何长度单位。</Typography.Paragraph>
    <div className="form-pair">{number('throatRadius', '喉尺度 a', 0)}{number('initialRadius', '初始径向坐标 l₀', 0)}</div>
    <Form.Item label="冲量参数 b" htmlFor="config-impacts" extra="支持负 b；每条光线以逗号分隔。">
      <Input id="config-impacts" aria-label="冲量参数 b" disabled={disabled} value={impacts}
        onChange={(event) => { setImpacts(event.target.value); onValidityChange?.(false); }} onBlur={commitImpacts} onPressEnter={commitImpacts} />
    </Form.Item>
    {error && <Alert type="error" title={error} showIcon role="alert" />}
    <Collapse ghost items={[{ key: 'integration', label: '积分与采样设置', children: <>
      {number('maxAffineParameter', '仿射参数预算 λ', 0)}
      {number('sampleCount', '采样点数', INPUT_LIMITS.minSamples, INPUT_LIMITS.maxSamples)}
      {number('relativeTolerance', '相对容差 rtol', 0)}
      {number('absoluteTolerance', '绝对容差 atol', 0)}
    </> }]} />
  </Form>;
}
