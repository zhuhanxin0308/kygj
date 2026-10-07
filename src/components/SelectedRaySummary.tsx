import { Descriptions } from 'antd';
import type { RunRecord, Trajectory } from '../domain/contracts';
import { rayColor } from '../visualization/palette';
import { DesignNotice, SemanticTag } from './DesignNotice';

interface Props { run: RunRecord | null; trajectory: Trajectory | null; previewDraft: boolean }

// 摘要只读冻结运行；模型草稿无论是否保存，都不能替换已选轨迹的参数来源。
export function SelectedRaySummary({ run, trajectory, previewDraft }: Props) {
  if (previewDraft) return <div className="selected-ray-summary"><DesignNotice type="info" title="当前为草稿几何预览！" /></div>;
  if (!run?.result || !trajectory) return null;
  return <section className="selected-ray-summary" aria-label="选中光线摘要">
    <div className="selected-ray-heading"><span><i style={{ background: rayColor(trajectory.impactParameter) }} />b = {trajectory.impactParameter}</span><SemanticTag tone="info">冻结运行</SemanticTag></div>
    <Descriptions column={2} size="small" colon={false} items={[
      { key: 'throat', label: 'a', children: <output aria-label="冻结喉尺度 a" className="numeric">{run.result.config.throatRadius}</output> },
      { key: 'initial', label: 'l₀', children: <output aria-label="冻结初始径向坐标 l₀" className="numeric">{run.result.config.initialRadius}</output> },
      { key: 'run', label: '运行', span: 2, children: <code className="break-all">{run.id}</code> },
    ]} />
  </section>;
}
