# 引力科研工作台

面向虫洞与引力物理研究的开源桌面工作台。采用 Tauri、Rust、React/TypeScript、Ant Design 和独立 Python 计算进程，核心代码按 GPL-3.0-or-later 提供。

当前为开发版本，已具备 Ellis 光线计算、运行来源记录、三维与图表联动、独立结果验证等实现；完整首发功能和六个桌面目标仍需按 PRD 完成验收。

## 从这里接续开发

| 材料 | 用途 |
| --- | --- |
| [设计稿与49页索引](design/README.md) | 浏览原图、识别v3基准与尚缺的设计页；34张科技版和49张旧版均随仓库保存 |
| [全局设计规范](design/v3/全局设计规范.json) | 语义色、布局尺寸、动效时长与交互约定 |
| [产品需求](PRD.md) | 完整功能、科学基准、工作包与验收要求 |
| [架构](ARCHITECTURE.md) | 业务边界、数据路径及实现责任 |
| [研发约定](AGENTS.md) | 中文注释、TDD、回退提交、数据与执行约束 |
| [桌面接口](contracts/desktop/) / [引擎接口](contracts/engine/) | 前后端和科学计算进程的协议 |

设计稿是视觉和交互参考，不是科学计算结果，也不表示相应功能已经实现。三维显示默认保持真实几何比例，不能为复刻示意外形而非等比拉伸物理坐标。

实现入口：桌面界面位于[工作台组件](src/components/Workbench.tsx)，主题令牌位于[src/design](src/design/)，业务与存储位于[workbench-core](crates/workbench-core/)，科学计算位于[engine](engine/)。
