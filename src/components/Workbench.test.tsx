import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import '@testing-library/jest-dom/vitest';
import { Workbench } from './Workbench';
import type { DesktopClient, FileDialogs } from '../services/desktop';
import { environment, model, preflight, project, run } from '../test/fixtures';

// GPU 与图表画布只替换渲染边界，工作区使用真实状态与服务接口。
vi.mock('../visualization/ResearchScene', () => ({ ResearchScene: ({ config, result, affine }: { config: { throatRadius: number }; result: unknown; affine: number }) => <div data-testid="scene" data-throat={config?.throatRadius} data-result={String(Boolean(result))} data-affine={affine}>Ellis 几何视图</div>, RAY_COLORS: ['#3EDCFF'] }));
vi.mock('../visualization/AnalysisChart', () => ({ AnalysisChart: () => <div>真实轨迹分析</div> }));
afterEach(cleanup);
const makeClient = () => ({
  available: () => true, createProject: vi.fn().mockResolvedValue(project), openProject: vi.fn().mockResolvedValue(project),
  getProject: vi.fn().mockResolvedValue(project), saveModel: vi.fn().mockResolvedValue({ ...model, id: 'model-2', label: '新版本' }),
  probeEnvironment: vi.fn().mockResolvedValue(environment), prepareRun: vi.fn().mockResolvedValue(preflight),
  startRun: vi.fn().mockResolvedValue({ ...run, state: 'running', result: null }), getRun: vi.fn().mockResolvedValue(run),
  cancelRun: vi.fn().mockResolvedValue({ ...run, state: 'cancelling', result: null }),
  exportRun: vi.fn().mockResolvedValue({ path: 'C:/Export/run.json', sha256: 'c'.repeat(64) }),
  listVerificationRules: vi.fn().mockResolvedValue({ rules: [], total: 0, nextOffset: null }),
  listVerificationRecords: vi.fn().mockResolvedValue({ records: [], total: 0, nextOffset: null }),
  getRunVerificationState: vi.fn(), executeVerification: vi.fn(), saveVerificationRuleVersion: vi.fn(), getVerificationRecord: vi.fn(),
  prepareProjectMigration: vi.fn(), applyProjectMigration: vi.fn(),
}) satisfies DesktopClient;
const dialogs: FileDialogs = { directory: async () => 'C:/Research', python: async () => environment.pythonExecutable };
// 工作区包含完整主题和模态层，使用独立的交互测试时限，不改变产品或科学预算。
const WORKBENCH_TEST_TIMEOUT_MS = 20000;
describe('工作区失败与可访问行为', () => {
  it('资源栏和检查器可独立收起，恢复后草稿与冻结运行保持原样', async () => {
    const client = makeClient();
    render(<Workbench client={client} dialogs={dialogs} />);
    fireEvent.click(screen.getByRole('button', { name: /打开项目/ }));
    await screen.findByText(model.label);
    fireEvent.change(screen.getByLabelText('喉尺度 a'), { target: { value: '2' } });
    fireEvent.click(screen.getByRole('button', { name: '收起项目资源' }));
    expect(screen.getByRole('button', { name: '展开项目资源' })).toHaveAttribute('aria-expanded', 'false');
    expect(screen.queryByRole('complementary', { name: '项目资源' })).not.toBeInTheDocument();
    expect(screen.getByTestId('scene')).toHaveAttribute('data-throat', '1');
    fireEvent.click(screen.getByRole('button', { name: '展开项目资源' }));
    expect(screen.getByRole('complementary', { name: '项目资源' })).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: '收起检查器' }));
    expect(screen.queryByRole('complementary', { name: '上下文检查器' })).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: '展开检查器' }));
    expect(screen.getByLabelText('喉尺度 a')).toHaveValue('2');
    expect(client.startRun).not.toHaveBeenCalled();
  }, 60000);
  it('继续研究只列真实运行并保留执行、检查和复核的独立状态', async () => {
    const client = makeClient();
    render(<Workbench client={client} dialogs={dialogs} />);
    expect(screen.getByText('暂无可继续的运行！')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: /打开项目/ }));
    await screen.findByText(model.label);
    const recent = screen.getByRole('region', { name: '继续研究' });
    expect(within(recent).getByRole('button', { name: `继续研究 ${run.id}` })).toBeInTheDocument();
    expect(within(recent).getByText('执行完成')).toBeInTheDocument();
    expect(within(recent).getByText('检查通过')).toBeInTheDocument();
    fireEvent.click(within(recent).getByRole('button', { name: `继续研究 ${run.id}` }));
    expect(screen.getByTestId('scene')).toHaveAttribute('data-result', 'true');
    expect(client.startRun).not.toHaveBeenCalled();
  }, 60000);
  it('无宿主时明确说明不可运行，没有虚构轨迹或进度', () => {
    render(<Workbench />);
    expect(screen.getByText(/浏览器预览未连接桌面宿主/)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '创建项目' })).toBeDisabled();
    expect(screen.getByText('尚无运行结果')).toBeInTheDocument();
    expect(screen.queryByText('100%')).not.toBeInTheDocument();
  }, WORKBENCH_TEST_TIMEOUT_MS);
  it('命令搜索可用键盘打开，未实现能力不可触发', () => {
    render(<Workbench />);
    fireEvent.keyDown(window, { key: 'k', ctrlKey: true });
    expect(screen.getByRole('dialog', { name: '命令搜索' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '工作流（尚未接入）' })).toBeDisabled();
  }, WORKBENCH_TEST_TIMEOUT_MS);
  it('真实项目结果、来源、原始样本与草稿几何保持独立', async () => {
    const client = makeClient();
    render(<Workbench client={client} dialogs={dialogs} />);
    fireEvent.click(screen.getByRole('button', { name: /打开项目/ }));
    await screen.findByText(model.label);
    expect(screen.getByTestId('scene')).toHaveAttribute('data-result', 'true');
    fireEvent.change(screen.getByLabelText('喉尺度 a'), { target: { value: '2' } });
    expect(screen.getByTestId('scene')).toHaveAttribute('data-throat', '1');
    fireEvent.click(screen.getByRole('button', { name: '预览草稿几何' }));
    expect(screen.getByTestId('scene')).toHaveAttribute('data-throat', '2');
    expect(screen.getByTestId('scene')).toHaveAttribute('data-result', 'false');
    fireEvent.click(screen.getByRole('button', { name: '查看运行结果' }));
    fireEvent.click(screen.getByRole('button', { name: '回到轨迹起点' }));
    fireEvent.click(screen.getByRole('button', { name: '前进一个采样间隔' }));
    fireEvent.click(screen.getByRole('button', { name: '播放展示' }));
    await waitFor(() => expect(Number(screen.getByTestId('scene').getAttribute('data-affine'))).toBeGreaterThan(0));
    fireEvent.click(screen.getByRole('button', { name: '暂停展示' }));
    expect(client.cancelRun).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole('button', { name: /原始样本/ }));
    const data = await screen.findByRole('dialog', { name: '原始轨迹样本' });
    expect(within(data).getByText(/原始样本 2 点/)).toBeInTheDocument();
    fireEvent.click(within(data).getAllByText('20.000000')[0].closest('tr')!);
    fireEvent.click(screen.getByRole('tab', { name: '来源' }));
    expect(await screen.findByText(model.contentHash)).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: /导出 JSON 研究记录/ }));
    await waitFor(() => expect(client.exportRun).toHaveBeenCalledWith(project.project.id, run.id, 'C:/Research'));
    fireEvent.click(screen.getByRole('button', { name: '运行' }));
    const records = await screen.findByRole('dialog', { name: '运行记录' });
    fireEvent.click(within(records).getByRole('button', { name: run.id }));
    fireEvent.click(screen.getByRole('button', { name: '验证' }));
    expect(await screen.findByText('人工复核未记录')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: '刷新项目运行' }));
    await waitFor(() => expect(client.getProject).toHaveBeenCalled());
    fireEvent.change(screen.getByLabelText('筛选项目资源'), { target: { value: '不存在' } });
    expect(screen.queryByRole('button', { name: model.label })).not.toBeInTheDocument();
  }, 60000);
  it('创建、保存、选择环境与预检确认后才提交，并可请求取消', async () => {
    const client = makeClient();
    client.createProject.mockResolvedValue({ ...project, runs: [] });
    client.getRun.mockResolvedValue({ ...run, state: 'running', result: null });
    client.prepareRun.mockResolvedValue({ ...preflight, modelVersionId: 'model-2' });
    render(<Workbench client={client} dialogs={dialogs} />);
    fireEvent.click(screen.getByRole('button', { name: '创建项目' }));
    fireEvent.change(await screen.findByLabelText('新项目名称'), { target: { value: '新研究' } });
    fireEvent.click(screen.getByRole('button', { name: '选择目录并创建' }));
    await waitFor(() => expect(client.createProject).toHaveBeenCalledWith('C:/Research', '新研究'));
    await screen.findByText(model.label);
    fireEvent.change(screen.getByLabelText('模型版本名称'), { target: { value: '新版本' } });
    fireEvent.click(screen.getByRole('button', { name: /保存版本/ }));
    await screen.findByText('新版本');
    fireEvent.click(screen.getByRole('button', { name: '环境' }));
    fireEvent.click(await screen.findByRole('button', { name: /选择并探测 Python/ }));
    await screen.findByText(environment.engineSourceHash);
    fireEvent.click(screen.getByRole('button', { name: /预检并准备运行/ }));
    expect(client.startRun).not.toHaveBeenCalled();
    await screen.findByText('预检就绪，可提交冻结运行。');
    expect(screen.getByText('300 秒 / 64 MiB 输出 / 100000 总采样点')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: '启动本地计算' }));
    await waitFor(() => expect(client.startRun).toHaveBeenCalled());
    fireEvent.click(await screen.findByRole('button', { name: /取消真实计算/ }));
    await screen.findByRole('button', { name: /等待执行端退出/ });
    expect(client.cancelRun).toHaveBeenCalledWith(project.project.id, run.id);
  }, 60000);
  it('保存失败显示宿主错误且命令搜索只执行用户所选动作', async () => {
    const client = makeClient(); client.saveModel.mockRejectedValue({ code: 'disk_full', message: '磁盘已满' });
    render(<Workbench client={client} dialogs={dialogs} />);
    fireEvent.keyDown(window, { key: 'k', ctrlKey: true });
    fireEvent.change(screen.getByLabelText('搜索命令'), { target: { value: '打开项目' } });
    fireEvent.click(within(screen.getByRole('dialog', { name: '命令搜索' })).getByRole('button', { name: '打开项目' }));
    await screen.findByText(model.label);
    fireEvent.click(screen.getByRole('button', { name: /保存版本/ }));
    expect(await screen.findByText('磁盘已满')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: /还原草稿/ }));
    fireEvent.click(screen.getByRole('button', { name: '模型' }));
  }, 20000);
  it('命令入口复用真实保存、探测、预检、查询和导出操作', async () => {
    const client = makeClient();
    client.prepareRun.mockResolvedValue({ ...preflight, modelVersionId: 'model-2' });
    render(<Workbench client={client} dialogs={dialogs} />);
    fireEvent.click(screen.getByRole('button', { name: /打开项目/ }));
    await screen.findByText(model.label);
    const command = (name: string) => {
      fireEvent.keyDown(window, { key: 'k', ctrlKey: true });
      fireEvent.change(screen.getByLabelText('搜索命令'), { target: { value: name } });
      fireEvent.click(within(screen.getByRole('dialog', { name: '命令搜索' })).getByRole('button', { name }));
    };
    command('选择 Python 环境');
    await waitFor(() => expect(client.probeEnvironment).toHaveBeenCalled());
    command('保存模型版本');
    await waitFor(() => expect(client.saveModel).toHaveBeenCalled());
    command('预检当前模型');
    await screen.findByText('预检就绪，可提交冻结运行。');
    fireEvent.click(screen.getByText('返回修改'));
    command('查看运行记录');
    fireEvent.click(within(await screen.findByRole('dialog', { name: '运行记录' })).getByRole('button', { name: '关闭' }));
    command('查看原始样本');
    fireEvent.click(within(await screen.findByRole('dialog', { name: '原始轨迹样本' })).getByRole('button', { name: '关闭' }));
    command('导出运行 JSON 记录');
    await waitFor(() => expect(client.exportRun).toHaveBeenCalled());
    command('创建项目');
    expect(await screen.findByLabelText('新项目名称')).toBeInTheDocument();
    expect(client.createProject).not.toHaveBeenCalled();
  }, 60000);
});
