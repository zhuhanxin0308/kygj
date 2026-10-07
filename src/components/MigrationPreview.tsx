import { Button, Descriptions, Modal, Space, Typography, theme } from 'antd';
import type { ProjectMigrationPlan, ProjectMigrationReceipt } from '../domain/verification';
import { DesignNotice } from './DesignNotice';

// 只展示宿主生成的只读计划；确认按钮是升级原项目的唯一界面入口。
export function MigrationPreview({ plan, receipt = null, busy, error, onCancel, onRefresh, onConfirm, onRetry }: {
  plan: ProjectMigrationPlan | null; busy: boolean; error: string | null;
  receipt?: ProjectMigrationReceipt | null;
  onCancel(): void; onRefresh(): void; onConfirm(): void; onRetry?(): void;
}) {
  const { token } = theme.useToken();
  return <Modal title={receipt ? '项目迁移回执' : '审阅项目格式迁移'} open={Boolean(plan || receipt)} onCancel={onCancel} closable={!busy} footer={<Space>
    <Button disabled={busy} onClick={onCancel}>{receipt ? '关闭迁移回执' : '取消迁移'}</Button>
    {receipt ? <Button type="primary" aria-label="重新打开已迁移项目" loading={busy} disabled={!onRetry} onClick={onRetry}>重新打开已迁移项目</Button> : <>
      <Button disabled={busy} onClick={onRefresh}>重新预览计划</Button>
      <Button type="primary" aria-label="确认备份并迁移此项目" loading={busy} onClick={onConfirm}>确认备份并迁移此项目</Button>
    </>}
  </Space>} width={token.screenMD}>
    {error && <DesignNotice type="error" title={error} />}
    {receipt ? <>
      <DesignNotice type="info" title="项目迁移已完成，回执与可恢复备份已保存。" description="重新打开只读取已迁移项目，保留这次迁移的备份身份。" />
      <Descriptions column={1} size="small" items={[
        { key: 'project', label: '项目身份', children: receipt.projectId },
        { key: 'directory', label: '已迁移项目目录', children: receipt.directory },
        { key: 'plan', label: '已提交计划', children: receipt.planId },
        { key: 'versions', label: '已完成格式变更', children: `v${receipt.fromVersion} → v${receipt.toVersion}` },
        { key: 'backup', label: '可恢复备份位置', children: receipt.backupPath },
        { key: 'hash', label: '备份 SHA-256', children: <code className="break-all">{receipt.backupSha256}</code> },
        { key: 'time', label: '迁移完成时间', children: receipt.migratedAt },
        { key: 'legacy', label: '保留的旧计算产物数', children: receipt.legacyResultCount },
      ]} />
    </> : plan && <>
      <DesignNotice type="warning" showIcon title="确认后将备份并升级所选旧项目。取消不会修改原项目。" />
      <Descriptions column={1} size="small" items={[
        { key: 'name', label: '项目', children: `${plan.projectName} · ${plan.projectId}` },
        { key: 'directory', label: '原项目目录', children: plan.directory },
        { key: 'versions', label: '格式变更', children: `v${plan.fromVersion} → v${plan.toVersion}` },
        { key: 'backup', label: '可恢复备份位置', children: plan.backupPath },
        { key: 'plan', label: '计划身份', children: plan.id },
        { key: 'fingerprint', label: '源内容指纹', children: <code className="break-all">{plan.sourceFingerprint}</code> },
      ]} />
      <Typography.Title level={5}>计划变更</Typography.Title>{plan.changes.map((change) => <Typography.Paragraph key={change}>{change}</Typography.Paragraph>)}
      {plan.warnings.map((warning) => <DesignNotice key={warning} type="warning" showIcon title={warning} />)}
    </>}
  </Modal>;
}
