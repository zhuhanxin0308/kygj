import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { EnvironmentDetails, RecordInspector } from './RecordInspector';
import { environment, run, result } from '../test/fixtures';

// 检查三种科研状态和来源展示，防止“执行完成”被替代成科学结论。
afterEach(cleanup);
describe('记录与环境检查器', () => {
  it('没有运行和环境时保持明确空状态', () => {
    render(<><RecordInspector run={null} trajectory={null} onSeek={vi.fn()} /><EnvironmentDetails environment={null} /></>);
    expect(screen.getByText('尚无运行结果')).toBeInTheDocument();
    expect(screen.getByText('尚未探测 Python 环境')).toBeInTheDocument();
  });
  it('展示真实依赖和完整引擎内容身份', () => {
    render(<EnvironmentDetails environment={environment} />);
    expect(screen.getByText(environment.pythonExecutable)).toBeInTheDocument();
    expect(screen.getByText(environment.engineSourceHash)).toBeInTheDocument();
  });
  it('选择事件定位时间轴，保留数值验证与人工复核区别', () => {
    const onSeek = vi.fn();
    render(<RecordInspector run={run} trajectory={result.trajectories[0]} onSeek={onSeek} />);
    expect(screen.getByText('执行完成')).toBeInTheDocument();
    expect(screen.getByText('检查通过')).toBeInTheDocument();
    expect(screen.getByText('人工复核未记录')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: /穿过喉部/ }));
    expect(onSeek).toHaveBeenCalledWith(10);
    expect(screen.getByText(/GYOTO 交叉验证/)).toBeInTheDocument();
  });
  it('故障、不收敛与没有事件不被隐藏', () => {
    render(<RecordInspector run={{ ...run, state: 'failed', validationStatus: 'failed', finishedAt: null, error: { code: 'solver_failed', message: '积分未收敛' } }}
      trajectory={{ ...result.trajectories[0], termination: 'solver_failed', events: [], validation: { status: 'failed', checks: [{ name: '能量', actual: 0.1, threshold: 1e-8, passed: false }] } }} onSeek={vi.fn()} />);
    expect(screen.getByText('积分未收敛')).toBeInTheDocument();
    expect(screen.getByText('本次轨迹尚无已确认事件。')).toBeInTheDocument();
    expect(screen.getByText('未通过')).toBeInTheDocument();
    expect(screen.getByText('尚未终止')).toBeInTheDocument();
  });
});
