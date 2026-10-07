import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import '@testing-library/jest-dom/vitest';
import { run } from '../test/fixtures';
import { SelectedRaySummary } from './SelectedRaySummary';

afterEach(cleanup);
const trajectory = run.result!.trajectories[0];

// 摘要测试关注来源身份与缺失状态；不会把显示夹具作为科研正确性的证据。
describe('选中光线的只读摘要', () => {
  it('无运行、无产物或无选中轨迹时不显示虚构的冻结参数', () => {
    const { rerender } = render(<SelectedRaySummary run={null} trajectory={null} previewDraft={false} />);
    expect(screen.queryByRole('region', { name: '选中光线摘要' })).not.toBeInTheDocument();
    rerender(<SelectedRaySummary run={{ ...run, result: null }} trajectory={trajectory} previewDraft={false} />);
    expect(screen.queryByLabelText('冻结喉尺度 a')).not.toBeInTheDocument();
    rerender(<SelectedRaySummary run={run} trajectory={null} previewDraft={false} />);
    expect(screen.queryByLabelText('冻结初始径向坐标 l₀')).not.toBeInTheDocument();
  });
  it('显示真实选中光线身份和冻结值，不提供原位修改入口', () => {
    render(<SelectedRaySummary run={run} trajectory={trajectory} previewDraft={false} />);
    expect(screen.getByText('b = 0')).toBeInTheDocument();
    expect(screen.getByText(run.id)).toBeInTheDocument();
    expect(screen.getByLabelText('冻结喉尺度 a')).toHaveTextContent('1');
    expect(screen.getByLabelText('冻结初始径向坐标 l₀')).toHaveTextContent('10');
    expect(screen.queryByRole('spinbutton')).not.toBeInTheDocument();
  });
  it('草稿预览明确替代运行摘要，返回结果后恢复原冻结值', () => {
    const { rerender } = render(<SelectedRaySummary run={run} trajectory={trajectory} previewDraft />);
    expect(screen.getByText('当前为草稿几何预览！')).toBeInTheDocument();
    expect(screen.queryByLabelText('冻结喉尺度 a')).not.toBeInTheDocument();
    rerender(<SelectedRaySummary run={run} trajectory={trajectory} previewDraft={false} />);
    expect(screen.queryByText('当前为草稿几何预览！')).not.toBeInTheDocument();
    expect(screen.getByLabelText('冻结喉尺度 a')).toHaveTextContent('1');
  });
});
