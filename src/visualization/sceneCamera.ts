import { MathUtils, Vector3 } from 'three';
import type { OrbitControls } from 'three-stdlib';
import { DESIGN_MOTION } from '../design/system';

// 所有参数只描述相机与显示符号，绝不改变物理坐标各轴之间的比例。
export const SCENE_DISPLAY = {
  cameraDirection: [0.6, 0.35, 1] as const, fitMargin: 1.15, orthographicMargin: 1.4,
  near: 0.01, far: 100, fieldOfView: 42, gridDivisions: 24, gridExtent: 3,
  zoomStep: 1.25, minZoom: 0.25, maxZoom: 8, minimumMarker: 0.009,
  selectedMarkerScale: 0.14, markerScale: 0.09, axisSize: 0.45,
  surfaceOpacity: 0.12, wireOpacity: 0.35,
} as const;

export function cameraPosition(extent: number, fieldOfView: number, aspect: number): [number, number, number] {
  const verticalAngle = MathUtils.degToRad(fieldOfView / 2);
  const horizontalAngle = Math.atan(Math.tan(verticalAngle) * Math.max(Number.EPSILON, aspect));
  const distance = extent * SCENE_DISPLAY.fitMargin / Math.sin(Math.min(verticalAngle, horizontalAngle));
  return new Vector3(...SCENE_DISPLAY.cameraDirection).normalize().multiplyScalar(distance).toArray();
}

type ResetControls = Pick<OrbitControls, 'object' | 'target' | 'position0' | 'target0' | 'zoom0' | 'update' | 'enabled'>;

// 复位包含相机、平移中心与倍率；退出或切换投影时可取消，避免留下冻结的控制器。
export function animateCameraReset(
  controls: ResetControls, reducedMotion: boolean,
  request: typeof requestAnimationFrame = requestAnimationFrame,
  cancel: typeof cancelAnimationFrame = cancelAnimationFrame,
): () => void {
  const camera = controls.object;
  const position = camera.position.clone();
  const target = controls.target.clone();
  const zoom = camera.zoom;
  const wasEnabled = controls.enabled;
  let frame = 0;
  let started: number | null = null;
  let stopped = false;
  const apply = (progress: number) => {
    if (progress === 1) {
      camera.position.copy(controls.position0); controls.target.copy(controls.target0); camera.zoom = controls.zoom0;
    } else {
      camera.position.lerpVectors(position, controls.position0, progress);
      controls.target.lerpVectors(target, controls.target0, progress);
      camera.zoom = zoom + (controls.zoom0 - zoom) * progress;
    }
    camera.updateProjectionMatrix();
    controls.update();
  };
  const stop = () => { stopped = true; cancel(frame); controls.enabled = wasEnabled; };
  if (reducedMotion) { apply(1); return stop; }
  controls.enabled = false;
  const step: FrameRequestCallback = (now) => {
    if (stopped) return;
    started ??= now;
    const progress = Math.min(1, (now - started) / DESIGN_MOTION.camera);
    // 三次平滑步进让起止速度均为零，缓动仅作用于观察相机。
    apply(progress * progress * (3 - 2 * progress));
    if (progress < 1) frame = request(step);
    else controls.enabled = wasEnabled;
  };
  frame = request(step);
  return stop;
}
