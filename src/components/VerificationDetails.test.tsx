import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { VerificationRecordDetails, VerificationRuleDetails } from './VerificationDetails';
import { verificationRecord, verificationRule } from '../test/verificationFixtures';

// 缺产物与迁移所见来源必须明确显示，不能被“检查已完成”替代。
afterEach(cleanup);
describe('验证证据详情', () => {
  it('缺产物失败记录保留错误、无实际值和前序记录', () => {
    render(<VerificationRecordDetails record={{ ...verificationRecord, conclusion: 'missing_artifact', executionStatus: 'failed', previousRecordId: 'previous',
      source: { ...verificationRecord.source, resultHash: null, resultOrigin: 'no_result' }, error: { code: 'missing', message: '产物不存在' },
      checks: [{ ...verificationRecord.checks[0], trajectoryIndex: null, actual: null, conclusion: 'missing_artifact', evidencePaths: [] }] }} />);
    expect(screen.getByText('缺少产物身份')).toBeInTheDocument();
    expect(screen.getByText(/未取得实际值/)).toBeInTheDocument();
    expect(screen.getByText(/运行级检查/)).toBeInTheDocument();
    expect(screen.getByText('previous')).toBeInTheDocument();
    expect(screen.getByText(/产物不存在/)).toBeInTheDocument();
  });
  it('迁移来源和派生规则如实显示，不把迁移指纹称为原始执行证明', () => {
    render(<><VerificationRecordDetails record={{ ...verificationRecord, conclusion: 'failed', source: { ...verificationRecord.source, resultOrigin: 'observed_at_migration' } }} />
      <VerificationRuleDetails rule={{ ...verificationRule, builtin: false, parentVersionId: 'parent-rule' }} /></>);
    expect(screen.getByText('迁移时所见内容；不能证明历史未被修改')).toBeInTheDocument();
    expect(screen.getByText('研究者派生版本')).toBeInTheDocument();
    expect(screen.getByText('parent-rule')).toBeInTheDocument();
  });
});
