import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { MigrationPreview } from './MigrationPreview';
import { migrationPlan } from '../test/verificationFixtures';

// 迁移确认不依赖点击模态背景；可恢复备份与历史哈希限制必须先展示。
afterEach(cleanup);
describe('旧格式迁移审阅', () => {
  it('展示实际计划并分别提供取消、重新预览和明确确认操作', () => {
    const onCancel = vi.fn(); const onRefresh = vi.fn(); const onConfirm = vi.fn();
    const { rerender } = render(<MigrationPreview plan={migrationPlan} busy={false} error={null} onCancel={onCancel} onRefresh={onRefresh} onConfirm={onConfirm} />);
    expect(screen.getByText(migrationPlan.backupPath)).toBeInTheDocument();
    expect(screen.getByText(migrationPlan.warnings[0])).toBeInTheDocument();
    expect(onConfirm).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole('button', { name: '取消迁移' })); expect(onCancel).toHaveBeenCalledOnce();
    fireEvent.click(screen.getByRole('button', { name: '重新预览计划' })); expect(onRefresh).toHaveBeenCalledOnce();
    fireEvent.click(screen.getByRole('button', { name: '确认备份并迁移此项目' })); expect(onConfirm).toHaveBeenCalledOnce();
    rerender(<MigrationPreview plan={migrationPlan} busy error="项目已变化" onCancel={onCancel} onRefresh={onRefresh} onConfirm={onConfirm} />);
    expect(screen.getByRole('button', { name: '取消迁移' })).toBeDisabled();
    expect(screen.getByText('项目已变化')).toBeInTheDocument();
  });
});
