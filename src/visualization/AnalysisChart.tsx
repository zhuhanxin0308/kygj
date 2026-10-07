import { useEffect, useMemo, useRef, useState } from 'react';
import { Empty, Segmented } from 'antd';
import * as echarts from 'echarts/core';
import { LineChart } from 'echarts/charts';
import { GridComponent, LegendComponent, MarkLineComponent, TooltipComponent, DataZoomComponent } from 'echarts/components';
import { CanvasRenderer } from 'echarts/renderers';
import type { EngineResult } from '../domain/contracts';
import { DISPLAY_LIMITS, displaySamples, residualSeries } from '../domain/geometry';
import { DESIGN_COLORS, DESIGN_TYPE } from '../design/system';
import { rayColor } from './palette';
import './visualization.css';

echarts.use([LineChart, GridComponent, LegendComponent, MarkLineComponent, TooltipComponent, DataZoomComponent, CanvasRenderer]);
export type ChartMode = 'radius' | 'residual';
type ResidualScale = 'linear' | 'log';
type ResidualMetric = 'energyError' | 'angularMomentumError' | 'nullError';
interface Props {
  result: EngineResult | null; selected: number; affine: number; mode: ChartMode;
  onMode(mode: ChartMode): void; onSelect(index: number): void; onSeek(affine: number): void;
  layout?: 'overview' | 'detail';
}
const METRICS = [
  { field: 'energyError', title: '能量', formula: '|ΔE| / E₀' },
  { field: 'angularMomentumError', title: '角动量', formula: '|ΔL| / (aE₀)' },
  { field: 'nullError', title: '零条件', formula: '|gₖₖ| / E₀²' },
] as const;
const CHART = { selectedWidth: 2.5, lineWidth: 1.5, inactiveOpacity: 0.6, gridLeft: 48, gridRight: 18, gridTop: 29, gridBottom: 28 } as const;

// 每条曲线保留参数身份；对数图中的零值留空且在第三维保存原值，不注入对数下限。
export function chartOption(
  result: EngineResult, selected: number, affine: number, mode: ChartMode,
  scale: ResidualScale = 'linear', metric: ResidualMetric = 'nullError',
): echarts.EChartsCoreOption {
  const logarithmic = mode === 'residual' && scale === 'log';
  return {
    animation: false, backgroundColor: 'transparent',
    textStyle: { color: DESIGN_COLORS.muted, fontFamily: DESIGN_TYPE.body, fontSize: 10 },
    grid: { left: CHART.gridLeft, right: CHART.gridRight, top: CHART.gridTop, bottom: CHART.gridBottom },
    legend: { type: 'scroll', top: 0, left: 'center', itemWidth: 16, itemHeight: 2, icon: 'roundRect', textStyle: { color: DESIGN_COLORS.muted, fontSize: 10 }, pageTextStyle: { color: DESIGN_COLORS.muted } },
    tooltip: { trigger: 'axis', backgroundColor: DESIGN_COLORS.surface, borderColor: DESIGN_COLORS.border, textStyle: { color: DESIGN_COLORS.text }, valueFormatter: (value: unknown) => value === null ? '0（对数坐标不显示）' : String(value) },
    xAxis: { type: 'value', name: 'λ', nameLocation: 'middle', nameGap: 18, axisLine: { lineStyle: { color: DESIGN_COLORS.muted } }, axisLabel: { color: DESIGN_COLORS.muted }, splitLine: { lineStyle: { color: DESIGN_COLORS.border, opacity: 0.6 } } },
    yAxis: {
      type: logarithmic ? 'log' : 'value', scale: mode === 'residual',
      axisLabel: { color: DESIGN_COLORS.muted, formatter: (value: number) => mode === 'residual' && value !== 0 ? value.toExponential(0) : String(value) },
      splitLine: { lineStyle: { color: DESIGN_COLORS.border, opacity: 0.6 } },
    },
    dataZoom: [{ type: 'inside', xAxisIndex: 0, filterMode: 'none' }],
    series: result.trajectories.map((ray, index) => {
      const samples = displaySamples(ray.samples);
      const data = mode === 'radius' ? samples.map((sample) => [sample.affine, sample.l])
        : residualSeries(samples, result.config.throatRadius, ray.impactParameter).map((sample) => [sample.affine, logarithmic && sample[metric] === 0 ? null : sample[metric], sample[metric]]);
      const color = rayColor(ray.impactParameter);
      return {
        name: `b = ${ray.impactParameter}`, type: 'line', showSymbol: false, smooth: false, connectNulls: false,
        data, encode: { x: 0, y: 1, tooltip: mode === 'radius' ? [1] : [2] },
        itemStyle: { color }, lineStyle: { color, width: index === selected ? CHART.selectedWidth : CHART.lineWidth, opacity: index === selected ? 1 : CHART.inactiveOpacity },
        ...(index === 0 ? { markLine: { silent: true, symbol: 'none', data: [{ xAxis: affine }], lineStyle: { color: DESIGN_COLORS.text, type: 'dashed', opacity: 0.7 }, label: { show: false } } } : {}),
      };
    }),
  };
}

function ChartCell({ result, selected, affine, onMode, onSelect, onSeek, scale, metric }: Omit<Props, 'result' | 'mode' | 'layout'> & { result: EngineResult; scale: ResidualScale; metric?: typeof METRICS[number] }) {
  const element = useRef<HTMLDivElement>(null);
  const chart = useRef<echarts.EChartsType | null>(null);
  const mode = metric ? 'residual' : 'radius';
  const option = useMemo(() => chartOption(result, selected, affine, mode, scale, metric?.field), [result, selected, affine, mode, scale, metric]);
  useEffect(() => {
    const instance = echarts.init(element.current!, undefined, { renderer: 'canvas' });
    chart.current = instance;
    const observer = new ResizeObserver(() => instance.resize()); observer.observe(element.current!);
    return () => { observer.disconnect(); instance.dispose(); chart.current = null; };
  }, []);
  useEffect(() => { chart.current?.setOption(option, true); }, [option]);
  useEffect(() => {
    const instance = chart.current!;
    const handler = (params: unknown) => {
      const entry = params as { seriesIndex?: number; value?: unknown };
      if (typeof entry.seriesIndex === 'number' && result.trajectories[entry.seriesIndex]) { onSelect(entry.seriesIndex); onMode(mode); }
      if (Array.isArray(entry.value) && typeof entry.value[0] === 'number' && Number.isFinite(entry.value[0])) onSeek(entry.value[0]);
    };
    instance.on('click', handler);
    return () => { instance.off('click', handler); };
  }, [result, mode, onMode, onSelect, onSeek]);
  return <div className="analysis-cell">
    <div className="analysis-cell-heading"><span>{metric ? `${metric.title}残差` : '径向轨迹'}</span><span className="analysis-formula">{metric?.formula ?? 'l(λ)'}</span></div>
    <div ref={element} className="chart-canvas" role="img" aria-label={metric ? `真实样本计算的${metric.title}残差图，点击曲线联动三维选择与时间轴` : '真实径向轨迹图，点击曲线联动三维选择与时间轴'} />
  </div>;
}

// 总览与明细始终并列径向轨迹和守恒曲线；旧 mode/onMode 仍记录当前点击的分析类型。
export function AnalysisChart({ result, selected, affine, mode, onMode, onSelect, onSeek, layout = 'overview' }: Props) {
  const [scale, setScale] = useState<ResidualScale>('log');
  const metrics = layout === 'detail' ? METRICS : [METRICS[2]];
  return <section className="analysis-panel" data-active-analysis={mode}>
    <div className="panel-heading"><div><strong>数据分析联动</strong><span className="subtle">{result ? `选中 b = ${result.trajectories[selected]?.impactParameter ?? result.trajectories[0]?.impactParameter}` : '径向与守恒同步'}</span></div>
      <Segmented size="small" aria-label="残差坐标" value={scale} onChange={(value) => setScale(value as ResidualScale)} options={[{ label: '线性坐标', value: 'linear' }, { label: '对数坐标', value: 'log' }]} />
    </div>
    {result ? <><div className={`analysis-grid analysis-${layout}`}>
      <ChartCell result={result} selected={selected} affine={affine} onMode={onMode} onSelect={onSelect} onSeek={onSeek} scale={scale} />
      {metrics.map((metric) => <ChartCell key={metric.field} result={result} selected={selected} affine={affine} onMode={onMode} onSelect={onSelect} onSeek={onSeek} scale={scale} metric={metric} />)}
    </div><div className="chart-note">真实采样 · 零值不映射为正数{scale === 'log' ? '（零值留空，切换线性查看）' : ''} · 显示≤{DISPLAY_LIMITS.maxLineSamples}点/光线，原始数据保留</div></>
      : <Empty image={Empty.PRESENTED_IMAGE_SIMPLE} description="计算完成后显示真实轨迹、事件与守恒残差。" />}
  </section>;
}
