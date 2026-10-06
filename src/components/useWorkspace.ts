import { useEffect, useReducer, useRef, useState } from 'react';
import { configSchema, configsEqual, validateProjectName } from '../domain/model';
import { initialSession, isActiveRun, sessionReducer } from '../domain/session';
import { desktopClient, fileDialogs, formatError, type DesktopClient, type FileDialogs } from '../services/desktop';

export const POLL_INTERVAL_MS = 1000;
// 操作锁阻止重复提交，代数标识阻止旧项目请求在切换后覆盖当前界面。
export function useWorkspace(client: DesktopClient = desktopClient, dialogs: FileDialogs = fileDialogs) {
  const [state, dispatch] = useReducer(sessionReducer, initialSession);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const operationLock = useRef(false);
  const generation = useRef(0);
  const alive = useRef(true);
  const current = useRef(state);
  current.current = state;
  useEffect(() => { alive.current = true; return () => { alive.current = false; generation.current += 1; }; }, []);

  async function perform(action: () => Promise<void>) {
    if (operationLock.current) return;
    operationLock.current = true;
    setBusy(true); setError(null); setNotice(null);
    try { await action(); }
    catch (failure) { if (alive.current) setError(formatError(failure)); }
    finally { operationLock.current = false; if (alive.current) setBusy(false); }
  }
  const requireProject = () => {
    if (!state.project) throw { code: 'project_required', message: '请先创建或打开项目。' };
    return state.project.project.id;
  };
  const selectedRun = state.project?.runs.find((run) => run.id === state.selectedRunId) ?? null;
  const selectedModel = state.project?.models.find((model) => model.id === state.selectedModelId) ?? null;
  const dirty = !selectedModel || !configsEqual(selectedModel.config, state.draft);

  // 轮询不重叠；未知状态保留人工核实入口，不自动重提运行。
  useEffect(() => {
    const projectId = state.project?.project.id;
    const activeIds = state.project?.runs.filter((run) => isActiveRun(run.state)).map((run) => run.id) ?? [];
    if (!projectId || activeIds.length === 0) return;
    const revision = generation.current;
    let stopped = false;
    let timer: ReturnType<typeof setTimeout>;
    async function poll() {
      try {
        for (const runId of activeIds) {
          const record = await client.getRun(projectId!, runId);
          if (stopped || revision !== generation.current) return;
          dispatch({ type: 'runReceived', run: record });
        }
      } catch (failure) {
        if (!stopped) setError(`运行状态刷新失败：${formatError(failure)}`);
      }
      if (!stopped) timer = setTimeout(() => void poll(), POLL_INTERVAL_MS);
    }
    timer = setTimeout(() => void poll(), POLL_INTERVAL_MS);
    return () => { stopped = true; clearTimeout(timer); };
  }, [client, state.project?.project.id, state.project?.runs.map((run) => `${run.id}:${run.state}`).join('|')]);

  return {
    state, dispatch, busy, error, notice, selectedRun, selectedModel, dirty,
    clearError: () => setError(null),
    createProject: (name: string) => perform(async () => {
      const invalid = validateProjectName(name);
      if (invalid) throw { code: 'invalid_name', message: invalid };
      const directory = await dialogs.directory();
      if (!directory) return;
      const project = await client.createProject(directory, name);
      generation.current += 1;
      dispatch({ type: 'projectLoaded', project });
      setNotice('项目已创建，研究数据保存在所选本地目录。');
    }),
    openProject: () => perform(async () => {
      const directory = await dialogs.directory();
      if (!directory) return;
      const project = await client.openProject(directory);
      generation.current += 1;
      dispatch({ type: 'projectLoaded', project });
    }),
    saveModel: (label: string) => perform(async () => {
      const projectId = requireProject();
      const parsed = configSchema.safeParse(state.draft);
      if (!parsed.success) throw { code: 'invalid_config', message: '模型参数无效，请检查正数、采样范围与冲量参数条件。' };
      if (!label.trim()) throw { code: 'invalid_label', message: '请填写模型版本名称。' };
      const model = await client.saveModel(projectId, label.trim(), parsed.data);
      dispatch({ type: 'modelSaved', model });
      setNotice('已追加保存模型版本，历史版本保留。');
    }),
    choosePython: () => perform(async () => {
      const path = await dialogs.python();
      if (!path) return;
      dispatch({ type: 'pythonChanged', path });
      const environment = await client.probeEnvironment(path);
      dispatch({ type: 'environmentReceived', environment });
      setNotice('已读取实际 Python、科学依赖和引擎身份。');
    }),
    prepareRun: () => perform(async () => {
      const projectId = requireProject();
      if (!selectedModel || dirty) throw { code: 'unsaved_model', message: '请先保存当前模型版本，再执行预检。' };
      if (!state.environment) throw { code: 'environment_required', message: '请先选择并探测 Python 环境。' };
      dispatch({ type: 'preflightCleared' });
      const preflight = await client.prepareRun(projectId, selectedModel.id, state.pythonExecutable);
      if (configsEqual(current.current.draft, preflight.config) && current.current.pythonExecutable === state.pythonExecutable
        && current.current.selectedModelId === preflight.modelVersionId) {
        dispatch({ type: 'preflightReceived', preflight });
      }
    }),
    startRun: () => perform(async () => {
      const projectId = requireProject();
      if (!state.preflight || state.preflight.status !== 'ready' || dirty) throw { code: 'preflight_required', message: '请完成当前版本的预检后再启动。' };
      const id = state.preflight.id;
      // 发出一次提交后即消费界面预检，响应丢失时只能查询，不能自动重试。
      dispatch({ type: 'preflightCleared' });
      const run = await client.startRun(projectId, id);
      dispatch({ type: 'runReceived', run });
      dispatch({ type: 'runSelected', id: run.id });
    }),
    cancelRun: () => perform(async () => {
      const projectId = requireProject();
      if (!selectedRun) throw { code: 'run_required', message: '请先选择运行。' };
      const run = await client.cancelRun(projectId, selectedRun.id);
      dispatch({ type: 'runReceived', run });
    }),
    refresh: () => perform(async () => {
      const projectId = requireProject();
      const project = await client.getProject(projectId);
      for (const run of project.runs) dispatch({ type: 'runReceived', run });
    }),
    exportRun: () => perform(async () => {
      const projectId = requireProject();
      if (!selectedRun) throw { code: 'run_required', message: '请先选择要导出的运行。' };
      const directory = await dialogs.directory();
      if (!directory) return;
      const record = await client.exportRun(projectId, selectedRun.id, directory);
      setNotice(`研究记录已导出：${record.path} · SHA-256 ${record.sha256}`);
    }),
  };
}
