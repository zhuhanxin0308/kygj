// @vitest-environment node
// 跨语言验收直接执行项目科学引擎，禁止用伪造响应代替真实数值结果。
import { spawn } from 'node:child_process';
import { resolve } from 'node:path';
import { describe, expect, it } from 'vitest';
import { engineResultSchema } from '../../src/domain/contracts';
import { configSchema, DEFAULT_CONFIG } from '../../src/domain/model';

const PROCESS_TIMEOUT_MS = 30_000;
const MAX_RESPONSE_BYTES = 64 * 1024 * 1024;
const python = process.env.GRAVITY_TEST_PYTHON ?? resolve(
  '.venv', process.platform === 'win32' ? 'Scripts/python.exe' : 'bin/python',
);

function executeEngine(request: unknown): Promise<Record<string, unknown>> {
  return new Promise((accept, reject) => {
    const child = spawn(python, ['-I', '-m', 'gravity_engine'], {
      shell: false, windowsHide: true, stdio: ['pipe', 'pipe', 'pipe'],
    });
    const output: Buffer[] = [];
    const diagnostics: Buffer[] = [];
    let bytes = 0;
    let failure: Error | null = null;
    const timeout = setTimeout(() => {
      failure = new Error('真实科学引擎超过集成测试时限！');
      child.kill();
    }, PROCESS_TIMEOUT_MS);
    child.on('error', (error) => { clearTimeout(timeout); reject(error); });
    child.stdout.on('data', (chunk: Buffer) => {
      bytes += chunk.length;
      if (bytes > MAX_RESPONSE_BYTES) {
        failure = new Error('科学引擎响应超过契约限制！');
        child.kill();
      } else output.push(chunk);
    });
    child.stderr.on('data', (chunk: Buffer) => diagnostics.push(chunk));
    child.stdin.on('error', (error) => { failure = error; });
    child.on('close', (code) => {
      clearTimeout(timeout);
      if (failure) return reject(failure);
      const lines = Buffer.concat(output).toString('utf8').trim().split('\n');
      if (lines.length !== 1) return reject(new Error('科学进程必须只返回一行 JSONL！'));
      try {
        const result = JSON.parse(lines[0]) as Record<string, unknown>;
        if (code !== 0 && result.type !== 'error') {
          return reject(new Error(Buffer.concat(diagnostics).toString('utf8') || `进程退出码：${code}`));
        }
        accept(result);
      } catch (error) { reject(error); }
    });
    child.stdin.end(`${JSON.stringify(request)}\n`);
  });
}

describe('真实 Python 与 TypeScript 数据协议', () => {
  it('穿喉、返回、负冲量和临界轨迹均能按原始冻结配置进入界面', async () => {
    const config = { ...DEFAULT_CONFIG, impactParameters: [0, 0.5, 2, -2, 1], sampleCount: 41 };
    const response = await executeEngine({ protocolVersion: 1, requestId: 'integration-ellis', action: 'traceEllis', config });
    const result = engineResultSchema.parse(response);
    expect(result.config).toEqual(config);
    expect(result.requestId).toBe('integration-ellis');
    expect(result.trajectories.map((ray) => ray.termination)).toEqual([
      'through', 'through', 'returned', 'returned', 'budget_exhausted',
    ]);
    expect(result.trajectories.slice(0, 4).every((ray) => ray.validation.status === 'passed')).toBe(true);
    expect(result.trajectories[4].validation.status).toBe('inconclusive');
    // 返回光线的转向半径取真实事件根，与 Ellis 解析值独立比较。
    const turning = result.trajectories[2].events.find((event) => event.kind === 'turning');
    expect(turning?.radius).toBeCloseTo(Math.sqrt(3), 7);
    const positive = result.trajectories[2].samples.at(-1)!;
    const negative = result.trajectories[3].samples.at(-1)!;
    expect(positive.phi).toBeCloseTo(-negative.phi, 7);
    expect(positive.l).toBeCloseTo(negative.l, 7);
  }, PROCESS_TIMEOUT_MS + 1_000);

  it.each([
    ['尺度平方溢出', { throatRadius: 1e200 }],
    ['布尔值不是实数参数', { throatRadius: true }],
    ['径向初态无效', { impactParameters: [11] }],
    ['总采样越界', { sampleCount: 100_000 }],
    ['禁止静默放宽容差', { relativeTolerance: Number.EPSILON }],
  ])('前端与引擎一致拒绝：%s', async (_name, patch) => {
    const config = { ...DEFAULT_CONFIG, ...patch };
    expect(configSchema.safeParse(config).success).toBe(false);
    const response = await executeEngine({ protocolVersion: 1, requestId: 'invalid-config', action: 'traceEllis', config });
    expect(response).toMatchObject({ type: 'error', requestId: 'invalid-config', error: { code: 'invalid_request' } });
  }, PROCESS_TIMEOUT_MS + 1_000);
});
