import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { Workbench } from './components/Workbench';
import './styles/shell.css';
import './styles/workbench.css';

// 应用入口仅负责 React 装配，研究状态和桌面能力由业务层管理。
const container = document.getElementById('root');
if (!container) throw new Error('应用根节点不存在。');
createRoot(container).render(<StrictMode><Workbench /></StrictMode>);
