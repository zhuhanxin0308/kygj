import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { PerspectiveCamera, OrthographicCamera } from 'three';
import type { OrbitControls as OrbitControlsImpl } from 'three-stdlib';
import type { ReactNode } from 'react';
import { ResearchScene } from './ResearchScene';
import { model, result } from '../test/fixtures';

// 使用真实 Three.js 几何与相机，只替换 WebGL 挂载边界以便无 GPU 测试。
const hooks = vi.hoisted(() => ({ camera: null as unknown, controls: null as unknown, invalidate: vi.fn(), fail: false, size: { height: 600, width: 800 } }));
// 多步骤 jsdom 交互包含完整 Ant Design 可访问性查询；该时限不是科学或性能验收门槛。
const INTERACTION_TEST_TIMEOUT_MS = 20000;
vi.mock('@react-three/fiber', () => ({
  Canvas: ({ children, orthographic }: { children: ReactNode; orthographic: boolean }) => {
    if (hooks.fail) throw new Error('GPU unavailable');
    // 投影切换与真实 Canvas 一致，测试两种相机的实际缩放路径。
    if (orthographic !== (hooks.camera instanceof OrthographicCamera)) {
      hooks.camera = orthographic ? new OrthographicCamera(-400, 400, 300, -300) : new PerspectiveCamera();
    }
    return <div data-testid="canvas" data-projection={String(orthographic)}>{children}</div>;
  },
  useThree: () => ({ camera: hooks.camera, invalidate: hooks.invalidate, size: hooks.size }),
}));
vi.mock('@react-three/drei', async () => {
  const { forwardRef, useEffect, useImperativeHandle, useMemo, useRef } = await import('react');
  const { OrbitControls: ActualOrbitControls } = await import('three-stdlib');
  return {
    // 保留真实控制器及原生滚轮监听，仅以 DOM 元素替代 WebGL 画布。
    OrbitControls: forwardRef<OrbitControlsImpl, { enableDamping: boolean }>(({ enableDamping }, ref) => {
      const controls = useMemo(() => new ActualOrbitControls(hooks.camera as PerspectiveCamera | OrthographicCamera), [hooks.camera]);
      const element = useRef<HTMLSpanElement>(null);
      controls.enableDamping = enableDamping;
      hooks.controls = controls;
      useImperativeHandle(ref, () => controls, [controls]);
      useEffect(() => {
        controls.connect(element.current!);
        return () => controls.dispose();
      }, [controls]);
      return <span ref={element} data-testid="orbit-controls">轨道相机控制</span>;
    }),
    Line: ({ onClick, points }: { onClick(event: { stopPropagation(): void }): void; points: unknown[] }) => <button aria-label="选中三维轨迹" data-points={points.length} onClick={onClick}>轨迹</button>,
  };
});
afterEach(() => { cleanup(); hooks.fail = false; hooks.size = { height: 600, width: 800 }; });
describe('三维场景与可访问操作', () => {
  it('从冻结结果构建轨迹并支持拾取、缩放、投影与剖切', () => {
    hooks.camera = new PerspectiveCamera();
    const onSelect = vi.fn();
    render(<ResearchScene config={model.config} result={result} selected={0} affine={20} onSelect={onSelect} />);
    fireEvent.click(screen.getByRole('button', { name: '选中三维轨迹' }));
    expect(onSelect).toHaveBeenCalledWith(0);
    const initialDistance = (hooks.controls as OrbitControlsImpl).getDistance();
    fireEvent.click(screen.getByRole('button', { name: '放大视图' }));
    expect(initialDistance / (hooks.controls as OrbitControlsImpl).getDistance() * (hooks.camera as PerspectiveCamera).zoom).toBeGreaterThan(1);
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
  it('平移和旋转后的复位恢复相机位置以及轨道中心', () => {
    hooks.camera = new PerspectiveCamera();
    render(<ResearchScene config={model.config} result={null} selected={0} affine={0} onSelect={vi.fn()} />);
    const controls = hooks.controls as OrbitControlsImpl;
    const camera = hooks.camera as PerspectiveCamera;
    const initialPosition = camera.position.clone();
    controls.target.set(3, -2, 1);
    camera.position.set(8, 6, 5);
    controls.update();
    fireEvent.click(screen.getByRole('button', { name: '复位视角' }));
    expect(controls.target.length()).toBe(0);
    expect(camera.position.distanceTo(initialPosition)).toBeCloseTo(0);
    controls.update();
    expect(camera.position.distanceTo(initialPosition)).toBeCloseTo(0);
  });
  it('透视滚轮与工具栏连续缩放沿用同一距离且保持平移中心', () => {
    hooks.camera = new PerspectiveCamera();
    render(<ResearchScene config={model.config} result={null} selected={0} affine={0} onSelect={vi.fn()} />);
    const controls = hooks.controls as OrbitControlsImpl;
    const camera = hooks.camera as PerspectiveCamera;
    controls.target.set(2, 1, -3);
    camera.position.add(controls.target);
    controls.update();
    const target = controls.target.clone();
    fireEvent.wheel(screen.getByTestId('orbit-controls'), { deltaY: -100 });
    const wheelDistance = controls.getDistance();
    fireEvent.click(screen.getByRole('button', { name: '放大视图' }));
    expect(controls.getDistance()).toBeCloseTo(wheelDistance / 1.25);
    expect(camera.zoom).toBe(1);
    expect(controls.target.equals(target)).toBe(true);
    fireEvent.keyDown(screen.getByRole('region'), { key: '-' });
    expect(controls.getDistance()).toBeCloseTo(wheelDistance);
  });
  it('正交滚轮缩放后按钮从当前倍率继续，调整视口保留倍率和复位基准', () => {
    hooks.camera = new PerspectiveCamera();
    const props = { config: model.config, result: null, selected: 0, affine: 0, onSelect: vi.fn() };
    const view = render(<ResearchScene {...props} />);
    fireEvent.click(screen.getByRole('button', { name: '切换投影' }));
    const camera = hooks.camera as OrthographicCamera;
    const baseline = camera.zoom;
    fireEvent.wheel(screen.getByTestId('orbit-controls'), { deltaY: -100 });
    const wheelZoom = camera.zoom;
    expect(wheelZoom).toBeGreaterThan(baseline);
    fireEvent.click(screen.getByRole('button', { name: '放大视图' }));
    expect(camera.zoom).toBeCloseTo(wheelZoom * 1.25);
    hooks.size = { width: 1600, height: 1200 };
    view.rerender(<ResearchScene {...props} />);
    expect(camera.zoom).toBeCloseTo(wheelZoom * 1.25 * 2);
    fireEvent.keyDown(screen.getByRole('region'), { key: 'r' });
    expect(camera.zoom).toBeCloseTo(baseline * 2);
  });
  it.each([false, true])('滚轮和按钮共享缩放上下界（正交：%s）', (orthographic) => {
    hooks.camera = new PerspectiveCamera();
    render(<ResearchScene config={model.config} result={null} selected={0} affine={0} onSelect={vi.fn()} />);
    if (orthographic) fireEvent.click(screen.getByRole('button', { name: '切换投影' }));
    const controls = hooks.controls as OrbitControlsImpl;
    const camera = hooks.camera as PerspectiveCamera | OrthographicCamera;
    const initialDistance = controls.getDistance();
    const initialZoom = camera.zoom;
    // 大倍率通过真实控制器进入边界，随后检验两类用户入口能否继续越界。
    controls.dollyOut(100);
    fireEvent.wheel(screen.getByTestId('orbit-controls'), { deltaY: -100 });
    fireEvent.click(screen.getByRole('button', { name: '放大视图' }));
    expect(orthographic ? camera.zoom / initialZoom : initialDistance / controls.getDistance()).toBeCloseTo(8);
    controls.dollyIn(100);
    fireEvent.wheel(screen.getByTestId('orbit-controls'), { deltaY: 100 });
    fireEvent.click(screen.getByRole('button', { name: '缩小视图' }));
    expect(orthographic ? camera.zoom / initialZoom : initialDistance / controls.getDistance()).toBeCloseTo(0.25);
  });
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
