import { useEffect, useState } from 'react';
import { Button, Empty, Modal, Select, Space, Table, Typography, theme } from 'antd';
import type { RunRecord } from '../domain/contracts';
import type { DesktopClient } from '../services/desktop';
import { useVerification } from './useVerification';
import { VerificationRecordDetails, VerificationRuleDetails, VerificationRuleEditor, VerificationStatus } from './VerificationDetails';
import { DesignNotice } from './DesignNotice';

// 独立验证只由显式执行产生记录；缺产物仍可执行以记录真实缺口。
export function VerificationPanel({ client, projectId, run }: { client: DesktopClient; projectId: string | null; run: RunRecord | null }) {
  const verification = useVerification(client, projectId, run?.id ?? null);
  const { token } = theme.useToken();
  const [ruleOpen, setRuleOpen] = useState(false);
  const [editOpen, setEditOpen] = useState(false);
  const executeLabel = verification.retryRequest ? '重试原独立验证请求' : verification.state?.latestRecord ? '再次执行独立验证' : '执行独立验证';
  useEffect(() => { setRuleOpen(false); setEditOpen(false); }, [projectId, run?.id]);
  if (!projectId) return <Empty image={Empty.PRESENTED_IMAGE_SIMPLE} description="打开项目后管理独立验证规则。" />;
  return <section aria-label="独立结果验证">
    <Typography.Title level={5}>独立结果验证</Typography.Title>
    <Typography.Paragraph type="secondary">对冻结产物按版本化规则重检。每次保留实际值、阈值、依据与来源；引擎诊断重评、保存样本及事件检查不代表 SCI07 全部验收完成。</Typography.Paragraph>
    {verification.error && <DesignNotice type="error" showIcon title={verification.error} role="alert" />}
    <Select aria-label="独立验证规则版本" style={{ width: '100%' }} loading={verification.loading} disabled={verification.busy}
      placeholder="选择已冻结规则" value={verification.selectedRule?.id} onChange={verification.selectRule}
      options={verification.rules.map((rule) => ({ value: rule.id, label: `${rule.title} · ${rule.id}` }))} />
    {verification.ruleNext !== null && <Button aria-label="加载更多规则版本" loading={verification.busy} onClick={() => void verification.loadMoreRules()}>加载更多规则版本</Button>}
    {!verification.loading && verification.rules.length === 0 && <DesignNotice type="warning" title="当前项目未提供独立验证规则，不能宣称检查通过。" />}
    <Space wrap style={{ marginBlock: token.marginSM }}>
      <Button disabled={!verification.selectedRule || verification.busy} onClick={() => setRuleOpen(true)}>查看所选规则完整依据</Button>
      <Button disabled={!verification.selectedRule || verification.busy} onClick={() => setEditOpen(true)}>创建规则新版本</Button>
      <Button disabled={verification.busy || verification.loading} onClick={() => void verification.refresh()}>刷新独立验证</Button>
    </Space>
    {!run ? <Typography.Paragraph>选择运行后执行独立检查与查看历史。</Typography.Paragraph> : <>
      {verification.state && <div>{verification.state.pendingRequestId ? <DesignNotice type="info" title="检查进行中，请刷新取得持久化结果。" description={`请求 ${verification.state.pendingRequestId}`} /> : <VerificationStatus conclusion={verification.state.conclusion} />}</div>}
      {verification.retryRequest && <DesignNotice type="warning" title="上次请求尚未确认，重试将继续核实同一次独立检查。" description={`客户端请求 ${verification.retryRequest.clientRequestId}`} />}
      {!run.result && <DesignNotice type="warning" title="当前运行没有计算产物；独立检查将记录产物缺口。" />}
      <Space wrap style={{ marginBlock: token.marginSM }}>
        <Button type="primary" aria-label={executeLabel} loading={verification.busy} disabled={!verification.retryRequest && (!verification.state || Boolean(verification.state.pendingRequestId))} onClick={() => void verification.execute()}>{executeLabel}</Button>
      </Space>
      {verification.state?.latestRecord && <Button block onClick={() => void verification.inspectRecord(verification.state!.latestRecord!.id)} disabled={verification.busy}>查看所选规则最近记录</Button>}
      <Typography.Title level={5}>运行验证历史（{verification.recordTotal}）</Typography.Title>
      <Table rowKey="id" size="small" dataSource={verification.records} pagination={false} loading={verification.loading} scroll={{ x: token.screenSM }} columns={[
        { title: '记录 / 规则', key: 'id', render: (_, record) => <><Button type="link" aria-label={`查看验证记录 ${record.id}`} disabled={verification.busy} onClick={() => void verification.inspectRecord(record.id)}>{record.id}</Button><div className="break-all">{record.ruleVersionId}</div><small>{record.finishedAt}</small></> },
        { title: '实际结论', key: 'conclusion', render: (_, record) => <VerificationStatus conclusion={record.conclusion} /> },
      ]} />
      {verification.recordNext !== null && <Button block aria-label="加载更多历史记录" loading={verification.busy} onClick={() => void verification.loadMoreRecords()}>加载更多历史记录</Button>}
    </>}
    <Modal title="独立验证规则" open={ruleOpen} onCancel={() => setRuleOpen(false)} footer={null} width={token.screenLG}>
      {verification.selectedRule && <VerificationRuleDetails rule={verification.selectedRule} />}
    </Modal>
    {editOpen && verification.selectedRule && <VerificationRuleEditor key={verification.selectedRule.id} rule={verification.selectedRule} busy={verification.busy} onSave={verification.saveRule} onClose={() => setEditOpen(false)} />}
    <Modal title="独立验证历史记录" open={Boolean(verification.detail)} onCancel={verification.closeDetail} footer={null} width={token.screenXL}>
      {verification.detail && <VerificationRecordDetails record={verification.detail} />}
    </Modal>
  </section>;
}
