import { cleanup, fireEvent, render, screen, within } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { RunRecord } from '../domain/contracts';
import { run } from '../test/fixtures';
import { RecentResearch } from './RecentResearch';

afterEach(cleanup);

describe('继续研究入口', () => {
  it('空项目没有虚构任务且无法进入空运行列表', () => {
    render(<RecentResearch runs={[]} onSelect={vi.fn()} onViewAll={vi.fn()} />);
    expect(screen.getByText('暂无可继续的运行！')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '查看全部运行' })).toBeDisabled();
  });

  it('仅展示最新三项并按真实身份选择，完整列表仍可进入', () => {
    const onSelect = vi.fn(); const onViewAll = vi.fn();
    const failed: RunRecord = { ...run, id: 'failed-run', state: 'failed', validationStatus: 'not_run', result: null, error: { code: 'process_spawn_failed', message: '进程启动失败' } };
    const active: RunRecord = { ...run, id: 'active-run', state: 'running', validationStatus: 'not_run', result: null, error: null, finishedAt: null };
    const unknown: RunRecord = { ...run, id: 'unknown-run', state: 'unknown', validationStatus: 'not_run', result: null, error: null, finishedAt: null };
    render(<RecentResearch runs={[failed, active, unknown, run]} onSelect={onSelect} onViewAll={onViewAll} />);
    expect(screen.queryByRole('button', { name: `继续研究 ${run.id}` })).not.toBeInTheDocument();
    const failedButton = screen.getByRole('button', { name: '继续研究 failed-run' });
    expect(within(failedButton).getByText('执行失败')).toBeInTheDocument();
    expect(within(failedButton).queryByText('检查通过')).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: '继续研究 unknown-run' }));
    expect(onSelect).toHaveBeenCalledExactlyOnceWith('unknown-run');
    fireEvent.click(screen.getByRole('button', { name: '查看全部运行' }));
    expect(onViewAll).toHaveBeenCalledOnce();
  });
});
