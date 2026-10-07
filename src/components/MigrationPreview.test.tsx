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
  it('迁移已提交后的恢复界面展示完整回执且只允许重新打开', () => {
    const onConfirm = vi.fn(); const onRefresh = vi.fn(); const onRetry = vi.fn();
    const receipt = { planId: migrationPlan.id, directory: migrationPlan.directory, projectId: migrationPlan.projectId, fromVersion: 1 as const, toVersion: 2 as const, backupPath: migrationPlan.backupPath, backupSha256: 'f'.repeat(64), migratedAt: migrationPlan.createdAt, legacyResultCount: 1 };
    const props = { plan: null, receipt, busy: false, error: '读取暂时失败', onCancel: vi.fn(), onConfirm, onRefresh, onRetry };
    const { rerender } = render(<MigrationPreview {...props} />);
    expect(screen.getByText(receipt.backupSha256)).toBeInTheDocument();
    expect(screen.getByText(receipt.backupPath)).toBeInTheDocument();
    expect(screen.getByText('读取暂时失败')).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: '确认备份并迁移此项目' })).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: '重新预览计划' })).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: '重新打开已迁移项目' }));
    expect(onRetry).toHaveBeenCalledOnce();
    expect(onConfirm).not.toHaveBeenCalled(); expect(onRefresh).not.toHaveBeenCalled();
    rerender(<MigrationPreview {...props} busy />);
    expect(screen.getByRole('button', { name: '关闭迁移回执' })).toBeDisabled();
  });
});
