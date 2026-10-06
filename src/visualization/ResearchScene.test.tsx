import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { PerspectiveCamera } from 'three';
import type { ReactNode } from 'react';
import { ResearchScene } from './ResearchScene';
import { model, result } from '../test/fixtures';

// 使用真实 Three.js 几何与相机，只替换 WebGL 挂载边界以便无 GPU 测试。
const hooks = vi.hoisted(() => ({ camera: null as unknown, invalidate: vi.fn(), fail: false }));
// 多步骤 jsdom 交互包含完整 Ant Design 可访问性查询；该时限不是科学或性能验收门槛。
const INTERACTION_TEST_TIMEOUT_MS = 20000;
vi.mock('@react-three/fiber', () => ({
  Canvas: ({ children, orthographic }: { children: ReactNode; orthographic: boolean }) => {
    if (hooks.fail) throw new Error('GPU unavailable');
    return <div data-testid="canvas" data-projection={String(orthographic)}>{children}</div>;
  },
  useThree: () => ({ camera: hooks.camera, invalidate: hooks.invalidate, size: { height: 600, width: 800 } }),
}));
vi.mock('@react-three/drei', () => ({
  OrbitControls: () => <span>轨道相机控制</span>,
  Line: ({ onClick, points }: { onClick(event: { stopPropagation(): void }): void; points: unknown[] }) => <button aria-label="选中三维轨迹" data-points={points.length} onClick={onClick}>轨迹</button>,
}));
afterEach(() => { cleanup(); hooks.fail = false; });
describe('三维场景与可访问操作', () => {
  it('从冻结结果构建轨迹并支持拾取、缩放、投影与剖切', () => {
    hooks.camera = new PerspectiveCamera();
    const onSelect = vi.fn();
    render(<ResearchScene config={model.config} result={result} selected={0} affine={20} onSelect={onSelect} />);
    fireEvent.click(screen.getByRole('button', { name: '选中三维轨迹' }));
    expect(onSelect).toHaveBeenCalledWith(0);
    fireEvent.click(screen.getByRole('button', { name: '放大视图' }));
    expect((hooks.camera as PerspectiveCamera).zoom).toBeGreaterThan(1);
    fireEvent.click(screen.getByRole('button', { name: '缩小视图' }));
    fireEvent.keyDown(screen.getByRole('region'), { key: '+' });
    fireEvent.keyDown(screen.getByRole('region'), { key: '-' });
    fireEvent.keyDown(screen.getByRole('region'), { key: 'r' });
    expect((hooks.camera as PerspectiveCamera).zoom).toBe(1);
    fireEvent.click(screen.getByRole('button', { name: '曲面剖切' }));
    expect(screen.getByRole('button', { name: '曲面剖切' })).toHaveAttribute('aria-pressed', 'true');
    fireEvent.click(screen.getByRole('button', { name: '切换投影' }));
    expect(screen.getByTestId('canvas')).toHaveAttribute('data-projection', 'true');
    expect((hooks.camera as PerspectiveCamera).zoom).toBeCloseTo(600 / (2 * Math.hypot(1, 0.1) * 1.4));
    fireEvent.click(screen.getByRole('button', { name: '复位视角' }));
    expect(screen.getByText(/显示真实运行轨迹/)).toBeInTheDocument();
  }, INTERACTION_TEST_TIMEOUT_MS);
  it('无结果只显示公式几何，无效参数暂停预览', () => {
    hooks.camera = new PerspectiveCamera();
    const props = { config: model.config, result: null, selected: 0, affine: 0, onSelect: vi.fn() };
    const view = render(<ResearchScene {...props} />);
    expect(screen.queryByRole('button', { name: '选中三维轨迹' })).not.toBeInTheDocument();
    expect(screen.getByText(/公式几何预览/)).toBeInTheDocument();
    view.rerender(<ResearchScene {...props} config={null} />);
    expect(screen.getByText('参数无效，已暂停几何预览。')).toBeInTheDocument();
  });
  it('GPU 创建失败时保留明确的二维数据入口提示', () => {
    hooks.camera = new PerspectiveCamera(); hooks.fail = true;
    const error = vi.spyOn(console, 'error').mockImplementation(() => undefined);
    render(<ResearchScene config={model.config} result={null} selected={0} affine={0} onSelect={vi.fn()} />);
    expect(screen.getByText('当前环境无法创建 WebGL 视图。')).toBeInTheDocument();
    error.mockRestore();
  });
});
