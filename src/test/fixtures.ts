import type { ProjectState, RunRecord, ModelVersion, PreflightReport, EnvironmentInfo, EngineResult } from '../domain/contracts';
import { DEFAULT_CONFIG } from '../domain/model';

// 固定夹具仅由测试导入，生产界面从桌面宿主读取科研数据。
export const environment: EnvironmentInfo = {
  pythonExecutable: 'C:/Python/python.exe', engineVersion: '0.1.0', pythonVersion: '3.14.0',
  numpyVersion: '2.4.0', scipyVersion: '1.17.0', engineSourceHash: 'a'.repeat(64),
};
export const model: ModelVersion = {
  id: 'model-1', projectId: 'project-1', label: 'Ellis v1', createdAt: '2026-10-06T00:00:00Z',
  config: { ...DEFAULT_CONFIG, impactParameters: [0], sampleCount: 2 }, contentHash: 'b'.repeat(64),
};
export const result: EngineResult = {
  protocolVersion: 1, requestId: 'run-1', type: 'result', config: model.config,
  trajectories: [{
    impactParameter: 0, termination: 'through',
    samples: [
      { affine: 0, t: 0, l: 10, theta: Math.PI / 2, phi: 0, kt: 1, kl: -1, kTheta: 0, kPhi: 0 },
      { affine: 20, t: 20, l: -10, theta: Math.PI / 2, phi: 0, kt: 1, kl: -1, kTheta: 0, kPhi: 0 },
    ],
    events: [{ kind: 'throat', affine: 10, radius: 0 }, { kind: 'exit_negative', affine: 20, radius: -10 }],
    diagnostics: { maxEnergyError: 0, maxAngularMomentumError: 0, maxNullError: 0, maxEquatorialError: 0, turningRadiusError: null, radialAnalyticError: 0, azimuthReferenceError: 0 },
    validation: { status: 'passed', checks: [{ name: '零条件', actual: 0, threshold: 1e-8, passed: true }] },
  }],
  environment: { pythonVersion: environment.pythonVersion, numpyVersion: environment.numpyVersion, scipyVersion: environment.scipyVersion },
};
export const run: RunRecord = {
  id: 'run-1', projectId: model.projectId, modelVersionId: model.id, createdAt: model.createdAt,
  startedAt: model.createdAt, finishedAt: '2026-10-06T00:00:01Z', state: 'completed', validationStatus: 'passed',
  request: { protocolVersion: 1, requestId: 'run-1', action: 'traceEllis', config: model.config },
  environment, result, error: null,
};
export const project: ProjectState = {
  project: { id: model.projectId, name: '测试研究', path: 'C:/Research/测试研究', createdAt: model.createdAt, schemaVersion: 1 },
  models: [model], runs: [run],
};
export const preflight: PreflightReport = {
  id: 'preflight-1', projectId: model.projectId, modelVersionId: model.id, config: model.config,
  environment, createdAt: model.createdAt, status: 'ready', issues: [],
  executionLimits: { maxWallTimeSeconds: 300, maxOutputBytes: 67108864, maxTotalSamples: 100000 },
};
