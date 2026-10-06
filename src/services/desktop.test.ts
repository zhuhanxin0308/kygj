import { describe, expect, it, vi } from 'vitest';
import { createDesktopClient, formatError, fileDialogs } from './desktop';
import { environment, model, preflight, project, run } from '../test/fixtures';
const openDialog = vi.hoisted(() => vi.fn());
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: openDialog }));

// 宿主缺失和损坏返回必须显式失败；只替换传输，不伪造生产能力。
describe('受控桌面 IPC', () => {
  it('浏览器缺少桌面宿主时不发送命令', async () => {
    const invoke = vi.fn();
    const client = createDesktopClient(invoke, () => false);
    await expect(client.getProject('p')).rejects.toMatchObject({ code: 'host_unavailable' });
    expect(invoke).not.toHaveBeenCalled();
  });
  it('始终包装单个 request 且拒绝未通过契约的数据', async () => {
    const invoke = vi.fn().mockResolvedValue({ fake: true });
    const client = createDesktopClient(invoke, () => true);
    await expect(client.getProject('p')).rejects.toMatchObject({ code: 'invalid_response' });
    expect(invoke).toHaveBeenCalledWith('get_project', { request: { projectId: 'p' } });
  });
  it('保留宿主中文结构化错误并隐藏未知异常', () => {
    expect(formatError({ code: 'blocked', message: '环境未就绪' })).toBe('环境未就绪');
    expect(formatError(new Error('internal secret stack'))).toBe('操作未完成，请重试或检查本地环境。');
    expect(formatError(null)).toBe('操作未完成，请重试或检查本地环境。');
  });
  it('所有命令使用固定参数映射并解析契约', async () => {
    const invoke = vi.fn();
    const client = createDesktopClient(invoke, () => true);
    invoke.mockResolvedValueOnce(project);
    expect(await client.createProject('C:/Research', '研究')).toEqual(project);
    expect(invoke).toHaveBeenLastCalledWith('create_project', { request: { parentDirectory: 'C:/Research', name: '研究' } });
    invoke.mockResolvedValueOnce(project);
    await client.openProject(project.project.path);
    expect(invoke).toHaveBeenLastCalledWith('open_project', { request: { directory: project.project.path } });
    invoke.mockResolvedValueOnce(project);
    await client.getProject(project.project.id);
    invoke.mockResolvedValueOnce(model);
    await client.saveModel(model.projectId, model.label, model.config);
    expect(invoke).toHaveBeenLastCalledWith('save_model', { request: { projectId: model.projectId, label: model.label, config: model.config } });
    invoke.mockResolvedValueOnce(environment);
    await client.probeEnvironment(environment.pythonExecutable);
    invoke.mockResolvedValueOnce(preflight);
    await client.prepareRun(model.projectId, model.id, environment.pythonExecutable);
    expect(invoke).toHaveBeenLastCalledWith('prepare_run', { request: { projectId: model.projectId, modelVersionId: model.id, pythonExecutable: environment.pythonExecutable } });
    invoke.mockResolvedValueOnce(run);
    await client.startRun(model.projectId, preflight.id);
    invoke.mockResolvedValueOnce(run);
    await client.getRun(model.projectId, run.id);
    invoke.mockResolvedValueOnce(run);
    await client.cancelRun(model.projectId, run.id);
    expect(invoke).toHaveBeenLastCalledWith('cancel_run', { request: { projectId: model.projectId, runId: run.id } });
    invoke.mockResolvedValueOnce({ path: 'C:/Export/run.json', sha256: 'c'.repeat(64) });
    await client.exportRun(model.projectId, run.id, 'C:/Export');
    expect(invoke).toHaveBeenLastCalledWith('export_run', { request: { projectId: model.projectId, runId: run.id, destinationDirectory: 'C:/Export' } });
  });
  it('非法本地配置在越过 IPC 前失败', async () => {
    const invoke = vi.fn();
    const client = createDesktopClient(invoke, () => true);
    await expect(client.saveModel('p', '版本', { ...model.config, throatRadius: -1 })).rejects.toBeDefined();
    expect(invoke).not.toHaveBeenCalled();
  });
  it('目录与Python选择均仅返回单个授权路径，取消不返回假路径', async () => {
    openDialog.mockResolvedValueOnce('C:/Research').mockResolvedValueOnce(null).mockResolvedValueOnce('C:/Python/python.exe').mockResolvedValueOnce(['a', 'b']);
    expect(await fileDialogs.directory()).toBe('C:/Research');
    expect(await fileDialogs.directory()).toBeNull();
    expect(await fileDialogs.python()).toBe('C:/Python/python.exe');
    expect(await fileDialogs.python()).toBeNull();
    expect(openDialog).toHaveBeenCalledWith(expect.objectContaining({ directory: false, multiple: false }));
  });
  it('结构合法但属于其他项目或运行的回复不能进入当前工作区', async () => {
    const invoke = vi.fn().mockResolvedValueOnce(project).mockResolvedValueOnce(run).mockResolvedValueOnce(preflight);
    const client = createDesktopClient(invoke, () => true);
    await expect(client.getProject('other-project')).rejects.toMatchObject({ code: 'invalid_response' });
    await expect(client.getRun(project.project.id, 'other-run')).rejects.toMatchObject({ code: 'invalid_response' });
    await expect(client.prepareRun(project.project.id, 'other-model', environment.pythonExecutable)).rejects.toMatchObject({ code: 'invalid_response' });
  });
});
