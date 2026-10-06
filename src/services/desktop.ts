import { invoke, isTauri } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import { z } from 'zod';
import { configSchema, type EllisConfig } from '../domain/model';
import {
  environmentSchema, exportSchema, modelVersionSchema, preflightSchema, projectStateSchema, runRecordSchema,
  type EnvironmentInfo, type ExportRecord, type ModelVersion, type PreflightReport, type ProjectState, type RunRecord,
} from '../domain/contracts';

export interface DesktopClient {
  available(): boolean;
  createProject(parentDirectory: string, name: string): Promise<ProjectState>;
  openProject(directory: string): Promise<ProjectState>;
  getProject(projectId: string): Promise<ProjectState>;
  saveModel(projectId: string, label: string, config: EllisConfig): Promise<ModelVersion>;
  probeEnvironment(pythonExecutable: string): Promise<EnvironmentInfo>;
  prepareRun(projectId: string, modelVersionId: string, pythonExecutable: string): Promise<PreflightReport>;
  startRun(projectId: string, preflightId: string): Promise<RunRecord>;
  getRun(projectId: string, runId: string): Promise<RunRecord>;
  cancelRun(projectId: string, runId: string): Promise<RunRecord>;
  exportRun(projectId: string, runId: string, destinationDirectory: string): Promise<ExportRecord>;
}
export interface FileDialogs { directory(): Promise<string | null>; python(): Promise<string | null> }
type Invoke = (command: string, args: { request: Record<string, unknown> }) => Promise<unknown>;
export function formatError(error: unknown): string {
  if (error && typeof error === 'object' && 'code' in error && 'message' in error && typeof error.message === 'string') return error.message;
  return '操作未完成，请重试或检查本地环境。';
}

// 所有 IPC 都通过单一校验入口，浏览器预览绝不回退到本地假数据。
export function createDesktopClient(transport: Invoke, available: () => boolean): DesktopClient {
  async function call<T>(command: string, request: Record<string, unknown>, schema: z.ZodType<T>): Promise<T> {
    if (!available()) throw { code: 'host_unavailable', message: '浏览器预览未连接桌面宿主，无法访问项目或执行计算。' };
    const response = await transport(command, { request });
    const parsed = schema.safeParse(response);
    if (!parsed.success) throw { code: 'invalid_response', message: '桌面宿主返回的数据未通过协议校验，本次结果未被接受。' };
    const identity = parsed.data as { projectId?: string; project?: { id: string }; id?: string; modelVersionId?: string };
    const responseProject = identity.projectId ?? identity.project?.id;
    const projectMismatch = request.projectId !== undefined && responseProject !== undefined && responseProject !== request.projectId;
    const runMismatch = request.runId !== undefined && command !== 'export_run' && identity.id !== request.runId;
    const modelMismatch = request.modelVersionId !== undefined && identity.modelVersionId !== request.modelVersionId;
    if (projectMismatch || runMismatch || modelMismatch) {
      throw { code: 'invalid_response', message: '桌面宿主回复的项目、模型或运行身份与请求不符，本次结果未被接受。' };
    }
    return parsed.data;
  }
  return {
    available,
    createProject: (parentDirectory, name) => call('create_project', { parentDirectory, name }, projectStateSchema),
    openProject: (directory) => call('open_project', { directory }, projectStateSchema),
    getProject: (projectId) => call('get_project', { projectId }, projectStateSchema),
    saveModel: async (projectId, label, config) => call('save_model', { projectId, label, config: configSchema.parse(config) }, modelVersionSchema),
    probeEnvironment: (pythonExecutable) => call('probe_environment', { pythonExecutable }, environmentSchema),
    prepareRun: (projectId, modelVersionId, pythonExecutable) => call('prepare_run', { projectId, modelVersionId, pythonExecutable }, preflightSchema),
    startRun: (projectId, preflightId) => call('start_run', { projectId, preflightId }, runRecordSchema),
    getRun: (projectId, runId) => call('get_run', { projectId, runId }, runRecordSchema),
    cancelRun: (projectId, runId) => call('cancel_run', { projectId, runId }, runRecordSchema),
    exportRun: (projectId, runId, destinationDirectory) => call('export_run', { projectId, runId, destinationDirectory }, exportSchema),
  };
}
export const desktopClient = createDesktopClient(invoke, isTauri);
export const fileDialogs: FileDialogs = {
  async directory() {
    const selected = await open({ directory: true, multiple: false, title: '选择研究目录' });
    return typeof selected === 'string' ? selected : null;
  },
  async python() {
    const selected = await open({ directory: false, multiple: false, title: '选择已安装的 Python 可执行文件' });
    return typeof selected === 'string' ? selected : null;
  },
};
