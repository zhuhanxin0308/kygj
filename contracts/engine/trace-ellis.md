# traceEllis

用途：执行冻结配置的超静态Ellis八分量零测地线积分。

输入为UTF-8单行JSON：`{protocolVersion:1,requestId,action:"traceEllis",config}`。config的字段、float64运算顺序和资源限制见[共享协议](../engine-v1.md)。未知、多余、重复字段及非有限值明确拒绝。

输出为共享协议定义的result或error。输出config与输入完全相同；轨迹顺序与impactParameters相同。执行预算结束、求解失败及数值验证状态分别保存。诊断名称和门槛随结果一起记录，源文件、软件版本由宿主运行快照保留。

引擎不写项目数据库、不启动服务。stdin只处理一个请求后退出，stdout只返回一行JSONL；取消、墙钟预算和输出捕获上限由宿主管理。成功退出码不能单独证明研究结果通过。

实际验证入口：`engine/tests/test_ellis.py`、`engine/tests/test_protocol.py`及`tests/integration/engine-contract.test.ts`。测试产物不进入生产项目。
