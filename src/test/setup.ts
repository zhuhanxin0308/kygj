import '@testing-library/jest-dom/vitest';
import { vi } from 'vitest';

// jsdom 没有布局引擎，只补齐浏览器 API；数值求解与桌面协议仍由各自测试验证。
if (typeof window !== 'undefined') {
// rc-component 的 test 环境固定 test-id，导致多组件标签冲突；使用其真实开发标识路径。
// 仅调整当前隔离测试 worker 的环境变量，不改动依赖源码。
vi.stubEnv('NODE_ENV', 'development');
Object.defineProperty(window, 'matchMedia', {
  writable: true,
  value: vi.fn().mockImplementation((query: string) => ({
    matches: false, media: query, onchange: null,
    addListener: vi.fn(), removeListener: vi.fn(),
    addEventListener: vi.fn(), removeEventListener: vi.fn(), dispatchEvent: vi.fn(),
  })),
});
class TestResizeObserver {
  observe() { /* 测试画布通过显式尺寸初始化，无浏览器布局回调。 */ }
  unobserve() { /* 对应无布局的监听取消。 */ }
  disconnect() { /* 对应无布局的生命周期结束。 */ }
}
vi.stubGlobal('ResizeObserver', TestResizeObserver);
const computedStyle = window.getComputedStyle.bind(window);
// jsdom 无伪元素布局，保留普通元素样式查询用于组件可访问性判断。
window.getComputedStyle = (element) => computedStyle(element);
}
