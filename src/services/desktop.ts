import { invoke, isTauri } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import { z } from 'zod';
import { configSchema, type EllisConfig } from '../domain/model';
import {
  environmentSchema, exportSchema, modelVersionSchema, preflightSchema, projectStateSchema, runRecordSchema,
  type EnvironmentInfo, type ExportRecord, type ModelVersion, type PreflightReport, type ProjectState, type RunRecord,
} from '../domain/contracts';
import {
  migrationPlanSchema, migrationReceiptSchema, verificationDraftSchema, verificationExecutionSchema,
  verificationRecordPageSchema, verificationRecordSchema, verificationRulePageSchema, verificationRuleSchema, verificationStateSchema,
  VERIFICATION_PAGE_LIMIT, type ProjectMigrationPlan, type ProjectMigrationReceipt, type VerificationExecution,
  type VerificationRecord, type VerificationRecordPage, type VerificationRuleDraft, type VerificationRulePage,
  type VerificationRuleVersion, type RunVerificationState,
} from '../domain/verification';

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
  listVerificationRules(projectId: string, offset: number, limit: number): Promise<VerificationRulePage>;
  saveVerificationRuleVersion(projectId: string, draft: VerificationRuleDraft): Promise<VerificationRuleVersion>;
  getRunVerificationState(projectId: string, runId: string, ruleVersionId: string): Promise<RunVerificationState>;
  executeVerification(projectId: string, request: VerificationExecution): Promise<VerificationRecord>;
  listVerificationRecords(projectId: string, runId: string, offset: number, limit: number): Promise<VerificationRecordPage>;
  getVerificationRecord(projectId: string, recordId: string): Promise<VerificationRecord>;
  prepareProjectMigration(directory: string): Promise<ProjectMigrationPlan>;
  applyProjectMigration(directory: string, planId: string): Promise<ProjectMigrationReceipt>;
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
    const identity = parsed.data as { projectId?: string; project?: { id: string }; id?: string; modelVersionId?: string; runId?: string; ruleVersionId?: string; planId?: string };
    const responseProject = identity.projectId ?? identity.project?.id;
    const projectMismatch = request.projectId !== undefined && responseProject !== undefined && responseProject !== request.projectId;
    const runMismatch = request.runId !== undefined && ['get_run', 'cancel_run'].includes(command) && identity.id !== request.runId;
    const modelMismatch = request.modelVersionId !== undefined && identity.modelVersionId !== request.modelVersionId;
    const verificationMismatch = request.runId !== undefined && command === 'get_run_verification_state' && identity.runId !== request.runId;
    const ruleMismatch = request.ruleVersionId !== undefined && identity.ruleVersionId !== request.ruleVersionId;
    const recordMismatch = request.recordId !== undefined && identity.id !== request.recordId;
    const planMismatch = request.planId !== undefined && identity.planId !== request.planId;
    if (projectMismatch || runMismatch || modelMismatch || verificationMismatch || ruleMismatch || recordMismatch || planMismatch) {
      throw { code: 'invalid_response', message: '桌面宿主回复的项目、模型或运行身份与请求不符，本次结果未被接受。' };
    }
    return parsed.data;
  }
  const rejectIdentity = () => { throw { code: 'invalid_response', message: '独立验证的项目、运行、规则、分页或请求来源不符，本次回复未被接受。' }; };
  const pagination = (offset: number, limit: number) => z.strictObject({ offset: z.number().int().nonnegative(), limit: z.number().int().min(1).max(VERIFICATION_PAGE_LIMIT) }).parse({ offset, limit });
  const validPage = (offset: number, limit: number, size: number, total: number, nextOffset: number | null) =>
    size <= limit && total >= size && (nextOffset === null || (nextOffset === offset + size && nextOffset > offset && nextOffset < total));
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
    listVerificationRules: async (projectId, offset, limit) => {
      const page = await call('list_verification_rules', { projectId, ...pagination(offset, limit) }, verificationRulePageSchema);
      if (!page.rules.every((rule) => rule.projectId === projectId)
        || !validPage(offset, limit, page.rules.length, page.total, page.nextOffset)) rejectIdentity();
      return page;
    },
    saveVerificationRuleVersion: async (projectId, draft) => {
      const rule = await call('save_verification_rule_version', { projectId, draft: verificationDraftSchema.parse(draft) }, verificationRuleSchema);
      if (rule.parentVersionId !== draft.baseVersionId || rule.builtin || rule.id === draft.baseVersionId
        || rule.title !== draft.title.trim() || rule.changeReason !== draft.changeReason.trim()
        || rule.checks.length !== draft.thresholds.length || !draft.thresholds.every((value) => rule.checks.some((check) =>
          check.metricId === value.metricId && check.threshold === value.threshold && check.basis === value.basis.trim()))) rejectIdentity();
      return rule;
    },
    getRunVerificationState: async (projectId, runId, ruleVersionId) => {
      const state = await call('get_run_verification_state', { projectId, runId, ruleVersionId }, verificationStateSchema);
      if (state.latestRecord && state.latestRecord.projectId !== projectId) rejectIdentity();
      return state;
    },
    executeVerification: async (projectId, request) => {
      const record = await call('execute_verification', { projectId, request: verificationExecutionSchema.parse(request) }, verificationRecordSchema);
      if (record.runId !== request.runId || record.ruleVersionId !== request.ruleVersionId
        || record.clientRequestId !== request.clientRequestId || record.previousRecordId !== request.previousRecordId) rejectIdentity();
      return record;
    },
    listVerificationRecords: async (projectId, runId, offset, limit) => {
      const page = await call('list_verification_records', { projectId, runId, ...pagination(offset, limit) }, verificationRecordPageSchema);
      if (!page.records.every((record) => record.projectId === projectId && record.runId === runId)
        || !validPage(offset, limit, page.records.length, page.total, page.nextOffset)) rejectIdentity();
      return page;
    },
    getVerificationRecord: (projectId, recordId) => call('get_verification_record', { projectId, recordId }, verificationRecordSchema),
    prepareProjectMigration: (directory) => call('prepare_project_migration', { directory }, migrationPlanSchema),
    applyProjectMigration: (directory, planId) => call('apply_project_migration', { directory, planId }, migrationReceiptSchema),
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
