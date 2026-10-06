import { Alert, Button, Descriptions, Modal, Space, Typography, theme } from 'antd';
import type { ProjectMigrationPlan } from '../domain/verification';

// 只展示宿主生成的只读计划；确认按钮是升级原项目的唯一界面入口。
export function MigrationPreview({ plan, busy, error, onCancel, onRefresh, onConfirm }: {
  plan: ProjectMigrationPlan | null; busy: boolean; error: string | null;
  onCancel(): void; onRefresh(): void; onConfirm(): void;
}) {
  const { token } = theme.useToken();
  return <Modal title="审阅项目格式迁移" open={Boolean(plan)} onCancel={onCancel} closable={!busy} footer={<Space>
    <Button disabled={busy} onClick={onCancel}>取消迁移</Button><Button disabled={busy} onClick={onRefresh}>重新预览计划</Button>
    <Button type="primary" loading={busy} onClick={onConfirm}>确认备份并迁移此项目</Button>
  </Space>} width={token.screenMD}>
    {plan && <>
      <Alert type="warning" showIcon title="确认后将备份并升级所选旧项目。取消不会修改原项目。" />
      {error && <Alert type="error" title={error} />}
      <Descriptions column={1} size="small" items={[
        { key: 'name', label: '项目', children: `${plan.projectName} · ${plan.projectId}` },
        { key: 'directory', label: '原项目目录', children: plan.directory },
        { key: 'versions', label: '格式变更', children: `v${plan.fromVersion} → v${plan.toVersion}` },
        { key: 'backup', label: '可恢复备份位置', children: plan.backupPath },
        { key: 'plan', label: '计划身份', children: plan.id },
        { key: 'fingerprint', label: '源内容指纹', children: <code className="break-all">{plan.sourceFingerprint}</code> },
      ]} />
      <Typography.Title level={5}>计划变更</Typography.Title>{plan.changes.map((change) => <Typography.Paragraph key={change}>{change}</Typography.Paragraph>)}
      {plan.warnings.map((warning) => <Alert key={warning} type="warning" showIcon title={warning} />)}
    </>}
  </Modal>;
}
