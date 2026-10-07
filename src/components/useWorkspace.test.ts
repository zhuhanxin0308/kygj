import { act, renderHook, waitFor, cleanup } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { useWorkspace } from './useWorkspace';
import type { DesktopClient, FileDialogs } from '../services/desktop';
import { environment, model, preflight, project, run } from '../test/fixtures';
import { migrationPlan, verificationState } from '../test/verificationFixtures';

// 用受控 IPC 传输检查提交顺序、失败保留和取消语义，后台没有实际进程。
const makeClient = () => ({
  available: () => true,
  createProject: vi.fn().mockResolvedValue(project), openProject: vi.fn().mockResolvedValue(project),
  getProject: vi.fn().mockResolvedValue(project), saveModel: vi.fn().mockResolvedValue({ ...model, id: 'model-2' }),
  probeEnvironment: vi.fn().mockResolvedValue(environment), prepareRun: vi.fn().mockResolvedValue(preflight),
  startRun: vi.fn().mockResolvedValue({ ...run, state: 'running', result: null }),
  getRun: vi.fn().mockResolvedValue(run), cancelRun: vi.fn().mockResolvedValue({ ...run, state: 'cancelling' }),
  exportRun: vi.fn().mockResolvedValue({ path: 'C:/Export/run.json', sha256: 'c'.repeat(64) }),
  listVerificationRules: vi.fn().mockResolvedValue({ rules: [], total: 0, nextOffset: null }),
  listVerificationRecords: vi.fn().mockResolvedValue({ records: [], total: 0, nextOffset: null }),
  getRunVerificationState: vi.fn().mockResolvedValue(verificationState), executeVerification: vi.fn(),
  saveVerificationRuleVersion: vi.fn(), getVerificationRecord: vi.fn(),
  prepareProjectMigration: vi.fn().mockResolvedValue(migrationPlan), applyProjectMigration: vi.fn(),
}) satisfies DesktopClient;
const makeDialogs = () => ({ directory: vi.fn().mockResolvedValue('C:/Research'), python: vi.fn().mockResolvedValue('C:/Python/python.exe') }) satisfies FileDialogs;
afterEach(() => { cleanup(); vi.useRealTimers(); });

describe('真实工作区操作编排', () => {
  it('旧项目先预览迁移，取消不修改项目，明确确认后按回执重新打开', async () => {
    const client = makeClient();
    client.openProject.mockRejectedValueOnce({ code: 'migration_required', message: '需要预览迁移' });
    const { result } = renderHook(() => useWorkspace(client, makeDialogs()));
    await act(() => result.current.openProject());
    expect(result.current.migrationPlan).toEqual(migrationPlan);
    expect(client.applyProjectMigration).not.toHaveBeenCalled();
    act(() => result.current.cancelMigration());
    expect(result.current.migrationPlan).toBeNull();
    client.openProject.mockRejectedValueOnce({ code: 'migration_required', message: '需要预览迁移' });
    await act(() => result.current.openProject());
    client.applyProjectMigration.mockResolvedValue({ planId: migrationPlan.id, directory: migrationPlan.directory, projectId: migrationPlan.projectId, fromVersion: 1, toVersion: 2, backupPath: migrationPlan.backupPath, backupSha256: 'f'.repeat(64), migratedAt: model.createdAt, legacyResultCount: 1 });
    await act(() => result.current.confirmMigration());
    expect(client.applyProjectMigration).toHaveBeenCalledWith(migrationPlan.directory, migrationPlan.id);
    expect(client.openProject).toHaveBeenLastCalledWith(migrationPlan.directory);
    expect(result.current.state.project?.project.id).toBe(migrationPlan.projectId);
    expect(result.current.migrationPlan).toBeNull();
    expect(result.current.notice).toContain(migrationPlan.backupPath);
  });
  it('迁移计划过期保留原项目与预览，重新预览不能自动执行', async () => {
    const client = makeClient();
    const { result } = renderHook(() => useWorkspace(client, makeDialogs()));
    await act(() => result.current.openProject());
    client.openProject.mockRejectedValueOnce({ code: 'migration_required', message: '需要迁移' });
    await act(() => result.current.openProject());
    client.applyProjectMigration.mockRejectedValue({ code: 'migration_plan_stale', message: '项目已变化，请重新预览' });
    await act(() => result.current.confirmMigration());
    expect(result.current.state.project).toEqual(project);
    expect(result.current.error).toContain('重新预览');
    await act(() => result.current.refreshMigration());
    expect(client.applyProjectMigration).toHaveBeenCalledTimes(1);
    expect(client.prepareProjectMigration).toHaveBeenLastCalledWith(migrationPlan.directory);
  });
  it('已完成迁移但读取失败时保留成功回执，重试只打开项目不重复迁移', async () => {
    const client = makeClient();
    client.openProject.mockRejectedValueOnce({ code: 'migration_required', message: '需要迁移' });
    client.applyProjectMigration.mockResolvedValue({ planId: migrationPlan.id, directory: migrationPlan.directory, projectId: migrationPlan.projectId, fromVersion: 1, toVersion: 2, backupPath: migrationPlan.backupPath, backupSha256: 'f'.repeat(64), migratedAt: model.createdAt, legacyResultCount: 1 });
    const { result } = renderHook(() => useWorkspace(client, makeDialogs()));
    await act(() => result.current.openProject());
    client.openProject.mockRejectedValueOnce({ code: 'read_failed', message: '迁移成功后暂时读取失败' });
    await act(() => result.current.confirmMigration());
    expect(result.current.migrationReceipt?.planId).toBe(migrationPlan.id);
    expect(result.current.migrationPlan).toBeNull();
    await act(() => result.current.retryMigratedProject());
    expect(client.applyProjectMigration).toHaveBeenCalledTimes(1);
    expect(client.openProject).toHaveBeenLastCalledWith(migrationPlan.directory);
    expect(result.current.state.project?.project.id).toBe(migrationPlan.projectId);
    expect(result.current.migrationReceipt).toBeNull();
    expect(result.current.notice).toContain('f'.repeat(64));
  });
  it('迁移回执保留期间拒绝再次确认，身份错误和再次读取失败均不覆盖当前项目', async () => {
    const client = makeClient();
    const { result } = renderHook(() => useWorkspace(client, makeDialogs()));
    await act(() => result.current.openProject());
    client.openProject.mockRejectedValueOnce({ code: 'migration_required', message: '需要迁移' });
    await act(() => result.current.openProject());
    const receipt = { planId: migrationPlan.id, directory: migrationPlan.directory, projectId: migrationPlan.projectId, fromVersion: 1, toVersion: 2, backupPath: migrationPlan.backupPath, backupSha256: 'f'.repeat(64), migratedAt: model.createdAt, legacyResultCount: 1 };
    client.applyProjectMigration.mockResolvedValue(receipt);
    client.openProject.mockResolvedValueOnce({ ...project, project: { ...project.project, id: 'wrong-project' } });
    await act(() => result.current.confirmMigration());
    expect(result.current.error).toContain('身份不一致');
    expect(result.current.state.project).toEqual(project);
    expect(result.current.migrationReceipt).toEqual(receipt);
    await act(() => result.current.confirmMigration());
    expect(client.applyProjectMigration).toHaveBeenCalledTimes(1);
    client.openProject.mockRejectedValueOnce({ code: 'read_failed', message: '再次读取失败' });
    await act(() => result.current.retryMigratedProject());
    expect(result.current.migrationReceipt).toEqual(receipt);
    expect(result.current.error).toBe('再次读取失败');
    act(() => result.current.cancelMigration());
    expect(result.current.migrationReceipt).toBeNull();
    await act(() => result.current.retryMigratedProject());
    expect(result.current.error).toContain('没有需要重新打开');
  });
  it('创建项目、保存版本、探测、预检、提交、查询与导出闭环', async () => {
    const client = makeClient();
    const dialogs = makeDialogs();
    const { result } = renderHook(() => useWorkspace(client, dialogs));
    await act(() => result.current.createProject('研究'));
    expect(client.createProject).toHaveBeenCalledWith('C:/Research', '研究');
    expect(result.current.state.project?.project.id).toBe(project.project.id);
    await act(() => result.current.saveModel('新版本'));
    expect(result.current.state.selectedModelId).toBe('model-2');
    await act(() => result.current.choosePython());
    expect(result.current.state.environment).toEqual(environment);
    client.prepareRun.mockResolvedValue({ ...preflight, modelVersionId: 'model-2' });
    await act(() => result.current.prepareRun());
    expect(result.current.state.preflight?.id).toBe(preflight.id);
    await act(() => result.current.startRun());
    expect(client.startRun).toHaveBeenCalledWith(project.project.id, preflight.id);
    expect(result.current.state.preflight).toBeNull();
    await act(() => result.current.refresh());
    await act(() => result.current.exportRun());
    expect(result.current.notice).toContain('run.json');
  });
  it('取消文件选择没有副作用；错误信息可见且保留原有项目', async () => {
    const client = makeClient();
    const dialogs = makeDialogs();
    dialogs.directory.mockResolvedValueOnce(null as never);
    const { result } = renderHook(() => useWorkspace(client, dialogs));
    await act(() => result.current.openProject());
    expect(client.openProject).not.toHaveBeenCalled();
    await act(() => result.current.openProject());
    client.saveModel.mockRejectedValueOnce({ code: 'disk_full', message: '磁盘空间不足' });
    await act(() => result.current.saveModel('新版本'));
    expect(result.current.error).toBe('磁盘空间不足');
    expect(result.current.state.project?.project.id).toBe(project.project.id);
  });
  it('输入改变必须重新保存版本再预检，不会使用旧参数', async () => {
    const client = makeClient();
    const { result } = renderHook(() => useWorkspace(client, makeDialogs()));
    await act(() => result.current.openProject());
    act(() => result.current.dispatch({ type: 'draftChanged', patch: { throatRadius: 2 } }));
    await act(() => result.current.prepareRun());
    expect(client.prepareRun).not.toHaveBeenCalled();
    expect(result.current.error).toContain('保存');
  });
  it('预检阻断与取消中均不伪报成功', async () => {
    const client = makeClient();
    client.openProject.mockResolvedValue({ ...project, runs: [{ ...run, state: 'running', result: null }] });
    client.prepareRun.mockResolvedValue({ ...preflight, status: 'blocked', issues: [{ code: 'missing', message: '依赖缺失' }] });
    const { result } = renderHook(() => useWorkspace(client, makeDialogs()));
    await act(() => result.current.openProject());
    await act(() => result.current.choosePython());
    await act(() => result.current.prepareRun());
    await act(() => result.current.startRun());
    expect(client.startRun).not.toHaveBeenCalled();
    await act(() => result.current.cancelRun());
    expect(result.current.state.project?.runs[0].state).toBe('cancelling');
  });
  it('仅轮询活动状态并根据真实退出结果结束', async () => {
    const client = makeClient();
    client.openProject.mockResolvedValue({ ...project, runs: [{ ...run, state: 'running', result: null }] });
    const { result } = renderHook(() => useWorkspace(client, makeDialogs()));
    await act(() => result.current.openProject());
    await waitFor(() => expect(client.getRun).toHaveBeenCalled(), { timeout: 3000 });
    await waitFor(() => expect(result.current.state.project?.runs[0].state).toBe('completed'));
  });
  it('没有项目、环境、合法参数或版本名时不会调用执行接口', async () => {
    const client = makeClient();
    const { result } = renderHook(() => useWorkspace(client, makeDialogs()));
    await act(() => result.current.saveModel('版本'));
    expect(result.current.error).toContain('打开项目');
    await act(() => result.current.createProject('../越界'));
    expect(client.createProject).not.toHaveBeenCalled();
    await act(() => result.current.openProject());
    await act(() => result.current.prepareRun());
    expect(result.current.error).toContain('Python');
    await act(() => result.current.saveModel(' '));
    expect(result.current.error).toContain('版本名称');
    act(() => result.current.dispatch({ type: 'draftChanged', patch: { throatRadius: 0 } }));
    await act(() => result.current.saveModel('版本'));
    expect(result.current.error).toContain('参数无效');
    expect(client.saveModel).not.toHaveBeenCalled();
    act(() => result.current.clearError());
    expect(result.current.error).toBeNull();
  });
  it('无运行不可取消或导出，取消Python选择保留原状态', async () => {
    const client = makeClient(); const dialogs = makeDialogs();
    client.openProject.mockResolvedValue({ ...project, runs: [] });
    dialogs.python.mockResolvedValue(null as never);
    const { result } = renderHook(() => useWorkspace(client, dialogs));
    await act(() => result.current.openProject());
    await act(() => result.current.cancelRun());
    expect(result.current.error).toContain('选择运行');
    await act(() => result.current.exportRun());
    expect(result.current.error).toContain('导出的运行');
    await act(() => result.current.choosePython());
    expect(client.probeEnvironment).not.toHaveBeenCalled();
  });
  it('并发点击只启动一个操作，导出取消不写文件', async () => {
    const client = makeClient(); const dialogs = makeDialogs();
    const { result } = renderHook(() => useWorkspace(client, dialogs));
    await act(() => Promise.all([result.current.openProject(), result.current.openProject()]));
    expect(client.openProject).toHaveBeenCalledOnce();
    dialogs.directory.mockResolvedValue(null as never);
    await act(() => result.current.exportRun());
    expect(client.exportRun).not.toHaveBeenCalled();
    await act(() => result.current.createProject('合法名称'));
    expect(client.createProject).not.toHaveBeenCalled();
  });
  it('预检期间修改草稿会丢弃过期报告', async () => {
    const client = makeClient();
    let resolve: (value: typeof preflight) => void = () => undefined;
    client.prepareRun.mockImplementation(() => new Promise((finish) => { resolve = finish; }));
    const { result } = renderHook(() => useWorkspace(client, makeDialogs()));
    await act(() => result.current.openProject());
    await act(() => result.current.choosePython());
    let pending: Promise<void>;
    act(() => { pending = result.current.prepareRun(); });
    act(() => result.current.dispatch({ type: 'draftChanged', patch: { throatRadius: 2 } }));
    await act(async () => { resolve(preflight); await pending; });
    expect(result.current.state.preflight).toBeNull();
  });
  it('轮询失败保留已有记录并公开错误，不将未知状态当失败', async () => {
    const client = makeClient();
    client.openProject.mockResolvedValue({ ...project, runs: [{ ...run, state: 'running', result: null }] });
    client.getRun.mockRejectedValue({ code: 'unavailable', message: '宿主暂不可达' });
    const { result } = renderHook(() => useWorkspace(client, makeDialogs()));
    await act(() => result.current.openProject());
    await waitFor(() => expect(result.current.error).toContain('宿主暂不可达'), { timeout: 3000 });
    expect(result.current.state.project?.runs[0].state).toBe('running');
    expect(client.startRun).not.toHaveBeenCalled();
  });
});
