import { describe, expect, it } from 'vitest';
import { initialSession, sessionReducer, isActiveRun, canStart } from './session';
import { environment, model, preflight, project, run } from '../test/fixtures';

// 先验证不依赖渲染的状态迁移，防止旧项目或预检污染当前任务。
describe('工作区状态边界', () => {
  it('草稿和环境改变使已有预检失效', () => {
    const state = { ...initialSession, preflight: { id: 'old' } as never };
    expect(sessionReducer(state, { type: 'draftChanged', patch: { throatRadius: 2 } }).preflight).toBeNull();
    expect(sessionReducer(state, { type: 'pythonChanged', path: '/python' }).preflight).toBeNull();
  });
  it('未知、终止态不自动重新轮询或重跑', () => {
    expect(isActiveRun('unknown')).toBe(false);
    expect(isActiveRun('completed')).toBe(false);
    expect(isActiveRun('cancelling')).toBe(true);
    expect(canStart(null, false)).toBe(false);
    expect(canStart({ status: 'ready' } as never, true)).toBe(false);
    expect(canStart({ status: 'ready' } as never, false)).toBe(true);
  });
  it('新项目重置视图身份并选取最新真实版本与运行', () => {
    const state = sessionReducer(initialSession, { type: 'projectLoaded', project });
    expect(state.selectedModelId).toBe(model.id);
    expect(state.selectedRunId).toBe(run.id);
    expect(state.draft).toEqual(model.config);
    expect(sessionReducer(state, { type: 'projectLoaded', project: { ...project, models: [], runs: [] } }).selectedRunId).toBeNull();
  });
  it('版本追加不覆盖旧版本，选择版本让预检失效', () => {
    const loaded = sessionReducer(initialSession, { type: 'projectLoaded', project });
    const updated = sessionReducer(loaded, { type: 'modelSaved', model: { ...model, id: 'model-2' } });
    expect(updated.project?.models.map((item) => item.id)).toEqual(['model-2', 'model-1']);
    const selected = sessionReducer({ ...updated, preflight }, { type: 'modelSelected', id: model.id });
    expect(selected.selectedModelId).toBe(model.id);
    expect(selected.preflight).toBeNull();
    expect(sessionReducer(selected, { type: 'modelSelected', id: 'missing' })).toBe(selected);
  });
  it('外项目运行消息不能污染当前项目，真实消息替换同身份记录', () => {
    const loaded = sessionReducer(initialSession, { type: 'projectLoaded', project: { ...project, runs: [{ ...run, state: 'running' }] } });
    expect(sessionReducer(loaded, { type: 'runReceived', run: { ...run, projectId: 'other' } })).toBe(loaded);
    const updated = sessionReducer(loaded, { type: 'runReceived', run: { ...run, state: 'cancelled' } });
    expect(updated.project?.runs).toHaveLength(1);
    expect(updated.project?.runs[0].state).toBe('cancelled');
    expect(sessionReducer(loaded, { type: 'runSelected', id: run.id }).selectedRunId).toBe(run.id);
  });
  it('环境与预检有独立记录，不能由运行结果自动代替', () => {
    const loaded = sessionReducer(initialSession, { type: 'projectLoaded', project });
    const withEnvironment = sessionReducer(loaded, { type: 'environmentReceived', environment });
    expect(withEnvironment.environment).toEqual(environment);
    expect(sessionReducer(withEnvironment, { type: 'preflightReceived', preflight }).preflight).toEqual(preflight);
    expect(sessionReducer({ ...withEnvironment, preflight }, { type: 'preflightCleared' }).preflight).toBeNull();
  });
  it('迟到的轮询不能回退取消中或已终止运行', () => {
    const running = { ...run, state: 'running' as const, result: null };
    const loaded = sessionReducer(initialSession, { type: 'projectLoaded', project: { ...project, runs: [running] } });
    const cancelling = sessionReducer(loaded, { type: 'runReceived', run: { ...running, state: 'cancelling' } });
    expect(sessionReducer(cancelling, { type: 'runReceived', run: running })).toBe(cancelling);
    const completed = sessionReducer(cancelling, { type: 'runReceived', run });
    expect(completed.project?.runs[0].state).toBe('completed');
    expect(sessionReducer(completed, { type: 'runReceived', run: running })).toBe(completed);
  });
  it('旧项目的版本与预检响应不能写入当前项目，未知运行不能被选中', () => {
    const loaded = sessionReducer(initialSession, { type: 'projectLoaded', project });
    expect(sessionReducer(loaded, { type: 'modelSaved', model: { ...model, projectId: 'other' } })).toBe(loaded);
    expect(sessionReducer(loaded, { type: 'preflightReceived', preflight: { ...preflight, projectId: 'other' } })).toBe(loaded);
    expect(sessionReducer(loaded, { type: 'runSelected', id: 'other-run' })).toBe(loaded);
  });
});
