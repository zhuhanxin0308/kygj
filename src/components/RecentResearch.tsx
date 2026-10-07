import { ArrowRightOutlined, CheckCircleOutlined, ClockCircleOutlined, ExclamationCircleOutlined, LoadingOutlined, RightOutlined } from '@ant-design/icons';
import { Button, Tooltip } from 'antd';
import type { RunRecord } from '../domain/contracts';
import { isActiveRun, RUN_LABELS, VALIDATION_LABELS } from '../domain/session';
import { DESIGN_COLORS } from '../design/system';

const RECENT_RUN_LIMIT = 3;

// 继续研究绑定真实运行身份，只切换已有结果，不隐式重新提交计算。
export function RecentResearch({ runs, onSelect, onViewAll }: {
  runs: RunRecord[]; onSelect(id: string): void; onViewAll(): void;
}) {
  return <section className="recent-research" aria-label="继续研究">
    <div className="panel-heading"><strong>继续研究</strong><Tooltip title="查看全部运行">
      <Button type="text" aria-label="查看全部运行" icon={<ArrowRightOutlined />} onClick={onViewAll} disabled={runs.length === 0} />
    </Tooltip></div>
    {runs.length === 0 ? <p className="empty-note">暂无可继续的运行！</p> : <div className="recent-list">
      {runs.slice(0, RECENT_RUN_LIMIT).map((run) => {
        const active = isActiveRun(run.state);
        const failed = run.state === 'failed' || run.validationStatus === 'failed';
        const Icon = failed ? ExclamationCircleOutlined : active ? LoadingOutlined : run.state === 'completed' ? CheckCircleOutlined : ClockCircleOutlined;
        const color = failed ? DESIGN_COLORS.error : active ? DESIGN_COLORS.primary : DESIGN_COLORS.warning;
        return <button className="recent-item" key={run.id} aria-label={`继续研究 ${run.id}`} onClick={() => onSelect(run.id)}>
          <Icon spin={active} style={{ color }} />
          <span className="recent-item-content"><strong>{run.id.slice(0, 8)} · Ellis 光线积分</strong>
            <span className="recent-state"><span>{RUN_LABELS[run.state]}</span><span>{VALIDATION_LABELS[run.validationStatus]}</span></span>
          </span><RightOutlined />
        </button>;
      })}
    </div>}
  </section>;
}
