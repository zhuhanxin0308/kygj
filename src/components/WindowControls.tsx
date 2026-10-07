import { useState } from 'react';
import { Button, message, Tooltip } from 'antd';
import { BorderOutlined, CloseOutlined, ExclamationCircleOutlined, MinusOutlined } from '@ant-design/icons';
import { isTauri } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';
import '../styles/window-controls.css';

type WindowAction = 'minimize' | 'toggleMaximize' | 'close';

// 一体标题栏直接调用原生窗口；close 会触发 Rust 现有退出门禁，绝不强制销毁进程。
export function WindowControls() {
  const [pending, setPending] = useState(false);
  const [messageApi, contextHolder] = message.useMessage();
  if (!isTauri()) return null;

  async function perform(action: WindowAction) {
    setPending(true);
    try {
      await getCurrentWindow()[action]();
    } catch {
      // 不将原生调用错误静默吞掉；保留窗口和全部科研状态，允许用户重试。
      void messageApi.error({ content: '窗口操作未完成！请重试。', icon: <ExclamationCircleOutlined /> });
    } finally {
      setPending(false);
    }
  }

  return <>
    {contextHolder}
    <div className="window-controls" role="group" aria-label="窗口控制" aria-busy={pending}>
      <Tooltip title="最小化窗口"><Button type="text" aria-label="最小化窗口" icon={<MinusOutlined />} disabled={pending} onClick={() => void perform('minimize')} /></Tooltip>
      <Tooltip title="最大化或还原窗口"><Button type="text" aria-label="最大化或还原窗口" icon={<BorderOutlined />} disabled={pending} onClick={() => void perform('toggleMaximize')} /></Tooltip>
      <Tooltip title="关闭窗口"><Button type="text" className="window-close" aria-label="关闭窗口" icon={<CloseOutlined />} disabled={pending} onClick={() => void perform('close')} /></Tooltip>
    </div>
  </>;
}
