import { useEffect, useMemo, useRef } from 'react';
import { Empty, Segmented } from 'antd';
import * as echarts from 'echarts/core';
import { LineChart } from 'echarts/charts';
import { GridComponent, LegendComponent, MarkLineComponent, TooltipComponent, DataZoomComponent } from 'echarts/components';
import { CanvasRenderer } from 'echarts/renderers';
import type { EngineResult } from '../domain/contracts';
import { displaySamples, residualSeries } from '../domain/geometry';
import { RAY_COLORS } from './palette';

echarts.use([LineChart, GridComponent, LegendComponent, MarkLineComponent, TooltipComponent, DataZoomComponent, CanvasRenderer]);
export type ChartMode = 'radius' | 'residual';
interface Props { result: EngineResult | null; selected: number; affine: number; mode: ChartMode; onMode(mode: ChartMode): void; onSelect(index: number): void; onSeek(affine: number): void }

// 图表直接消费冻结产物，显示抽样不回写数据；零残差保持零而非伪造对数下限。
export function chartOption(result: EngineResult, selected: number, affine: number, mode: ChartMode): echarts.EChartsCoreOption {
  const trajectory = result.trajectories[selected] ?? result.trajectories[0];
  const residuals = residualSeries(displaySamples(trajectory.samples), result.config.throatRadius, trajectory.impactParameter);
  const series = mode === 'radius' ? result.trajectories.map((ray, index) => ({
    name: `b = ${ray.impactParameter}`, type: 'line', showSymbol: false, smooth: false,
    data: displaySamples(ray.samples).map((sample) => [sample.affine, sample.l]),
    lineStyle: { width: index === selected ? 3 : 1, opacity: index === selected ? 1 : 0.4 },
  })) : ([['energyError', '能量'], ['angularMomentumError', '角动量'], ['nullError', '零条件']] as const).map(([field, name]) => ({
    name, type: 'line', showSymbol: false, smooth: false, data: residuals.map((sample) => [sample.affine, sample[field]]),
  }));
  return {
    animation: false, color: RAY_COLORS, backgroundColor: 'transparent',
    textStyle: { color: '#A6B7CF', fontFamily: 'Segoe UI, Microsoft YaHei, sans-serif' },
    grid: { left: 70, right: 24, top: 38, bottom: 56 },
    legend: { top: 0, textStyle: { color: '#A6B7CF' } },
    tooltip: { trigger: 'axis', backgroundColor: '#111A29', borderColor: '#314c65', textStyle: { color: '#EDF4FF' } },
    xAxis: { type: 'value', name: 'λ', nameTextStyle: { color: '#A6B7CF' }, splitLine: { lineStyle: { color: '#1b2b40' } } },
    yAxis: { type: 'value', name: mode === 'radius' ? 'l(λ)' : '归一化绝对残差', scale: true, axisLabel: { formatter: (value: number) => mode === 'residual' && value !== 0 ? value.toExponential(1) : String(value) }, splitLine: { lineStyle: { color: '#1b2b40' } } },
    dataZoom: [{ type: 'inside', xAxisIndex: 0 }, { type: 'slider', height: 14, bottom: 12, borderColor: '#223449' }],
    series: series.map((item, index) => ({ ...item, ...(index === 0 ? { markLine: { silent: true, symbol: 'none', data: [{ xAxis: affine }], lineStyle: { color: '#E6F5FF', type: 'dashed' }, label: { show: false } } } : {}) })),
  };
}

export function AnalysisChart({ result, selected, affine, mode, onMode, onSelect, onSeek }: Props) {
  const element = useRef<HTMLDivElement>(null);
  const chart = useRef<echarts.EChartsType | null>(null);
  const option = useMemo(() => result ? chartOption(result, selected, affine, mode) : null, [result, selected, affine, mode]);
  useEffect(() => {
    if (!element.current || !result) return;
    const instance = echarts.init(element.current, undefined, { renderer: 'canvas' });
    chart.current = instance;
    const observer = new ResizeObserver(() => instance.resize()); observer.observe(element.current);
    return () => { observer.disconnect(); instance.dispose(); chart.current = null; };
  }, [Boolean(result)]);
  useEffect(() => { if (option) chart.current?.setOption(option, true); }, [option]);
  useEffect(() => {
    const instance = chart.current;
    if (!instance) return;
    const handler = (params: unknown) => {
      const entry = params as { seriesIndex?: number; value?: unknown };
      if (mode === 'radius' && typeof entry.seriesIndex === 'number') onSelect(entry.seriesIndex);
      if (Array.isArray(entry.value) && typeof entry.value[0] === 'number') onSeek(entry.value[0]);
    };
    instance.on('click', handler);
    return () => { instance.off('click', handler); };
  }, [result, mode, onSelect, onSeek]);
  return <section className="analysis-panel">
    <div className="panel-heading"><strong>二维分析联动</strong><Segmented size="small" aria-label="分析图类型" value={mode} onChange={(value) => onMode(value as ChartMode)} options={[{ label: '径向轨迹', value: 'radius' }, { label: '守恒残差', value: 'residual' }]} /></div>
    {result ? <><div ref={element} className="chart-canvas" role="img" aria-label={mode === 'radius' ? '真实径向轨迹图，点击曲线联动三维选择与时间轴' : '真实样本计算的守恒残差图'} /><div className="chart-note">展示采样最多 2000 点/光线 · 原始数据完整保留 · 事件定位与验证以引擎记录为准</div></>
      : <Empty image={Empty.PRESENTED_IMAGE_SIMPLE} description="计算完成后显示真实轨迹、事件与守恒残差。" />}
  </section>;
}
