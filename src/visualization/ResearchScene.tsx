import { Component, useEffect, useMemo, useRef, useState, type ReactNode } from 'react';
import { Canvas, useThree } from '@react-three/fiber';
import { Line, OrbitControls } from '@react-three/drei';
import { Alert, Button, Checkbox, Popover, Space, Tooltip } from 'antd';
import { BorderOutlined, CompressOutlined, EyeOutlined, MinusOutlined, PlusOutlined, ReloadOutlined, ScissorOutlined } from '@ant-design/icons';
import { BufferAttribute, BufferGeometry, DoubleSide } from 'three';
import type { EllisConfig } from '../domain/model';
import type { EngineResult } from '../domain/contracts';
import { displaySamples, embeddingPoint, makeSurface, sampleAtAffine } from '../domain/geometry';
import { RAY_COLORS } from './palette';

const CAMERA = { distanceScale: 3.8, upScale: 2.2, depthScale: 3.2, near: 0.01, far: 100, fitMargin: 1.4, gridDivisions: 24, fieldOfView: 42, zoomStep: 1.25, minZoom: 0.25, maxZoom: 8, minimumMarker: 0.009 };
interface Props { config: EllisConfig | null; result: EngineResult | null; selected: number; affine: number; onSelect(index: number): void }

// GPU 不可用时保留数据和诊断入口，不渲染伪造的静态截图替代结果。
class SceneBoundary extends Component<{ children: ReactNode }, { failed: boolean }> {
  state = { failed: false };
  static getDerivedStateFromError() { return { failed: true }; }
  render() { return this.state.failed ? <Alert type="warning" showIcon title="当前环境无法创建 WebGL 视图。" description="二维数据、来源查询和科研结果不受影响。" /> : this.props.children; }
}

function Surface({ config, cut, scale }: { config: EllisConfig; cut: boolean; scale: number }) {
  const geometry = useMemo(() => {
    const mesh = makeSurface(config.throatRadius, config.initialRadius, cut, scale);
    const buffer = new BufferGeometry();
    buffer.setAttribute('position', new BufferAttribute(new Float32Array(mesh.positions), 3));
    buffer.setIndex(mesh.indices); buffer.computeVertexNormals();
    return buffer;
  }, [config.throatRadius, config.initialRadius, cut, scale]);
  useEffect(() => () => geometry.dispose(), [geometry]);
  return <group>
    <mesh geometry={geometry}><meshStandardMaterial color="#173d62" transparent opacity={0.24} side={DoubleSide} depthWrite={false} /></mesh>
    <mesh geometry={geometry}><meshBasicMaterial color="#4589bd" wireframe transparent opacity={0.22} side={DoubleSide} depthWrite={false} /></mesh>
  </group>;
}

function CameraSettings({ extent, zoom, reset, orthographic }: { extent: number; zoom: number; reset: number; orthographic: boolean }) {
  const { camera, invalidate, size } = useThree();
  useEffect(() => {
    camera.position.set(CAMERA.distanceScale, CAMERA.upScale, CAMERA.depthScale);
    camera.lookAt(0, 0, 0); camera.updateProjectionMatrix(); invalidate();
  }, [camera, extent, reset, invalidate]);
  useEffect(() => {
    camera.zoom = zoom * (orthographic ? Math.min(size.width, size.height) / (2 * extent * CAMERA.fitMargin) : 1);
    camera.updateProjectionMatrix(); invalidate();
  }, [camera, zoom, invalidate, orthographic, size.width, size.height, extent]);
  return null;
}

function SceneContent({ config, result, selected, affine, onSelect, cut, surface, grid, zoom, reset, orthographic }: Props & {
  config: EllisConfig; cut: boolean; surface: boolean; grid: boolean; zoom: number; reset: number; orthographic: boolean;
}) {
  // 仅显示坐标归一化，防止合法双精度科研尺度在 GPU Float32 中溢出或被相机裁剪。
  const scale = Math.max(config.initialRadius, config.throatRadius);
  const extent = Math.hypot(config.initialRadius / scale, config.throatRadius / scale);
  const point = (l: number, phi: number) => embeddingPoint(l, phi, config.throatRadius).map((coordinate) => coordinate / scale) as [number, number, number];
  return <>
    <ambientLight intensity={1.2} /><directionalLight position={[4, 8, 6]} intensity={2} />
    <CameraSettings extent={extent} zoom={zoom} reset={reset} orthographic={orthographic} />
    <OrbitControls makeDefault enableDamping={false} target={[0, 0, 0]} />
    {surface && <Surface config={config} cut={cut} scale={scale} />}
    {grid && <gridHelper args={[extent * 3, CAMERA.gridDivisions, '#244b65', '#162b3e']} position={[0, -extent, 0]} />}
    {result?.trajectories.map((trajectory, index) => {
      const points = displaySamples(trajectory.samples, affine).map((sample) => point(sample.l, sample.phi));
      const marker = sampleAtAffine(trajectory.samples, affine);
      const color = RAY_COLORS[index % RAY_COLORS.length];
      return <group key={`${index}-${trajectory.impactParameter}`}>
        {points.length >= 2 && <Line points={points} color={color} lineWidth={index === selected ? 3 : 1.5}
          transparent opacity={index === selected ? 1 : 0.5} onClick={(event) => { event.stopPropagation(); onSelect(index); }} />}
        {marker && <mesh position={point(marker.l, marker.phi)} onClick={(event) => { event.stopPropagation(); onSelect(index); }}>
          <sphereGeometry args={[Math.max(CAMERA.minimumMarker, config.throatRadius / scale * (index === selected ? 0.14 : 0.09)), 16, 12]} />
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
  const [orthographic, setOrthographic] = useState(false);
  const [zoom, setZoom] = useState(1);
  const [reset, setReset] = useState(0);
  const wrapper = useRef<HTMLDivElement>(null);
  const zoomBy = (factor: number) => setZoom((current) => Math.min(CAMERA.maxZoom, Math.max(CAMERA.minZoom, current * factor)));
  const resetCamera = () => { setZoom(1); setReset((current) => current + 1); };
  return <div className="scene-panel" ref={wrapper}>
    <div className="panel-heading"><div><strong>三维研究视场</strong><span className="subtle">Ellis 赤道嵌入</span></div>
      <Space size={2}>
        <Tooltip title="放大（+）"><Button aria-label="放大视图" type="text" icon={<PlusOutlined />} onClick={() => zoomBy(CAMERA.zoomStep)} /></Tooltip>
        <Tooltip title="缩小（−）"><Button aria-label="缩小视图" type="text" icon={<MinusOutlined />} onClick={() => zoomBy(1 / CAMERA.zoomStep)} /></Tooltip>
        <Tooltip title="复位视角（R）"><Button aria-label="复位视角" type="text" icon={<ReloadOutlined />} onClick={resetCamera} /></Tooltip>
        <Tooltip title={orthographic ? '切换透视投影' : '切换正交投影'}><Button aria-label="切换投影" type={orthographic ? 'primary' : 'text'} icon={<BorderOutlined />} onClick={() => { setOrthographic(!orthographic); resetCamera(); }} /></Tooltip>
        <Tooltip title="曲面剖切"><Button aria-label="曲面剖切" aria-pressed={cut} type={cut ? 'primary' : 'text'} icon={<ScissorOutlined />} onClick={() => setCut(!cut)} /></Tooltip>
        <Popover title="场景图层" content={<Space orientation="vertical"><Checkbox checked={surface} onChange={(event) => setSurface(event.target.checked)}>嵌入曲面</Checkbox><Checkbox checked={grid} onChange={(event) => setGrid(event.target.checked)}>参考网格</Checkbox></Space>}>
          <Button aria-label="场景图层" type="text" icon={<EyeOutlined />} />
        </Popover>
        <Tooltip title="切换全屏"><Button aria-label="切换全屏" type="text" icon={<CompressOutlined />} onClick={() => { if (document.fullscreenElement) void document.exitFullscreen(); else void wrapper.current?.requestFullscreen?.(); }} /></Tooltip>
      </Space>
    </div>
    <div className="scene-canvas" tabIndex={0} role="region" aria-label="可交互三维视图，拖动旋转，右键平移，滚轮缩放；键盘加减缩放，R复位"
      onKeyDown={(event) => { if (event.key === '+' || event.key === '=') zoomBy(CAMERA.zoomStep); if (event.key === '-') zoomBy(1 / CAMERA.zoomStep); if (event.key.toLowerCase() === 'r') resetCamera(); }}>
      {props.config ? <SceneBoundary><Canvas key={String(orthographic)} orthographic={orthographic} frameloop="demand" dpr={[1, 2]}
        camera={{ fov: CAMERA.fieldOfView, near: CAMERA.near, far: CAMERA.far, zoom: 1 }}>
        <SceneContent {...props} config={props.config} cut={cut} surface={surface} grid={grid} zoom={zoom} reset={reset} orthographic={orthographic} />
      </Canvas></SceneBoundary> : <Alert type="warning" title="参数无效，已暂停几何预览。" />}
      <div className="scene-caption"><span className="live-dot" />{props.result ? '显示真实运行轨迹 · 光标插值仅用于显示' : '公式几何预览 · 尚未生成计算轨迹'}</div>
      <div className="scene-coordinate">R = √(l² + a²)<br />z = a asinh(l/a)<br /><span>显示按模型尺度归一化 · 光标大小为显示符号</span><br /><span>嵌入坐标不表示传播时间</span></div>
    </div>
  </div>;
}
