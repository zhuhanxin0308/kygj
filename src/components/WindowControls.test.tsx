import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { WindowControls } from './WindowControls';

// 只替换桌面窗口边界；关闭操作必须发出可拦截的请求，不得直接销毁宿主。
const native = vi.hoisted(() => ({
  available: true,
  minimize: vi.fn(), toggleMaximize: vi.fn(), close: vi.fn(), destroy: vi.fn(),
}));
vi.mock('@tauri-apps/api/core', () => ({ isTauri: () => native.available }));
vi.mock('@tauri-apps/api/window', () => ({ getCurrentWindow: () => native }));
afterEach(cleanup);
beforeEach(() => {
  native.available = true;
  for (const method of [native.minimize, native.toggleMaximize, native.close, native.destroy]) method.mockReset().mockResolvedValue(undefined);
});

describe('一体标题栏的真实窗口控制', () => {
  it('浏览器预览不提供无法执行的窗口按钮', () => {
    native.available = false;
    render(<WindowControls />);
    expect(screen.queryByRole('group', { name: '窗口控制' })).not.toBeInTheDocument();
    expect(native.close).not.toHaveBeenCalled();
  });

  it('最小化和最大化复用原生窗口，关闭保留宿主安全退出门禁', async () => {
    render(<WindowControls />);
    fireEvent.click(screen.getByRole('button', { name: '最小化窗口' }));
    await waitFor(() => expect(native.minimize).toHaveBeenCalledOnce());
    await waitFor(() => expect(screen.getByRole('button', { name: '最大化或还原窗口' })).toBeEnabled());
    fireEvent.click(screen.getByRole('button', { name: '最大化或还原窗口' }));
    await waitFor(() => expect(native.toggleMaximize).toHaveBeenCalledOnce());
    await waitFor(() => expect(screen.getByRole('button', { name: '关闭窗口' })).toBeEnabled());
    fireEvent.click(screen.getByRole('button', { name: '关闭窗口' }));
    await waitFor(() => expect(native.close).toHaveBeenCalledOnce());
    expect(native.destroy).not.toHaveBeenCalled();
  });

  it('等待原生回复时阻止重复请求，失败显示可重试提示', async () => {
    let rejectClose!: (reason: Error) => void;
    native.close.mockImplementationOnce(() => new Promise<void>((_resolve, reject) => { rejectClose = reject; }));
    render(<WindowControls />);
    fireEvent.click(screen.getByRole('button', { name: '关闭窗口' }));
    fireEvent.click(screen.getByRole('button', { name: '关闭窗口' }));
    expect(native.close).toHaveBeenCalledOnce();
    expect(screen.getByRole('button', { name: '最小化窗口' })).toBeDisabled();
    await act(async () => rejectClose(new Error('测试原生拒绝')));
    expect(await screen.findByText('窗口操作未完成！请重试。')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: '关闭窗口' }));
    await waitFor(() => expect(native.close).toHaveBeenCalledTimes(2));
    expect(native.destroy).not.toHaveBeenCalled();
  });
});
