import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import '@testing-library/jest-dom/vitest';
import { ConfigEditor } from './ConfigEditor';
import { DEFAULT_CONFIG } from '../domain/model';

// 参数表单拒绝无效草稿，并把高级参数保留在真实配置中。
afterEach(cleanup);
describe('模型配置表单', () => {
  it('修改基本参数与冲量参数，调用完整配置回调', () => {
    const onChange = vi.fn();
    render(<ConfigEditor config={DEFAULT_CONFIG} onChange={onChange} />);
    fireEvent.change(screen.getByLabelText('喉尺度 a'), { target: { value: '2' } });
    expect(onChange).toHaveBeenCalledWith({ throatRadius: 2 });
    fireEvent.change(screen.getByLabelText('冲量参数 b'), { target: { value: '0, -0.5, 2' } });
    fireEvent.blur(screen.getByLabelText('冲量参数 b'));
    expect(onChange).toHaveBeenCalledWith({ impactParameters: [0, -0.5, 2] });
  });
  it('无效冲量显示可操作错误且不提交', () => {
    const onChange = vi.fn();
    const onValidityChange = vi.fn();
    render(<ConfigEditor config={DEFAULT_CONFIG} onChange={onChange} onValidityChange={onValidityChange} />);
    fireEvent.change(screen.getByLabelText('冲量参数 b'), { target: { value: '0,,1' } });
    fireEvent.blur(screen.getByLabelText('冲量参数 b'));
    expect(screen.getByRole('alert')).toHaveTextContent('冲量参数');
    expect(onChange).not.toHaveBeenCalled();
    expect(onValidityChange).toHaveBeenLastCalledWith(false);
  });
  it('渐进展开积分设置并修改采样预算', () => {
    const onChange = vi.fn();
    render(<ConfigEditor config={DEFAULT_CONFIG} onChange={onChange} />);
    fireEvent.click(screen.getByText('积分与采样设置'));
    fireEvent.change(screen.getByLabelText('采样点数'), { target: { value: '2001' } });
    expect(onChange).toHaveBeenCalledWith({ sampleCount: 2001 });
  });
});
