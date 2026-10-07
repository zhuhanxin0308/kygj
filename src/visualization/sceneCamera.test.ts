import { describe, expect, it, vi } from 'vitest';
import { PerspectiveCamera, Vector3 } from 'three';
import { animateCameraReset, cameraPosition } from './sceneCamera';

// 逐帧推进真实相机与目标，不用等待真实时间或依赖 WebGL。
function cameraFixture() {
  const camera = new PerspectiveCamera(); camera.position.set(8, 6, 5); camera.zoom = 2;
  const controls = { object: camera, target: new Vector3(3, -2, 1), position0: new Vector3(0.6, 0.8, 3), target0: new Vector3(), zoom0: 1, update: vi.fn(), enabled: true };
  const frames: FrameRequestCallback[] = [];
  const request = (callback: FrameRequestCallback) => { frames.push(callback); return frames.length; };
  return { camera, controls, frames, request, cancel: vi.fn() };
}
describe('相机构图与平滑复位', () => {
  it('构图按视口适配且不改动科研坐标', () => {
    const wide = cameraPosition(1, 42, 2);
    const narrow = cameraPosition(1, 42, 0.5);
    expect(new Vector3(...narrow).length()).toBeGreaterThan(new Vector3(...wide).length());
    expect(wide[2]).toBeGreaterThan(wide[0]);
    expect(cameraPosition(2, 42, 2)[2]).toBeCloseTo(wide[2] * 2);
  });
  it('360毫秒内逐帧插值位置、目标与倍率，结束恢复交互', () => {
    const fixture = cameraFixture();
    animateCameraReset(fixture.controls, false, fixture.request, fixture.cancel);
    expect(fixture.controls.enabled).toBe(false);
    fixture.frames.shift()!(0); fixture.frames.shift()!(180);
    expect(fixture.camera.position.x).toBeLessThan(8);
    expect(fixture.camera.position.x).toBeGreaterThan(0.6);
    expect(fixture.camera.zoom).toBeGreaterThan(1);
    fixture.frames.shift()!(360);
    expect(fixture.camera.position.equals(fixture.controls.position0)).toBe(true);
    expect(fixture.controls.target.equals(fixture.controls.target0)).toBe(true);
    expect(fixture.camera.zoom).toBe(1);
    expect(fixture.controls.enabled).toBe(true);
  });
  it('减少动态效果立即复位，取消动画不会遗留禁用控制器', () => {
    const reduced = cameraFixture();
    animateCameraReset(reduced.controls, true, reduced.request, reduced.cancel);
    expect(reduced.frames).toHaveLength(0);
    expect(reduced.camera.position.equals(reduced.controls.position0)).toBe(true);
    const animated = cameraFixture();
    const stop = animateCameraReset(animated.controls, false, animated.request, animated.cancel);
    stop(); expect(animated.cancel).toHaveBeenCalled(); expect(animated.controls.enabled).toBe(true);
  });
});
