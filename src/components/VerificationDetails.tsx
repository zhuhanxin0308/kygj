import { Descriptions, Form, Input, InputNumber, Modal, Table, Typography, theme } from 'antd';
import { CONCLUSION_LABELS, VERIFICATION_PAGE_SIZE, verificationParagraphSchema, verificationTextSchema, type VerificationRecord, type VerificationRuleDraft, type VerificationRuleVersion } from '../domain/verification';
import { DesignNotice, SemanticTag } from './DesignNotice';

const numberText = (value: number | null) => value === null ? '未取得实际值' : value === 0 ? '0' : value.toExponential(8);
export function VerificationStatus({ conclusion }: { conclusion: VerificationRecord['conclusion'] }) {
  const tones = { passed: 'success', failed: 'error', missing_artifact: 'warning', inconclusive: 'warning', not_run: 'neutral', not_applicable: 'neutral' } as const;
  return <SemanticTag tone={tones[conclusion]}>{CONCLUSION_LABELS[conclusion]}</SemanticTag>;
}

// 历史详情使用记录自身冻结的实际值、依据与来源，不借用当前编辑规则。
export function VerificationRecordDetails({ record }: { record: VerificationRecord }) {
  const { token } = theme.useToken();
  return <>
    <VerificationStatus conclusion={record.conclusion} />
    <Descriptions column={1} size="small" items={[
      { key: 'id', label: '验证记录', children: record.id },
      { key: 'project', label: '项目', children: record.projectId },
      { key: 'run', label: '来源运行', children: record.runId },
      { key: 'rule', label: '冻结规则版本', children: record.ruleVersionId },
      { key: 'previous', label: '前序验证记录', children: record.previousRecordId ?? '首次检查' },
      { key: 'request', label: '验证请求 / 客户端请求', children: `${record.requestId} / ${record.clientRequestId}` },
      { key: 'method', label: '检查方法与版本', children: `${record.methodId} · ${record.methodVersion}` },
      { key: 'executor', label: '执行者', children: record.executedBy },
      { key: 'execution', label: '检查执行状态', children: { completed: '检查执行完成', failed: '检查执行失败', interrupted: '检查中断' }[record.executionStatus] },
      { key: 'time', label: '开始 / 完成', children: `${record.startedAt} / ${record.finishedAt}` },
      { key: 'input', label: '输入 SHA-256', children: <code className="break-all">{record.source.requestHash}</code> },
      { key: 'environment', label: '环境 SHA-256', children: <code className="break-all">{record.source.environmentHash}</code> },
      { key: 'result', label: '产物 SHA-256', children: <code className="break-all">{record.source.resultHash ?? '缺少产物身份'}</code> },
      { key: 'origin', label: '产物身份建立来源', children: { captured_at_completion: '计算完成时记录', observed_at_migration: '迁移时所见内容；不能证明历史未被修改', no_result: '未取得产物' }[record.source.resultOrigin] },
      { key: 'format', label: '哈希格式', children: record.source.hashFormat },
      { key: 'hash', label: '验证记录 SHA-256', children: <code className="break-all">{record.contentHash}</code> },
    ]} />
    {record.error && <DesignNotice type="error" title={`${record.error.code}：${record.error.message}`} />}
    <Table size="small" rowKey={(check) => `${check.metricId}:${check.trajectoryIndex}`} dataSource={record.checks} pagination={{ pageSize: VERIFICATION_PAGE_SIZE }} scroll={{ x: token.screenMD }} columns={[
      { title: '检查与光线', key: 'metric', render: (_, check) => <>{check.title}<br />{check.trajectoryIndex === null ? '运行级检查' : `光线 ${check.trajectoryIndex + 1}，b = ${check.impactParameter}`}</> },
      { title: '实际值 / 冻结阈值', key: 'values', render: (_, check) => <span className="numeric">{numberText(check.actual)}<br />≤ {numberText(check.threshold)}<br />{check.unit}</span> },
      { title: '结论与原因', key: 'conclusion', render: (_, check) => <><VerificationStatus conclusion={check.conclusion} /><p>{check.message}</p><code>{check.reasonCode}</code></> },
      { title: '冻结依据与证据', key: 'evidence', render: (_, check) => <><p style={{ whiteSpace: 'pre-wrap' }}>{check.basis}</p><p>证据范围：{check.evidenceScope}</p>{check.evidencePaths.map((path) => <div key={path}><code className="break-all">{path}</code></div>)}</> },
    ]} />
  </>;
}

export function VerificationRuleDetails({ rule }: { rule: VerificationRuleVersion }) {
  const { token } = theme.useToken();
  return <>
    <Descriptions column={1} size="small" items={[
      { key: 'title', label: '名称', children: rule.title }, { key: 'id', label: '版本身份', children: rule.id },
      { key: 'family', label: '规则系列', children: rule.ruleFamilyId }, { key: 'parent', label: '父版本', children: rule.parentVersionId ?? '初始版本' },
      { key: 'method', label: '检查方法与版本', children: `${rule.methodId} · ${rule.methodVersion}` },
      { key: 'author', label: '创建者 / 时间', children: `${rule.createdBy} / ${rule.createdAt}` },
      { key: 'reason', label: '版本变更理由', children: <span style={{ whiteSpace: 'pre-wrap' }}>{rule.changeReason}</span> }, { key: 'type', label: '规则来源', children: rule.builtin ? '内置预定义规则' : '研究者派生版本' },
      { key: 'hash', label: '规则 SHA-256', children: <code className="break-all">{rule.contentHash}</code> },
    ]} />
    <Table rowKey="metricId" size="small" dataSource={rule.checks} pagination={false} scroll={{ x: token.screenMD }} columns={[
      { title: '检查', dataIndex: 'title' }, { title: '阈值', key: 'threshold', render: (_, check) => `≤ ${numberText(check.threshold)} ${check.unit}` },
      { title: '完整依据', key: 'basis', render: (_, check) => <span style={{ whiteSpace: 'pre-wrap' }}>{check.basis}</span> }, { title: '适用条件', dataIndex: 'applicability' }, { title: '证据范围', dataIndex: 'evidenceScope' },
    ]} />
  </>;
}

// 全部指标继承旧规则，任何阈值或依据修改均保存成新版本。
const validateRuleText = (schema: typeof verificationTextSchema) => (_: unknown, value: unknown) => {
  const parsed = schema.safeParse(value);
  return parsed.success ? Promise.resolve() : Promise.reject(new Error(parsed.error.issues[0].message));
};
export function VerificationRuleEditor({ rule, busy, onSave, onClose }: {
  rule: VerificationRuleVersion; busy: boolean; onSave(draft: VerificationRuleDraft): Promise<boolean>; onClose(): void;
}) {
  const [form] = Form.useForm<VerificationRuleDraft>();
  const { token } = theme.useToken();
  return <Modal title="创建独立规则新版本" open onCancel={onClose} okText="保存为新规则版本" cancelText="取消" confirmLoading={busy} width={token.screenMD}
    onOk={() => { void form.validateFields().then(async (draft) => { if (await onSave({ ...draft, baseVersionId: rule.id })) onClose(); }).catch(() => undefined); }}>
    <Typography.Paragraph>原规则与历史失败记录永久保留。请输入研究依据和变更理由，新规则需另行执行检查。</Typography.Paragraph>
    <Form form={form} layout="vertical" initialValues={{ title: rule.title, changeReason: '', thresholds: rule.checks.map(({ metricId, threshold, basis }) => ({ metricId, threshold, basis })) }} disabled={busy}>
      <Form.Item name="title" label="规则版本名称" rules={[{ required: true, whitespace: true, message: '请填写版本名称。' }, { validator: validateRuleText(verificationTextSchema) }]}><Input /></Form.Item>
      <Form.Item name="changeReason" label="变更理由" rules={[{ required: true, whitespace: true, message: '请说明变更理由。' }, { validator: validateRuleText(verificationParagraphSchema) }]}><Input.TextArea autoSize /></Form.Item>
      {rule.checks.map((check, index) => <div key={check.metricId}>
        <Typography.Title level={5}>{check.title}</Typography.Title><Typography.Paragraph type="secondary">{check.applicability} · {check.evidenceScope}</Typography.Paragraph>
        <Form.Item name={['thresholds', index, 'metricId']} hidden><Input /></Form.Item>
        <Form.Item name={['thresholds', index, 'threshold']} label={`${check.title}阈值`} extra={check.metricId === 'propagation_completion' ? '传播终止条件固定为零，不允许放宽。' : undefined} rules={[{ required: true, type: 'number', min: 0, message: '请填写有限非负数。' }]}><InputNumber min={0} disabled={busy || check.metricId === 'propagation_completion'} style={{ width: '100%' }} /></Form.Item>
        <Form.Item name={['thresholds', index, 'basis']} label={`${check.title}依据`} rules={[{ required: true, whitespace: true, message: '请填写完整依据。' }, { validator: validateRuleText(verificationParagraphSchema) }]}><Input.TextArea autoSize /></Form.Item>
      </div>)}
    </Form>
  </Modal>;
}
