import { useEffect, useRef, useState } from 'react';
import { formatError, type DesktopClient } from '../services/desktop';
import { verificationDraftSchema, VERIFICATION_PAGE_SIZE, type VerificationExecution, type VerificationRecord, type VerificationRuleDraft, type VerificationRuleVersion, type RunVerificationState } from '../domain/verification';

// 项目/运行和规则选择各有代数守卫，迟到请求不串入新工作区；历史只追加。
export function useVerification(client: DesktopClient, projectId: string | null, runId: string | null) {
  const [rules, setRules] = useState<VerificationRuleVersion[]>([]);
  const [ruleId, setRuleId] = useState<string | null>(null);
  const [records, setRecords] = useState<VerificationRecord[]>([]);
  const [state, setState] = useState<RunVerificationState | null>(null);
  const [detail, setDetail] = useState<VerificationRecord | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [busy, setBusy] = useState(false);
  const [ruleNext, setRuleNext] = useState<number | null>(null);
  const [recordNext, setRecordNext] = useState<number | null>(null);
  const [recordTotal, setRecordTotal] = useState(0);
  const [retryRequests, setRetryRequests] = useState<Record<string, VerificationExecution>>({});
  const epoch = useRef(0);
  const selection = useRef(0);
  const lock = useRef(false);
  const selectedRule = rules.find((rule) => rule.id === ruleId) ?? null;
  const retryRequest = ruleId ? retryRequests[ruleId] ?? null : null;
  const scope = useRef({ projectId, runId, ruleId });
  scope.current = { projectId, runId, ruleId };

  useEffect(() => {
    const token = ++epoch.current;
    lock.current = false; setBusy(false); setError(null); setRules([]); setRuleId(null); setRecords([]);
    setState(null); setDetail(null); setRuleNext(null); setRecordNext(null); setRecordTotal(0); setRetryRequests({});
    if (!projectId) { setLoading(false); return; }
    setLoading(true);
    void Promise.all([
      client.listVerificationRules(projectId, 0, VERIFICATION_PAGE_SIZE),
      runId ? client.listVerificationRecords(projectId, runId, 0, VERIFICATION_PAGE_SIZE) : Promise.resolve(null),
    ]).then(([rulePage, recordPage]) => {
      if (token !== epoch.current) return;
      setRules(rulePage.rules); setRuleNext(rulePage.nextOffset); setRuleId(rulePage.rules[0]?.id ?? null);
      if (recordPage) { setRecords(recordPage.records); setRecordNext(recordPage.nextOffset); setRecordTotal(recordPage.total); }
    }).catch((failure) => { if (token === epoch.current) setError(formatError(failure)); })
      .finally(() => { if (token === epoch.current) setLoading(false); });
    return () => { epoch.current += 1; };
  }, [client, projectId, runId]);

  useEffect(() => {
    const token = ++selection.current;
    setState(null);
    if (!projectId || !runId || !ruleId) return;
    void client.getRunVerificationState(projectId, runId, ruleId).then((value) => {
      if (token === selection.current) setState(value);
    }).catch((failure) => { if (token === selection.current) setError(formatError(failure)); });
    return () => { selection.current += 1; };
  }, [client, projectId, runId, ruleId]);

  async function perform(action: (valid: () => boolean) => Promise<void>) {
    if (lock.current || !projectId) return false;
    const token = epoch.current;
    const valid = () => token === epoch.current && scope.current.projectId === projectId && scope.current.runId === runId;
    lock.current = true; setBusy(true); setError(null);
    try { await action(valid); return valid(); }
    catch (failure) { if (valid()) setError(formatError(failure)); return false; }
    finally { if (valid()) { lock.current = false; setBusy(false); } }
  }
  async function refresh(valid: () => boolean) {
    if (!projectId) return;
    // 首次目录加载失败时必须能够重新读取规则；成功后由选择效应读取对应状态。
    if (!ruleId) {
      const [rulePage, recordPage] = await Promise.all([
        client.listVerificationRules(projectId, 0, VERIFICATION_PAGE_SIZE),
        runId ? client.listVerificationRecords(projectId, runId, 0, VERIFICATION_PAGE_SIZE) : Promise.resolve(null),
      ]);
      if (!valid()) return;
      setRules(rulePage.rules); setRuleNext(rulePage.nextOffset); setRuleId(rulePage.rules[0]?.id ?? null);
      if (recordPage) { setRecords(recordPage.records); setRecordNext(recordPage.nextOffset); setRecordTotal(recordPage.total); }
      return;
    }
    if (!runId) return;
    const [nextState, page] = await Promise.all([
      client.getRunVerificationState(projectId, runId, ruleId), client.listVerificationRecords(projectId, runId, 0, VERIFICATION_PAGE_SIZE),
    ]);
    if (!valid()) return;
    if (scope.current.ruleId === ruleId) setState(nextState);
    setRecords(page.records); setRecordNext(page.nextOffset); setRecordTotal(page.total);
  }
  return {
    rules, selectedRule, records, state, detail, error, loading, busy, ruleNext, recordNext, recordTotal, retryRequest,
    selectRule: (id: string) => { if (rules.some((rule) => rule.id === id)) setRuleId(id); },
    closeDetail: () => setDetail(null),
    refresh: () => perform(refresh),
    loadMoreRules: () => perform(async (valid) => {
      if (ruleNext === null) return;
      const page = await client.listVerificationRules(projectId!, ruleNext, VERIFICATION_PAGE_SIZE);
      if (valid()) { setRules((previous) => [...previous, ...page.rules.filter((rule) => !previous.some((old) => old.id === rule.id))]); setRuleNext(page.nextOffset); }
    }),
    loadMoreRecords: () => perform(async (valid) => {
      if (!runId || recordNext === null) return;
      const page = await client.listVerificationRecords(projectId!, runId, recordNext, VERIFICATION_PAGE_SIZE);
      if (valid()) { setRecords((previous) => [...previous, ...page.records.filter((record) => !previous.some((old) => old.id === record.id))]); setRecordNext(page.nextOffset); setRecordTotal(page.total); }
    }),
    saveRule: (draft: VerificationRuleDraft) => perform(async (valid) => {
      const parsed = verificationDraftSchema.safeParse(draft);
      if (!parsed.success || !selectedRule || draft.baseVersionId !== selectedRule.id
        || draft.thresholds.length !== selectedRule.checks.length || !selectedRule.checks.every((check) => draft.thresholds.some((value) => value.metricId === check.metricId))) {
        throw { code: 'invalid_rule', message: '请保留全部检查项，填写有限非负阈值、完整依据和变更理由。' };
      }
      const rule = await client.saveVerificationRuleVersion(projectId!, parsed.data);
      if (valid()) { setRules((previous) => [...previous, rule]); setRuleId(rule.id); }
    }),
    execute: () => perform(async (valid) => {
      if (!runId || !ruleId || (!retryRequest && (!state || state.pendingRequestId))) throw { code: 'verification_not_ready', message: '请等待规则状态读取完成，或先刷新正在进行的独立检查。' };
      // 回复未确认前保留整个请求，手动重试复用幂等身份和前序记录，切换规则也不能丢失它。
      const request = retryRequest ?? { runId, ruleVersionId: ruleId, clientRequestId: crypto.randomUUID(), previousRecordId: state?.latestRecord?.id ?? null };
      setRetryRequests((previous) => ({ ...previous, [ruleId]: request }));
      const record = await client.executeVerification(projectId!, request);
      if (!valid()) return;
      setRetryRequests((previous) => {
        const remaining = { ...previous };
        delete remaining[ruleId];
        return remaining;
      });
      // 返回值是真实持久化记录；失败结论同样保留，不以调用成功决定绿色状态。
      setDetail(record);
      setRecords((previous) => previous.some((item) => item.id === record.id) ? previous : [...previous, record]);
      // 已提交的前序关系必须重新读取，刷新失败时禁止沿用旧状态创建下一次检查。
      if (scope.current.ruleId === ruleId) setState(null);
      await refresh(valid);
      if (valid()) setRecords((previous) => previous.some((item) => item.id === record.id) ? previous : [...previous, record]);
    }),
    inspectRecord: (id: string) => perform(async (valid) => {
      const record = await client.getVerificationRecord(projectId!, id);
      if (record.runId !== runId) throw { code: 'invalid_response', message: '历史记录不属于当前运行。' };
      if (valid()) setDetail(record);
    }),
  };
}
