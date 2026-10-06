import type { EnvironmentInfo, ModelVersion, PreflightReport, ProjectState, RunRecord } from './contracts';
import { DEFAULT_CONFIG, type EllisConfig } from './model.ts';

// 已保存版本、可编辑草稿、运行快照三者分别维护，编辑不得污染旧结果。
export interface SessionState {
  project: ProjectState | null;
  selectedModelId: string | null;
  selectedRunId: string | null;
  draft: EllisConfig;
  pythonExecutable: string;
  environment: EnvironmentInfo | null;
  preflight: PreflightReport | null;
}
export const initialSession: SessionState = {
  project: null, selectedModelId: null, selectedRunId: null, draft: DEFAULT_CONFIG,
  pythonExecutable: '', environment: null, preflight: null,
};
export type SessionAction =
  | { type: 'projectLoaded'; project: ProjectState }
  | { type: 'draftChanged'; patch: Partial<EllisConfig> }
  | { type: 'modelSaved'; model: ModelVersion }
  | { type: 'modelSelected'; id: string }
  | { type: 'runSelected'; id: string }
  | { type: 'pythonChanged'; path: string }
  | { type: 'environmentReceived'; environment: EnvironmentInfo }
  | { type: 'preflightReceived'; preflight: PreflightReport }
  | { type: 'preflightCleared' }
  | { type: 'runReceived'; run: RunRecord };

export function sessionReducer(state: SessionState, action: SessionAction): SessionState {
  switch (action.type) {
    case 'projectLoaded': return {
      ...initialSession, project: action.project, selectedModelId: action.project.models[0]?.id ?? null,
      selectedRunId: action.project.runs[0]?.id ?? null, draft: action.project.models[0]?.config ?? DEFAULT_CONFIG,
      pythonExecutable: state.pythonExecutable, environment: state.environment,
    };
    case 'draftChanged': return { ...state, draft: { ...state.draft, ...action.patch }, preflight: null };
    case 'pythonChanged': return { ...state, pythonExecutable: action.path, environment: null, preflight: null };
    case 'environmentReceived': return { ...state, environment: action.environment, pythonExecutable: action.environment.pythonExecutable, preflight: null };
    case 'preflightReceived': return action.preflight.projectId === state.project?.project.id
      && state.project.models.some((model) => model.id === action.preflight.modelVersionId)
      ? { ...state, preflight: action.preflight } : state;
    case 'preflightCleared': return { ...state, preflight: null };
    case 'modelSaved': return state.project && action.model.projectId === state.project.project.id ? {
      ...state, project: { ...state.project, models: [action.model, ...state.project.models] },
      selectedModelId: action.model.id, draft: action.model.config, preflight: null,
    } : state;
    case 'modelSelected': {
      const model = state.project?.models.find((item) => item.id === action.id);
      return model ? { ...state, selectedModelId: model.id, draft: model.config, preflight: null } : state;
    }
    case 'runSelected': return state.project?.runs.some((run) => run.id === action.id)
      ? { ...state, selectedRunId: action.id } : state;
    case 'runReceived': {
      if (!state.project || action.run.projectId !== state.project.project.id) return state;
      const previous = state.project.runs.find((run) => run.id === action.run.id);
      // 取消与完成可能和正在传输的轮询竞争，迟到响应不能倒退已确认的执行阶段。
      if (previous && previous.state !== action.run.state) {
        const terminal = ['completed', 'failed', 'cancelled'].includes(previous.state);
        const cancelRegression = previous.state === 'cancelling' && ['queued', 'running'].includes(action.run.state);
        const queueRegression = previous.state === 'running' && action.run.state === 'queued';
        if (terminal || cancelRegression || queueRegression) return state;
      }
      const exists = state.project.runs.some((run) => run.id === action.run.id);
      const runs = exists ? state.project.runs.map((run) => run.id === action.run.id ? action.run : run)
        : [action.run, ...state.project.runs];
      return { ...state, project: { ...state.project, runs }, selectedRunId: state.selectedRunId ?? action.run.id };
    }
  }
}

export const isActiveRun = (state: RunRecord['state']) => ['queued', 'running', 'cancelling'].includes(state);
export const canStart = (preflight: PreflightReport | null, busy: boolean) => preflight?.status === 'ready' && !busy;
export const RUN_LABELS: Record<RunRecord['state'], string> = {
  queued: '已排队', running: '正在计算', completed: '执行完成', failed: '执行失败',
  cancelling: '正在取消，等待退出', cancelled: '已取消', unknown: '状态待核实',
};
export const VALIDATION_LABELS: Record<RunRecord['validationStatus'], string> = {
  not_run: '尚未检查', passed: '检查通过', failed: '检查未通过', inconclusive: '证据不足',
};
export const EVENT_LABELS = { throat: '穿过喉部', turning: '径向转向', exit_negative: '到达负侧边界', return_positive: '返回正侧边界' };
