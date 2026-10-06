import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { AnalysisChart, chartOption } from './AnalysisChart';
import { result } from '../test/fixtures';

// 只替换 ECharts 的画布生命周期，图表选项和物理残差使用生产实现。
const mocks = vi.hoisted(() => ({ init: vi.fn(), use: vi.fn(), setOption: vi.fn(), resize: vi.fn(), dispose: vi.fn(), on: vi.fn(), off: vi.fn() }));
vi.mock('echarts/core', () => ({ use: mocks.use, init: mocks.init }));
afterEach(cleanup);
describe('真实结果二维可视化', () => {
  it('径向图保留样本与光标，残差零值保持真实零值', () => {
    const radius = chartOption(result, 0, 10, 'radius') as { series: { data: number[][]; markLine?: { data: unknown[] } }[] };
    expect(radius.series[0].data).toEqual([[0, 10], [20, -10]]);
    expect(radius.series[0].markLine?.data).toEqual([{ xAxis: 10 }]);
    const residual = chartOption(result, 9, 0, 'residual') as { series: { data: number[][] }[]; yAxis: { axisLabel: { formatter(value: number): string } } };
    expect(residual.series).toHaveLength(3);
    expect(residual.series.every((series) => series.data.every((point) => point[1] === 0))).toBe(true);
    expect(residual.yAxis.axisLabel.formatter(0)).toBe('0');
    expect(residual.yAxis.axisLabel.formatter(1e-8)).toBe('1.0e-8');
  });
  it('挂载、更新和卸载图表，曲线点击联动选择与仿射参数', () => {
    mocks.init.mockReturnValue(mocks);
    const onSelect = vi.fn(); const onSeek = vi.fn();
    const props = { result: null, selected: 0, affine: 0, mode: 'radius' as const, onMode: vi.fn(), onSelect, onSeek };
    const view = render(<AnalysisChart {...props} />);
    expect(screen.getByText(/计算完成后显示真实轨迹/)).toBeInTheDocument();
    expect(mocks.init).not.toHaveBeenCalled();
    view.rerender(<AnalysisChart {...props} result={result} />);
    expect(mocks.init).toHaveBeenCalledOnce();
    expect(mocks.setOption).toHaveBeenCalled();
    const handler = mocks.on.mock.calls.at(-1)![1] as (entry: unknown) => void;
    handler({ seriesIndex: 0, value: [8, 2] });
    expect(onSelect).toHaveBeenCalledWith(0); expect(onSeek).toHaveBeenCalledWith(8);
    handler({});
    view.rerender(<AnalysisChart {...props} result={result} mode="residual" />);
    const residualHandler = mocks.on.mock.calls.at(-1)![1] as (entry: unknown) => void;
    residualHandler({ seriesIndex: 2, value: [10, 0] });
    expect(onSelect).toHaveBeenCalledTimes(1);
    view.unmount(); expect(mocks.dispose).toHaveBeenCalledOnce(); expect(mocks.off).toHaveBeenCalled();
  });
});
