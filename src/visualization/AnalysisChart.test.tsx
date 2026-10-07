import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { AnalysisChart, chartOption } from './AnalysisChart';
import { result } from '../test/fixtures';
import { rayColor } from './palette';

// 只替换 ECharts 的画布生命周期，图表选项和物理残差使用生产实现。
const mocks = vi.hoisted(() => ({ init: vi.fn(), use: vi.fn(), setOption: vi.fn(), resize: vi.fn(), dispose: vi.fn(), on: vi.fn(), off: vi.fn() }));
vi.mock('echarts/core', () => ({ use: mocks.use, init: mocks.init }));
afterEach(cleanup);
type Option = { series: { data: (number | null)[][]; lineStyle: { color: string }; markLine?: { data: unknown[] } }[]; yAxis: { type: string; axisLabel: { formatter(value: number): string } } };
const props = { result, selected: 0, affine: 0, mode: 'radius' as const, onMode: vi.fn(), onSelect: vi.fn(), onSeek: vi.fn() };
describe('真实结果二维可视化', () => {
  it('径向图保留样本、光标和参数绑定颜色', () => {
    const radius = chartOption(result, 0, 10, 'radius') as Option;
    expect(radius.series[0].data).toEqual([[0, 10], [20, -10]]);
    expect(radius.series[0].markLine?.data).toEqual([{ xAxis: 10 }]);
    expect(radius.series[0].lineStyle.color).toBe(rayColor(0));
  });
  it('线性残差保留零，对数残差只留缺口且保留原值，不伪造正值', () => {
    const linear = chartOption(result, 9, 0, 'residual', 'linear', 'nullError') as Option;
    const logarithmic = chartOption(result, 0, 0, 'residual', 'log', 'nullError') as Option;
    expect(linear.series[0].data).toEqual([[0, 0, 0], [20, 0, 0]]);
    expect(logarithmic.yAxis.type).toBe('log');
    expect(logarithmic.series[0].data).toEqual([[0, null, 0], [20, null, 0]]);
    expect(linear.yAxis.axisLabel.formatter(0)).toBe('0');
    expect(linear.yAxis.axisLabel.formatter(1e-8)).toBe('1e-8');
    expect(result.trajectories[0].samples[0].kt).toBe(1);
  });
  it('非零守恒量残差直接来自原始样本，颜色不跟随输入顺序变化', () => {
    const rays = [2, 0.5, 0].map((impactParameter) => ({ ...result.trajectories[0], impactParameter, samples: result.trajectories[0].samples.map((sample) => ({ ...sample, kt: 1.01 })) }));
    const option = chartOption({ ...result, trajectories: rays }, 1, 0, 'residual', 'log', 'energyError') as Option;
    expect(option.series.map((series) => series.lineStyle.color)).toEqual([rayColor(2), rayColor(0.5), rayColor(0)]);
    expect(option.series[0].data[0][1]).toBeCloseTo(0.01);
  });
  it('总览同时显示径向与守恒图，并能选择线性坐标查看真实零值', () => {
    mocks.init.mockReturnValue(mocks);
    render(<AnalysisChart {...props} />);
    expect(screen.getByRole('img', { name: /径向轨迹图/ })).toBeInTheDocument();
    expect(screen.getByRole('img', { name: /零条件残差图/ })).toBeInTheDocument();
    expect(mocks.init).toHaveBeenCalledTimes(2);
    expect(screen.getByText(/零值不映射为正数/)).toBeInTheDocument();
    fireEvent.click(screen.getByText('线性坐标'));
    expect((mocks.setOption.mock.calls.at(-1)![0] as Option).yAxis.type).toBe('value');
  });
  it('详细布局包含三个真实守恒量小图，各图均能联动光线与时间轴', () => {
    mocks.init.mockReturnValue(mocks);
    const onSelect = vi.fn(); const onSeek = vi.fn(); const onMode = vi.fn();
    const view = render(<AnalysisChart {...props} layout="detail" onSelect={onSelect} onSeek={onSeek} onMode={onMode} />);
    expect(screen.getAllByRole('img')).toHaveLength(4);
    expect(screen.getByRole('img', { name: /能量残差图/ })).toBeInTheDocument();
    expect(screen.getByRole('img', { name: /角动量残差图/ })).toBeInTheDocument();
    const handler = mocks.on.mock.calls.at(-1)![1] as (entry: unknown) => void;
    handler({ seriesIndex: 0, value: [8, 2] });
    expect(onSelect).toHaveBeenCalledWith(0); expect(onSeek).toHaveBeenCalledWith(8);
    expect(onMode).toHaveBeenCalledWith('residual');
    handler({});
    view.unmount();
    expect(mocks.dispose).toHaveBeenCalledTimes(4);
    expect(mocks.off).toHaveBeenCalledTimes(4);
  });
  it('无结果展示空态，结果载入后创建两图并完整清理', () => {
    mocks.init.mockReturnValue(mocks);
    const view = render(<AnalysisChart {...props} result={null} />);
    expect(screen.getByText(/计算完成后显示真实轨迹/)).toBeInTheDocument();
    expect(mocks.init).not.toHaveBeenCalled();
    view.rerender(<AnalysisChart {...props} />);
    expect(mocks.init).toHaveBeenCalledTimes(2);
    view.unmount(); expect(mocks.dispose).toHaveBeenCalledTimes(2);
  });
});
