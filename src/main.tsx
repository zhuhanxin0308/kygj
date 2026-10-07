import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { Workbench } from './components/Workbench';
import { DESIGN_CSS_VARIABLES } from './design/system';
import './styles/shell.css';
import './styles/workbench.css';

// 应用入口仅负责 React 装配，研究状态和桌面能力由业务层管理。
const container = document.getElementById('root');
if (!container) throw new Error('应用根节点不存在。');
// 弹窗和图层通过portal挂到body，也必须继承与主工作区完全相同的令牌。
for (const [name, value] of Object.entries(DESIGN_CSS_VARIABLES)) document.documentElement.style.setProperty(name, String(value));
createRoot(container).render(<StrictMode><Workbench /></StrictMode>);
