import { Descriptions, Empty, Table, Typography } from 'antd';
import { DesignNotice as Alert, SemanticTag } from './DesignNotice';
import type { EnvironmentInfo, RunRecord, Trajectory } from '../domain/contracts';
import { EVENT_LABELS, RUN_LABELS, VALIDATION_LABELS } from '../domain/session';

const numberText = (value: number) => value === 0 ? '0' : value.toExponential(6);
export function EnvironmentDetails({ environment }: { environment: EnvironmentInfo | null }) {
  if (!environment) return <Empty image={Empty.PRESENTED_IMAGE_SIMPLE} description="尚未探测 Python 环境" />;
  return <Descriptions size="small" column={1} colon={false} items={[
    { key: 'path', label: 'Python 路径', children: <span className="break-all">{environment.pythonExecutable}</span> },
    { key: 'python', label: 'Python', children: environment.pythonVersion },
    { key: 'engine', label: '计算引擎', children: environment.engineVersion },
    { key: 'numpy', label: 'NumPy', children: environment.numpyVersion },
    { key: 'scipy', label: 'SciPy', children: environment.scipyVersion },
    { key: 'hash', label: '引擎 SHA-256', children: <code className="break-all">{environment.engineSourceHash}</code> },
  ]} />;
}

// 执行、数值验证与人工复核独立显示；数值通过绝不推导现实物理结论。
export function RecordInspector({ run, trajectory, onSeek }: { run: RunRecord | null; trajectory: Trajectory | null; onSeek(affine: number): void }) {
  if (!run) return <Empty image={Empty.PRESENTED_IMAGE_SIMPLE} description="尚无运行结果" />;
  return <div className="record-inspector">
    <div className="status-tags"><SemanticTag tone={run.state === 'completed' ? 'info' : run.state === 'failed' ? 'error' : 'warning'}>{RUN_LABELS[run.state]}</SemanticTag>
      <SemanticTag tone={run.validationStatus === 'passed' ? 'success' : run.validationStatus === 'failed' ? 'error' : 'warning'}>{VALIDATION_LABELS[run.validationStatus]}</SemanticTag><SemanticTag>人工复核未记录</SemanticTag></div>
    {run.error && <Alert type="error" showIcon title={run.error.message} description={run.error.code} />}
    <Descriptions column={1} size="small" items={[
      { key: 'run', label: '运行身份', children: <code className="break-all">{run.id}</code> },
      { key: 'model', label: '模型版本', children: <code className="break-all">{run.modelVersionId}</code> },
      { key: 'created', label: '创建时间', children: run.createdAt },
      { key: 'finished', label: '完成时间', children: run.finishedAt ?? '尚未终止' },
    ]} />
    {trajectory && <>
      <Typography.Title level={5}>选中光线 · b = {trajectory.impactParameter}</Typography.Title>
      <Typography.Paragraph type="secondary">{({ through: '已穿过喉部并抵达负侧边界', returned: '已转向并返回正侧边界', budget_exhausted: '仿射参数预算耗尽，传播归类未定', solver_failed: '求解未收敛，保留可用样本' })[trajectory.termination]}</Typography.Paragraph>
      <Table size="small" pagination={false} rowKey="name" dataSource={trajectory.validation.checks} columns={[
        { title: '检查', dataIndex: 'name' },
        { title: '实际 / 门槛', key: 'actual', render: (_, check) => <span className="numeric">{numberText(check.actual)}<br /><small>≤ {numberText(check.threshold)}</small></span> },
        { title: '状态', dataIndex: 'passed', render: (passed: boolean) => <SemanticTag tone={passed ? 'success' : 'error'}>{passed ? '通过' : '未通过'}</SemanticTag> },
      ]} />
      <Typography.Title level={5}>事件定位</Typography.Title>
      {trajectory.events.length === 0 ? <Typography.Text type="secondary">本次轨迹尚无已确认事件。</Typography.Text> : <div className="event-list">
        {trajectory.events.map((event, index) => <button key={`${event.kind}-${index}`} className="event-row" onClick={() => onSeek(event.affine)}>
          <span>{EVENT_LABELS[event.kind]}</span><span>λ = {event.affine.toPrecision(7)}</span>
        </button>)}
      </div>}
    </>}
    <Alert type="info" showIcon title="数值检查仅针对当前假定几何。" description="当前计算路径为 SciPy；GYOTO 交叉验证、独立科研复核与完整首发验收尚未完成。" />
  </div>;
}
