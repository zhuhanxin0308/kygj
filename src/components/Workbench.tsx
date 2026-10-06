import { lazy, Suspense, useEffect, useMemo, useState } from 'react';
import {
  Alert, Button, ConfigProvider, Descriptions, Empty, Input, Modal, Select, Slider, Space, Table, Tabs, Tag, Tooltip, Typography, theme,
} from 'antd';
import zhCN from 'antd/locale/zh_CN';
import {
  ApiOutlined, ApartmentOutlined, CheckCircleOutlined, DatabaseOutlined, ExperimentOutlined, ExportOutlined,
  FolderOpenOutlined, FolderOutlined, FunctionOutlined, PauseOutlined, PlayCircleOutlined, PlusOutlined,
  ReloadOutlined, SaveOutlined, SearchOutlined, StepBackwardOutlined, StepForwardOutlined, StopOutlined, SwapOutlined,
} from '@ant-design/icons';
import { configSchema } from '../domain/model';
import { advanceTimeline, DISPLAY_LIMITS, sampleAtAffine, timeBounds } from '../domain/geometry';
import { isActiveRun, RUN_LABELS, VALIDATION_LABELS } from '../domain/session';
import { desktopClient, fileDialogs, type DesktopClient, type FileDialogs } from '../services/desktop';
import type { ChartMode } from '../visualization/AnalysisChart';
import { RAY_COLORS } from '../visualization/palette';
import { ConfigEditor } from './ConfigEditor';
import { EnvironmentDetails, RecordInspector } from './RecordInspector';
import { useWorkspace } from './useWorkspace';

// 大型可视化依赖独立加载，项目表单和宿主操作无需等待其解析完成。
const ResearchScene = lazy(() => import('../visualization/ResearchScene').then((module) => ({ default: module.ResearchScene })));
const AnalysisChart = lazy(() => import('../visualization/AnalysisChart').then((module) => ({ default: module.AnalysisChart })));

const THEME = {
  algorithm: theme.darkAlgorithm,
  token: { colorPrimary: '#3EDCFF', colorInfo: '#3EDCFF', colorSuccess: '#36D6B0', colorWarning: '#FFB65C', colorError: '#FF6B7A', colorBgBase: '#090E17', colorBgContainer: '#111A29', colorBgElevated: '#17243A', colorText: '#EDF4FF', colorTextSecondary: '#A6B7CF', colorBorder: '#263a51', borderRadius: 6, fontFamily: 'Inter, Segoe UI, Microsoft YaHei, sans-serif', fontSize: 13, controlHeight: 32 },
};
interface Props { client?: DesktopClient; dialogs?: FileDialogs }

// 顶层只装配真实项目操作和视图；未实现领域能力保留固定导航并明确禁用。
export function Workbench({ client = desktopClient, dialogs = fileDialogs }: Props) {
  const workspace = useWorkspace(client, dialogs);
  const { state, selectedRun, selectedModel, dirty, busy } = workspace;
  const [inspector, setInspector] = useState('parameters');
  const [createOpen, setCreateOpen] = useState(false);
  const [projectName, setProjectName] = useState('Ellis 光传播研究');
  const [versionLabel, setVersionLabel] = useState('Ellis 模型版本');
  const [commandOpen, setCommandOpen] = useState(false);
  const [commandQuery, setCommandQuery] = useState('');
  const [runsOpen, setRunsOpen] = useState(false);
  const [dataOpen, setDataOpen] = useState(false);
  const [preflightOpen, setPreflightOpen] = useState(false);
  const [selectedRay, setSelectedRay] = useState(0);
  const [affine, setAffine] = useState(0);
  const [playing, setPlaying] = useState(false);
  const [rate, setRate] = useState(1);
  const [chartMode, setChartMode] = useState<ChartMode>('radius');
  const [previewDraft, setPreviewDraft] = useState(false);
  const [resourceQuery, setResourceQuery] = useState('');
  const [inputValid, setInputValid] = useState(true);
  const available = client.available();
  const result = previewDraft ? null : selectedRun?.result ?? null;
  const config = result?.config ?? (configSchema.safeParse(state.draft).success ? state.draft : null);
  const trajectory = result?.trajectories[selectedRay] ?? null;
  const bounds = useMemo(() => timeBounds(result?.trajectories.map((ray) => ray.samples) ?? []), [result]);
  const cursor = trajectory ? sampleAtAffine(trajectory.samples, affine) : null;
  const activeRuns = state.project?.runs.filter((run) => isActiveRun(run.state)).length ?? 0;
  const selectedModelHash = state.project?.models.find((model) => model.id === selectedRun?.modelVersionId)?.contentHash;

  useEffect(() => { setSelectedRay(0); setPlaying(false); setAffine(bounds[1]); }, [selectedRun?.id, bounds[1], previewDraft]);
  useEffect(() => { if (state.preflight) setPreflightOpen(true); }, [state.preflight]);
  useEffect(() => {
    const handler = (event: KeyboardEvent) => {
      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'k') { event.preventDefault(); setCommandOpen((open) => !open); }
    };
    window.addEventListener('keydown', handler); return () => window.removeEventListener('keydown', handler);
  }, []);
  useEffect(() => {
    if (!playing || bounds[1] === 0) return;
    let previous = performance.now();
    const timer = setInterval(() => {
      const now = performance.now();
      const elapsed = (now - previous) / 1000; previous = now;
      setAffine((current) => {
        const next = advanceTimeline(current, elapsed, bounds[1] / DISPLAY_LIMITS.secondsPerPlayback * rate, bounds[1]);
        if (!next.playing) setPlaying(false);
        return next.affine;
      });
    }, DISPLAY_LIMITS.frameIntervalMs);
    return () => clearInterval(timer);
  }, [playing, rate, bounds]);
  const seek = (value: number) => { setPlaying(false); setAffine(Math.min(bounds[1], Math.max(0, value))); };
  const selectRun = (id: string) => { workspace.dispatch({ type: 'runSelected', id }); setPreviewDraft(false); };
  const play = () => { if (affine >= bounds[1]) setAffine(0); setPlaying(!playing); };
  const commands = [
    { name: '创建项目', disabled: !available || busy, action: () => setCreateOpen(true) },
    { name: '打开项目', disabled: !available || busy, action: () => void workspace.openProject() },
    { name: '选择 Python 环境', disabled: !available || busy, action: () => void workspace.choosePython() },
    { name: '保存模型版本', disabled: !state.project || busy || !inputValid, action: () => void workspace.saveModel(versionLabel) },
    { name: '预检当前模型', disabled: !state.project || busy || !inputValid, action: () => void workspace.prepareRun() },
    { name: '查看运行记录', disabled: !state.project, action: () => setRunsOpen(true) },
    { name: '查看原始样本', disabled: !result, action: () => setDataOpen(true) },
    { name: '导出运行 JSON 记录', disabled: !selectedRun || busy, action: () => void workspace.exportRun() },
  ];
  const navigation = [
    { label: '项目', icon: <FolderOutlined />, action: () => setCreateOpen(true), disabled: !available },
    { label: '模型', icon: <FunctionOutlined />, action: () => setInspector('parameters'), disabled: false },
    { label: '数据', icon: <DatabaseOutlined />, action: () => setDataOpen(true), disabled: !result },
    { label: '工作流（尚未接入）', short: '工作流', icon: <ApartmentOutlined />, disabled: true },
    { label: '运行', icon: <PlayCircleOutlined />, action: () => setRunsOpen(true), disabled: !state.project },
    { label: '验证', icon: <CheckCircleOutlined />, action: () => setInspector('validation'), disabled: false },
    { label: '比较（尚未接入）', short: '比较', icon: <SwapOutlined />, disabled: true },
    { label: '环境', icon: <ApiOutlined />, action: () => setInspector('environment'), disabled: false },
    { label: '交付：导出运行记录', short: '交付', icon: <ExportOutlined />, action: () => void workspace.exportRun(), disabled: !selectedRun || busy },
  ];
  return <ConfigProvider theme={THEME} locale={zhCN}>
    <div className="workbench">
      <header className="topbar">
        <div className="brand"><span className="brand-mark">Λ</span><strong>引力科研工作台</strong></div>
        <span className="workspace-name">{state.project?.project.name ?? '本地研究工作区'}</span>
        <Button className="command-trigger" icon={<SearchOutlined />} onClick={() => setCommandOpen(true)}><span>搜索或执行命令</span><kbd>Ctrl K</kbd></Button>
        <span className="host-status"><span className={`live-dot ${available ? '' : 'muted'}`} />{available ? '本地桌面' : '浏览器预览'}</span>
        <Tag variant="filled">开发中 · 0.1.0</Tag>
      </header>
      <div className="workspace-body">
        <nav className="primary-nav" aria-label="主导航">
          {navigation.map((entry) => <Tooltip placement="right" title={entry.label} key={entry.label}><div className="nav-entry"><Button aria-label={entry.label} type="text" icon={entry.icon} disabled={entry.disabled} onClick={entry.action} /><span>{entry.short ?? entry.label}</span></div></Tooltip>)}
          <div className="nav-bottom"><Tooltip title="核心功能开源永久免费"><span>GPL<br />3.0+</span></Tooltip></div>
        </nav>
        <aside className="resource-panel">
          <div className="panel-heading"><strong>项目资源</strong><Tooltip title="创建项目"><Button aria-label="创建项目" type="text" icon={<PlusOutlined />} disabled={!available || busy} onClick={() => setCreateOpen(true)} /></Tooltip></div>
          <div className="resource-actions"><Button icon={<FolderOpenOutlined />} disabled={!available || busy} onClick={() => void workspace.openProject()}>打开项目</Button>
            <Tooltip title="刷新项目运行"><Button aria-label="刷新项目运行" icon={<ReloadOutlined />} disabled={!state.project || busy} onClick={() => void workspace.refresh()} /></Tooltip></div>
          <Input aria-label="筛选项目资源" prefix={<SearchOutlined />} placeholder="在项目中筛选" value={resourceQuery} onChange={(event) => setResourceQuery(event.target.value)} allowClear />
          <div className="resource-section"><div className="section-label"><FunctionOutlined />模型版本 <span>{state.project?.models.length ?? 0}</span></div>
            {(state.project?.models ?? []).filter((model) => model.label.toLowerCase().includes(resourceQuery.toLowerCase())).map((model) => <button className={`resource-item ${state.selectedModelId === model.id ? 'selected' : ''}`} key={model.id} onClick={() => { workspace.dispatch({ type: 'modelSelected', id: model.id }); setInspector('parameters'); }}><ExperimentOutlined /><span>{model.label}</span></button>)}
            {!state.project?.models.length && <p className="empty-note">编辑参数后保存首个版本。</p>}
          </div>
          <div className="resource-section"><div className="section-label"><PlayCircleOutlined />最近运行 <span>{state.project?.runs.length ?? 0}</span></div>
            {(state.project?.runs ?? []).filter((run) => run.id.includes(resourceQuery)).slice(0, 8).map((run) => <button className={`run-item ${state.selectedRunId === run.id ? 'selected' : ''}`} key={run.id} onClick={() => selectRun(run.id)}><span className="run-short">{run.id.slice(0, 12)}</span><span>{RUN_LABELS[run.state]}</span><small>{VALIDATION_LABELS[run.validationStatus]}</small></button>)}
            {!state.project?.runs.length && <p className="empty-note">尚无运行结果</p>}
          </div>
          <div className="project-location"><span>研究目录</span><code>{state.project?.project.path ?? '尚未打开项目'}</code><small>本地数据 · 无需账号</small></div>
        </aside>
        <main className="research-main">
          {!available && <Alert type="warning" showIcon title="浏览器预览未连接桌面宿主，项目与计算操作不可用。" />}
          {workspace.error && <Alert type="error" showIcon title={workspace.error} closable onClose={workspace.clearError} role="alert" />}
          {workspace.notice && <Alert type="success" showIcon title={workspace.notice} className="operation-notice" />}
          <div className="context-bar"><div><Tag color="cyan">Ellis</Tag><span>{result ? '运行冻结配置' : '当前模型草稿'}</span>{dirty && <Tag color="gold">草稿未保存</Tag>}</div>
            {selectedRun?.result && <Button size="small" onClick={() => setPreviewDraft(!previewDraft)}>{previewDraft ? '查看运行结果' : '预览草稿几何'}</Button>}</div>
          <Suspense fallback={<Alert type="info" title="正在加载三维可视化组件。" />}><ResearchScene config={config} result={result} selected={selectedRay} affine={affine} onSelect={setSelectedRay} /></Suspense>
          <div className="timeline" aria-label="展示时间轴">
            <Tooltip title="回到起点"><Button aria-label="回到轨迹起点" type="text" icon={<StepBackwardOutlined />} disabled={!result} onClick={() => seek(0)} /></Tooltip>
            <Tooltip title={playing ? '暂停展示' : '播放展示'}><Button aria-label={playing ? '暂停展示' : '播放展示'} type="primary" icon={playing ? <PauseOutlined /> : <PlayCircleOutlined />} disabled={!result} onClick={play} /></Tooltip>
            <Tooltip title="前进一个采样间隔"><Button aria-label="前进一个采样间隔" type="text" icon={<StepForwardOutlined />} disabled={!trajectory} onClick={() => seek(affine + bounds[1] / Math.max(1, (trajectory?.samples.length ?? 2) - 1))} /></Tooltip>
            <span className="timeline-label">展示 λ <b>{affine.toFixed(3)}</b> / {bounds[1].toFixed(3)}</span>
            <Slider aria-label="展示仿射参数" min={0} max={bounds[1] || 1} step={Math.max(bounds[1] / 10000, Number.EPSILON)} value={affine} disabled={!result} onChange={seek} tooltip={{ formatter: (value) => `λ = ${value?.toFixed(5)}` }} />
            <Select aria-label="展示速度" value={rate} onChange={setRate} options={[0.5, 1, 2, 4].map((value) => ({ value, label: `${value}×` }))} />
          </div>
          {result && <div className="ray-selector"><span>光线选择</span>{result.trajectories.map((ray, index) => <Button key={index} size="small" type={selectedRay === index ? 'primary' : 'text'} onClick={() => setSelectedRay(index)}><i style={{ background: RAY_COLORS[index % RAY_COLORS.length] }} />b = {ray.impactParameter}</Button>)}
            <Button size="small" icon={<DatabaseOutlined />} onClick={() => setDataOpen(true)}>原始样本</Button></div>}
          <Suspense fallback={<Alert type="info" title="正在加载分析图组件。" />}><AnalysisChart result={result} selected={selectedRay} affine={affine} mode={chartMode} onMode={setChartMode} onSelect={setSelectedRay} onSeek={seek} /></Suspense>
        </main>
        <aside className="inspector-panel">
          <div className="panel-heading"><strong>上下文检查器</strong><Tag color={dirty ? 'gold' : 'cyan'}>{dirty ? '草稿' : '已保存'}</Tag></div>
          <Tabs activeKey={inspector} onChange={setInspector} size="small" items={[
            { key: 'parameters', label: '参数', children: <>
              <ConfigEditor config={state.draft} disabled={busy} onValidityChange={setInputValid} onChange={(patch) => workspace.dispatch({ type: 'draftChanged', patch })} />
              <Input aria-label="模型版本名称" placeholder="模型版本名称" value={versionLabel} onChange={(event) => setVersionLabel(event.target.value)} disabled={busy} />
              <Space className="inspector-actions"><Button icon={<SaveOutlined />} disabled={!state.project || busy || !inputValid} onClick={() => void workspace.saveModel(versionLabel)}>保存版本</Button><Button icon={<ReloadOutlined />} disabled={!selectedModel || busy} onClick={() => selectedModel && workspace.dispatch({ type: 'modelSelected', id: selectedModel.id })}>还原草稿</Button></Space>
              <Alert type="info" showIcon title="先保存模型与选择环境，再预检运行。" />
            </> },
            { key: 'validation', label: '验证', children: <RecordInspector run={selectedRun} trajectory={trajectory} onSeek={seek} /> },
            { key: 'source', label: '来源', children: selectedRun ? <>
              <Descriptions column={1} size="small" items={[
                { key: 'run', label: '运行', children: <code className="break-all">{selectedRun.id}</code> },
                { key: 'model', label: '模型版本', children: <code className="break-all">{selectedRun.modelVersionId}</code> },
                { key: 'hash', label: '内容 SHA-256', children: <code className="break-all">{selectedModelHash ?? '模型版本身份缺失'}</code> },
                { key: 'physical', label: '物理约定', children: 'G=c=1，(−+++)，E=1；赤道零测地线' },
              ]} /><Typography.Title level={5}>冻结输入</Typography.Title><pre className="source-json">{JSON.stringify(selectedRun.request.config, null, 2)}</pre>
              <EnvironmentDetails environment={selectedRun.environment} /><Button block icon={<ExportOutlined />} disabled={busy} onClick={() => void workspace.exportRun()}>导出 JSON 研究记录</Button>
            </> : <Empty image={Empty.PRESENTED_IMAGE_SIMPLE} description="选择运行后检查冻结输入与来源。" /> },
            { key: 'environment', label: '环境', children: <><Button block icon={<ApiOutlined />} disabled={!available || busy} onClick={() => void workspace.choosePython()}>选择并探测 Python</Button><EnvironmentDetails environment={state.environment} /><p className="empty-note">选择已安装本产品 gravity_engine 的 Python。此操作只读取实际版本，不自动安装依赖。</p></> },
          ]} />
          <div className="inspector-footer">
            {cursor && <div className="cursor-readout"><span>当前光标（插值）</span><code>l = {cursor.l.toPrecision(6)}<br />φ = {cursor.phi.toPrecision(6)} rad</code></div>}
            <Button type="primary" size="large" block icon={<PlayCircleOutlined />} disabled={!available || !state.project || busy || !inputValid} loading={busy} onClick={() => void workspace.prepareRun()}>预检并准备运行</Button>
            {selectedRun && isActiveRun(selectedRun.state) && <Button danger block icon={<StopOutlined />} disabled={busy || selectedRun.state === 'cancelling'} onClick={() => void workspace.cancelRun()}>{selectedRun.state === 'cancelling' ? '等待执行端退出' : '取消真实计算'}</Button>}
          </div>
        </aside>
      </div>
      <footer className="statusbar"><span><span className={`live-dot ${available ? '' : 'muted'}`} />{busy ? '正在处理操作' : available ? '桌面宿主已连接' : '仅公式预览'}</span><span>执行 <b>{activeRuns}</b> 项活动</span><span>验证 {selectedRun ? VALIDATION_LABELS[selectedRun.validationStatus] : '尚未检查'}</span><span>复核 未记录</span><span className="status-end">{result ? `${result.trajectories.length} 条真实轨迹` : '无计算产物'} · {state.environment ? `Python ${state.environment.pythonVersion}` : '环境未探测'}</span></footer>
      <Modal title="创建本地研究项目" open={createOpen} onCancel={() => setCreateOpen(false)} okText="选择目录并创建" cancelText="取消" confirmLoading={busy} okButtonProps={{ disabled: !available }} onOk={() => { void workspace.createProject(projectName).then(() => setCreateOpen(false)); }}>
        <Typography.Paragraph>在所选父目录中创建项目子目录。已有目录不会被覆盖。</Typography.Paragraph><Input aria-label="新项目名称" value={projectName} onChange={(event) => setProjectName(event.target.value)} />
      </Modal>
      <Modal title="命令搜索" aria-label="命令搜索" open={commandOpen} onCancel={() => setCommandOpen(false)} footer={null}>
        <Input aria-label="搜索命令" autoFocus prefix={<SearchOutlined />} value={commandQuery} onChange={(event) => setCommandQuery(event.target.value)} placeholder="输入项目、环境、预检或导出" />
        <div className="command-list">{commands.filter((command) => command.name.includes(commandQuery)).map((command) => <Button block type="text" key={command.name} disabled={command.disabled} onClick={() => { setCommandOpen(false); command.action(); }}>{command.name}</Button>)}</div>
      </Modal>
      <Modal title="运行记录" aria-label="运行记录" open={runsOpen} onCancel={() => setRunsOpen(false)} footer={null} width={850}>
        <Table rowKey="id" size="small" dataSource={state.project?.runs ?? []} pagination={{ pageSize: 10 }} columns={[
          { title: '运行身份', dataIndex: 'id', render: (id: string) => <Button type="link" onClick={() => { selectRun(id); setRunsOpen(false); }}>{id.slice(0, 16)}</Button> },
          { title: '执行', dataIndex: 'state', render: (value: keyof typeof RUN_LABELS) => RUN_LABELS[value] },
          { title: '数值验证', dataIndex: 'validationStatus', render: (value: keyof typeof VALIDATION_LABELS) => VALIDATION_LABELS[value] },
          { title: '创建时间', dataIndex: 'createdAt' },
        ]} />
      </Modal>
      <Modal title="原始轨迹样本" aria-label="原始轨迹样本" open={dataOpen} onCancel={() => setDataOpen(false)} footer={null} width={1100}>
        {trajectory ? <><Typography.Paragraph>冻结运行 {selectedRun?.id} · b = {trajectory.impactParameter} · 原始样本 {trajectory.samples.length} 点</Typography.Paragraph>
          <Table rowKey="affine" size="small" dataSource={trajectory.samples} pagination={{ pageSize: 20, showSizeChanger: false }} scroll={{ x: 900 }} onRow={(sample) => ({ onClick: () => { seek(sample.affine); setDataOpen(false); } })} columns={['affine', 't', 'l', 'theta', 'phi', 'kt', 'kl', 'kTheta', 'kPhi'].map((key) => ({ title: key === 'affine' ? 'λ' : key, dataIndex: key, render: (value: number) => <span className="numeric">{value.toPrecision(8)}</span> }))} /></> : <Empty description="当前没有真实轨迹数据。" />}
      </Modal>
      <Modal title="本地运行预检" open={preflightOpen} onCancel={() => setPreflightOpen(false)} okText="启动本地计算" cancelText="返回修改" confirmLoading={busy} okButtonProps={{ disabled: state.preflight?.status !== 'ready' || busy }} onOk={() => { void workspace.startRun().then(() => setPreflightOpen(false)); }}>
        {state.preflight && <><Alert showIcon type={state.preflight.status === 'ready' ? 'success' : 'error'} title={state.preflight.status === 'ready' ? '预检就绪，可提交冻结运行。' : '预检阻断，尚未启动计算。'} />
          <Descriptions column={1} size="small" items={[
            { key: 'model', label: '模型版本', children: state.preflight.modelVersionId },
            { key: 'rays', label: '光线 / 每条样本', children: `${state.preflight.config.impactParameters.length} / ${state.preflight.config.sampleCount}` },
            { key: 'affine', label: '仿射参数预算', children: state.preflight.config.maxAffineParameter },
            { key: 'execution', label: '计算位置', children: '当前电脑 · 独立 Python 进程' },
            { key: 'limits', label: '实际执行限额', children: `${state.preflight.executionLimits.maxWallTimeSeconds} 秒 / ${state.preflight.executionLimits.maxOutputBytes / (1024 * 1024)} MiB 输出 / ${state.preflight.executionLimits.maxTotalSamples} 总采样点` },
          ]} /><EnvironmentDetails environment={state.preflight.environment} />
          {state.preflight.issues.map((issue) => <Alert key={issue.code} type="error" title={issue.message} />)}
          <Typography.Paragraph type="secondary">预检通过只说明输入与环境可执行；数值检查和人工复核在计算后分别记录。</Typography.Paragraph>
        </>}
      </Modal>
    </div>
  </ConfigProvider>;
}
