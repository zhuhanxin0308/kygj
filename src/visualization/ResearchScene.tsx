import { Component, useEffect, useMemo, useRef, useState, type ReactNode, type RefObject } from 'react';
import { Canvas, useThree } from '@react-three/fiber';
import { GizmoHelper, GizmoViewport, Line, OrbitControls } from '@react-three/drei';
import { Button, Checkbox, Popover, Tooltip } from 'antd';
import { DesignNotice as Alert } from '../components/DesignNotice';
import { AimOutlined, BorderOutlined, CompressOutlined, DragOutlined, EyeOutlined, MinusOutlined, PlusOutlined, ReloadOutlined, RotateRightOutlined, ScissorOutlined } from '@ant-design/icons';
import { BufferAttribute, BufferGeometry, DoubleSide, MOUSE, Vector3 } from 'three';
import type { OrbitControls as OrbitControlsImpl } from 'three-stdlib';
import type { EllisConfig } from '../domain/model';
import type { EngineResult } from '../domain/contracts';
import { displaySamples, embeddingPoint, makeSurface, sampleAtAffine } from '../domain/geometry';
import { DESIGN_COLORS } from '../design/system';
import { rayColor } from './palette';
import { animateCameraReset, cameraPosition, SCENE_DISPLAY as CAMERA } from './sceneCamera';
import './visualization.css';

interface Props { config: EllisConfig | null; result: EngineResult | null; selected: number; affine: number; onSelect(index: number): void }
type SceneTool = 'rotate' | 'pan' | 'pick';

// GPU 不可用时保留数据和诊断入口，不渲染伪造的静态截图替代结果。
class SceneBoundary extends Component<{ children: ReactNode }, { failed: boolean }> {
  state = { failed: false };
  static getDerivedStateFromError() { return { failed: true }; }
  render() { return this.state.failed ? <Alert type="warning" showIcon title="当前环境无法创建 WebGL 视图。" description="二维数据、来源查询和科研结果不受影响。" /> : this.props.children; }
}

function Surface({ config, cut, scale }: { config: EllisConfig; cut: boolean; scale: number }) {
  const geometry = useMemo(() => {
    const mesh = makeSurface(config.throatRadius, config.initialRadius, cut, scale);
    const positions = new BufferAttribute(new Float32Array(mesh.positions), 3);
    const buffer = new BufferGeometry();
    buffer.setAttribute('position', positions);
    buffer.setIndex(mesh.indices); buffer.computeVertexNormals();
    // 经纬线与曲面共享真实顶点，但各用独立索引，绝不把三角面降采样成展示网格。
    const wire = new BufferGeometry();
    wire.setAttribute('position', positions);
    wire.setIndex(mesh.wireIndices);
    return { surface: buffer, wire };
  }, [config.throatRadius, config.initialRadius, cut, scale]);
  useEffect(() => () => { geometry.surface.dispose(); geometry.wire.dispose(); }, [geometry]);
  return <group>
    <mesh geometry={geometry.surface}><meshStandardMaterial color={DESIGN_COLORS.secondary} transparent opacity={CAMERA.surfaceOpacity} side={DoubleSide} depthWrite={false} /></mesh>
    <lineSegments geometry={geometry.wire}><lineBasicMaterial color={DESIGN_COLORS.secondary} transparent opacity={CAMERA.wireOpacity} depthWrite={false} /></lineSegments>
  </group>;
}

function CameraSettings({ extent, controls, orthographic }: { extent: number; controls: RefObject<OrbitControlsImpl | null>; orthographic: boolean }) {
  const { camera, invalidate, size } = useThree();
  const previousFitZoom = useRef(1);
  const previousFitDistance = useRef(1);
  useEffect(() => {
    const orbit = controls.current;
    if (!orbit) return;
    // 复位基准同时保存位置、平移中心和倍率，避免控制器下一帧又看向旧中心。
    orbit.target.set(0, 0, 0);
    camera.position.set(...cameraPosition(extent, CAMERA.fieldOfView, size.width / size.height));
    camera.zoom = 1;
    previousFitZoom.current = 1;
    const initialDistance = camera.position.length();
    previousFitDistance.current = initialDistance;
    orbit.minDistance = initialDistance / CAMERA.maxZoom;
    orbit.maxDistance = initialDistance / CAMERA.minZoom;
    camera.updateProjectionMatrix();
    orbit.update();
    orbit.saveState();
    invalidate();
  }, [camera, extent, controls, invalidate]);
  useEffect(() => {
    const orbit = controls.current;
    if (!orbit) return;
    // 正交适配视口时保留用户已选倍率；滚轮、触控、按钮共用控制器的缩放边界。
    const fitZoom = orthographic ? Math.min(size.width, size.height) / (2 * extent * CAMERA.orthographicMargin) : 1;
    camera.zoom *= fitZoom / previousFitZoom.current;
    previousFitZoom.current = fitZoom;
    orbit.minZoom = fitZoom * CAMERA.minZoom;
    orbit.maxZoom = fitZoom * CAMERA.maxZoom;
    orbit.zoom0 = fitZoom;
    if (!orthographic) {
      const fittedPosition = new Vector3(...cameraPosition(extent, CAMERA.fieldOfView, size.width / size.height));
      const distance = fittedPosition.length();
      camera.position.sub(orbit.target).multiplyScalar(distance / previousFitDistance.current).add(orbit.target);
      previousFitDistance.current = distance;
      orbit.position0.copy(fittedPosition);
      orbit.minDistance = distance / CAMERA.maxZoom;
      orbit.maxDistance = distance / CAMERA.minZoom;
    }
    camera.updateProjectionMatrix();
    orbit.update();
    invalidate();
  }, [camera, controls, invalidate, orthographic, size.width, size.height, extent]);
  return null;
}

function SceneContent({ config, result, selected, affine, onSelect, cut, surface, grid, axes, hiddenRays, tool, controls, orthographic }: Props & {
  config: EllisConfig; cut: boolean; surface: boolean; grid: boolean; axes: boolean; hiddenRays: ReadonlySet<number>; tool: SceneTool;
  controls: RefObject<OrbitControlsImpl | null>; orthographic: boolean;
}) {
  // 仅显示坐标归一化，防止合法双精度科研尺度在 GPU Float32 中溢出或被相机裁剪。
  const scale = Math.max(config.initialRadius, config.throatRadius);
  const extent = Math.hypot(...embeddingPoint(config.initialRadius, 0, config.throatRadius).map((coordinate) => coordinate / scale));
  const point = (l: number, phi: number) => embeddingPoint(l, phi, config.throatRadius).map((coordinate) => coordinate / scale) as [number, number, number];
  return <>
    <ambientLight intensity={1.2} /><directionalLight position={[4, 8, 6]} intensity={2} />
    <CameraSettings extent={extent} controls={controls} orthographic={orthographic} />
    <OrbitControls ref={controls} makeDefault enableDamping={false} enableRotate={tool !== 'pick'}
      mouseButtons={{ LEFT: tool === 'pan' ? MOUSE.PAN : MOUSE.ROTATE, MIDDLE: MOUSE.DOLLY, RIGHT: MOUSE.PAN }} />
    {surface && <Surface config={config} cut={cut} scale={scale} />}
    {grid && <gridHelper args={[extent * CAMERA.gridExtent, CAMERA.gridDivisions, DESIGN_COLORS.border, DESIGN_COLORS.raised]} position={[0, -extent, 0]} />}
    {axes && <><axesHelper args={[extent * CAMERA.axisSize]} onUpdate={(helper) => helper.setColors(DESIGN_COLORS.secondary, DESIGN_COLORS.primary, DESIGN_COLORS.warning)} /><GizmoHelper alignment="bottom-left" margin={[52, 66]}>
      <GizmoViewport axisColors={[DESIGN_COLORS.secondary, DESIGN_COLORS.primary, DESIGN_COLORS.warning]} labels={['X', 'Y', 'Z']} labelColor={DESIGN_COLORS.text} hideNegativeAxes disabled />
    </GizmoHelper></>}
    {result?.trajectories.map((trajectory, index) => {
      if (hiddenRays.has(trajectory.impactParameter)) return null;
      const points = displaySamples(trajectory.samples, affine).map((sample) => point(sample.l, sample.phi));
      const marker = sampleAtAffine(trajectory.samples, affine);
      const color = rayColor(trajectory.impactParameter);
      return <group key={`${index}-${trajectory.impactParameter}`}>
        {points.length >= 2 && <Line points={points} color={color} lineWidth={index === selected ? 3 : 1.5}
          transparent opacity={index === selected ? 1 : 0.5} onClick={(event) => { event.stopPropagation(); onSelect(index); }} />}
        {marker && <mesh position={point(marker.l, marker.phi)} onClick={(event) => { event.stopPropagation(); onSelect(index); }}>
          <sphereGeometry args={[Math.max(CAMERA.minimumMarker, config.throatRadius / scale * (index === selected ? CAMERA.selectedMarkerScale : CAMERA.markerScale)), 16, 12]} />
          <meshBasicMaterial color={color} />
        </mesh>}
      </group>;
    })}
  </>;
}

export function ResearchScene(props: Props) {
  const [cut, setCut] = useState(false);
  const [surface, setSurface] = useState(true);
  const [grid, setGrid] = useState(true);
  const [axes, setAxes] = useState(true);
  const [layersOpen, setLayersOpen] = useState(false);
  const [hiddenRays, setHiddenRays] = useState<ReadonlySet<number>>(new Set());
  const [tool, setTool] = useState<SceneTool>('rotate');
  const [orthographic, setOrthographic] = useState(false);
  const controls = useRef<OrbitControlsImpl | null>(null);
  const wrapper = useRef<HTMLDivElement>(null);
  const layerContent = useRef<HTMLDivElement>(null);
  const layerFocusOrigin = useRef<HTMLElement | null>(null);
  const keyboardLayerOpen = useRef(false);
  const cancelReset = useRef<(() => void) | null>(null);
  // 图层弹层通过portal挂载；键盘焦点必须显式进入弹层，并在关闭时回到原控件。
  const focusLayer = () => {
    if (keyboardLayerOpen.current) layerContent.current?.querySelector<HTMLInputElement>('input[type="checkbox"]')?.focus();
  };
  const closeLayers = () => {
    setLayersOpen(false); keyboardLayerOpen.current = false;
    if (layerFocusOrigin.current?.isConnected) layerFocusOrigin.current.focus();
  };
  useEffect(() => {
    if (!layersOpen) return;
    const frame = requestAnimationFrame(focusLayer);
    return () => cancelAnimationFrame(frame);
  }, [layersOpen]);
  useEffect(() => () => cancelReset.current?.(), [orthographic]);
  // 按钮和键盘沿用滚轮当前状态，不再维护与真实相机脱节的第二套倍率。
  const zoomBy = (factor: number) => { cancelReset.current?.(); controls.current?.dollyOut(factor); };
  const resetCamera = () => {
    cancelReset.current?.();
    if (controls.current) cancelReset.current = animateCameraReset(controls.current, window.matchMedia('(prefers-reduced-motion: reduce)').matches);
  };
  const setRayVisible = (impact: number, visible: boolean) => setHiddenRays((current) => {
    const next = new Set(current); if (visible) next.delete(impact); else next.add(impact); return next;
  });
  return <div className="scene-panel" ref={wrapper}>
    <div className="panel-heading"><div><strong>三维研究视场</strong><span className="subtle">Ellis 赤道嵌入</span></div>
      <div className="scene-tools" role="toolbar" aria-label="三维场景工具">
        <Tooltip title="旋转视角"><Button aria-label="旋转视角" aria-pressed={tool === 'rotate'} type={tool === 'rotate' ? 'primary' : 'text'} icon={<RotateRightOutlined />} onClick={() => setTool('rotate')} /></Tooltip>
        <Tooltip title="拖动平移"><Button aria-label="拖动平移" aria-pressed={tool === 'pan'} type={tool === 'pan' ? 'primary' : 'text'} icon={<DragOutlined />} onClick={() => setTool('pan')} /></Tooltip>
        <Tooltip title="放大（+）"><Button aria-label="放大视图" type="text" icon={<PlusOutlined />} onClick={() => zoomBy(CAMERA.zoomStep)} /></Tooltip>
        <Tooltip title="缩小（−）"><Button aria-label="缩小视图" type="text" icon={<MinusOutlined />} onClick={() => zoomBy(1 / CAMERA.zoomStep)} /></Tooltip>
        <Tooltip title={orthographic ? '切换透视投影' : '切换正交投影'}><Button aria-label="切换投影" type={orthographic ? 'primary' : 'text'} icon={<BorderOutlined />} onClick={() => setOrthographic(!orthographic)} /></Tooltip>
        <Tooltip title="曲面剖切"><Button aria-label="曲面剖切" aria-pressed={cut} type={cut ? 'primary' : 'text'} icon={<ScissorOutlined />} onClick={() => setCut(!cut)} /></Tooltip>
        <Popover title="场景图层" trigger="click" open={layersOpen} afterOpenChange={(open) => { if (open) focusLayer(); }}
          onOpenChange={(open) => { if (open) layerFocusOrigin.current = document.activeElement as HTMLElement; setLayersOpen(open); }}
          content={<div className="scene-layer-list" ref={layerContent} onFocusCapture={() => { keyboardLayerOpen.current = false; }}
            onKeyDown={(event) => { if (event.key === 'Escape') { event.preventDefault(); event.stopPropagation(); closeLayers(); } }}>
          <Checkbox autoFocus={keyboardLayerOpen.current} checked={surface} onChange={(event) => setSurface(event.target.checked)}>嵌入曲面</Checkbox>
          <Checkbox checked={grid} onChange={(event) => setGrid(event.target.checked)}>参考网格</Checkbox>
          <Checkbox checked={axes} onChange={(event) => setAxes(event.target.checked)}>坐标轴</Checkbox>
          <span className="scene-layer-section">光线路径 · 仅控制显示</span>
          {props.result?.trajectories.map((ray, index) => <div key={`${index}-${ray.impactParameter}`} className={`scene-ray-layer${index === props.selected ? ' is-selected' : ''}`}>
            <Checkbox aria-label={`光线 b = ${ray.impactParameter}`} checked={!hiddenRays.has(ray.impactParameter)} onChange={(event) => setRayVisible(ray.impactParameter, event.target.checked)}><i style={{ background: rayColor(ray.impactParameter) }} />b = {ray.impactParameter}</Checkbox>
          </div>)}
          {!props.result && <span className="subtle">计算完成后显示光线图层</span>}
        </div>}>
          <Tooltip title="图层（L）"><Button aria-label="场景图层" aria-expanded={layersOpen} type={layersOpen ? 'primary' : 'text'} icon={<EyeOutlined />} /></Tooltip>
        </Popover>
        <Tooltip title="拾取光线"><Button aria-label="拾取光线" aria-pressed={tool === 'pick'} type={tool === 'pick' ? 'primary' : 'text'} icon={<AimOutlined />} onClick={() => setTool('pick')} /></Tooltip>
        <Tooltip title="复位视角（R）"><Button aria-label="复位视角" type="text" icon={<ReloadOutlined />} onClick={resetCamera} /></Tooltip>
        <Tooltip title="切换全屏"><Button aria-label="切换全屏" type="text" icon={<CompressOutlined />} onClick={() => { if (document.fullscreenElement) void document.exitFullscreen(); else void wrapper.current?.requestFullscreen?.(); }} /></Tooltip>
      </div>
    </div>
    <div className="scene-canvas" data-tool={tool} tabIndex={0} role="region" aria-label="可交互三维视图，拖动旋转，右键平移，滚轮缩放；键盘加减缩放，R复位，L图层，Escape关闭图层"
      onKeyDown={(event) => {
        if (event.key === '+' || event.key === '=') zoomBy(CAMERA.zoomStep);
        if (event.key === '-') zoomBy(1 / CAMERA.zoomStep);
        if (event.key.toLowerCase() === 'r') resetCamera();
        if (event.key.toLowerCase() === 'l') {
          event.preventDefault();
          if (layersOpen) closeLayers();
          else { layerFocusOrigin.current = event.currentTarget; keyboardLayerOpen.current = true; setLayersOpen(true); }
        }
        if (event.key === 'Escape') closeLayers();
      }}>
      {props.config ? <SceneBoundary><Canvas key={String(orthographic)} orthographic={orthographic} frameloop="demand" dpr={[1, 2]}
        camera={{ fov: CAMERA.fieldOfView, near: CAMERA.near, far: CAMERA.far, zoom: 1 }}>
        <SceneContent {...props} config={props.config} cut={cut} surface={surface} grid={grid} axes={axes} hiddenRays={hiddenRays} tool={tool} controls={controls} orthographic={orthographic} />
      </Canvas></SceneBoundary> : <Alert type="warning" title="参数无效，已暂停几何预览。" />}
      <div className="scene-caption"><span className="live-dot" />{props.result ? '显示真实运行轨迹 · 光标插值仅用于显示' : '公式几何预览 · 尚未生成计算轨迹'}</div>
      <div className="scene-scale-note">真实嵌入比例 · X 为嵌入轴</div>
      <div className="scene-coordinate">R = √(l² + a²)<br />X = a asinh(l/a)<br /><span>统一尺度归一化 · 光标为显示符号</span><br /><span>嵌入坐标不表示传播时间</span></div>
    </div>
  </div>;
}
