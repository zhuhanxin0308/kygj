# 独立科学计算进程协议 v1

这是宿主与Python进程的内部接口。传输为UTF-8逐行JSON，一次进程处理一个请求后退出；标准输出只含一条最终响应，标准错误用于诊断。进程退出前未取得合法响应不能登记成功。取消由宿主终止进程并等待实际退出，不能伪造计算结果。协议校验的消息大小上限由宿主集中配置。

## 请求

```json
{
  "protocolVersion": 1,
  "requestId": "一个非空唯一关联标识",
  "action": "traceEllis",
  "config": {
    "throatRadius": 1.0,
    "initialRadius": 10.0,
    "impactParameters": [0.0, 0.5, 2.0],
    "maxAffineParameter": 40.0,
    "sampleCount": 1001,
    "relativeTolerance": 1e-10,
    "absoluteTolerance": 1e-12
  }
}
```

- 所有数值必须有限。喉尺度和初始半径严格正，采样点数2—100000、每批1—256条光线；这些是命名的资源保护上限，不是许可或收费限制。
- 每批总采样点数不得超过100000，JSON输入行不得超过1 MiB，requestId长度1—128字符；超限明确拒绝，不自动减少样本。重复JSON字段与布尔假数字同样拒绝。
- maxAffineParameter有限且严格正，不另设物理上限；宿主执行预算负责限制实际耗时。relativeTolerance至少100×float64机器精度且小于1，防止SciPy自动放宽；absoluteTolerance有限且严格正。初态计算若发生浮点溢出，应作为输入错误返回。
- 初始事件为(0, initialRadius, π/2, 0)，E=1，L=b；k^t=1、k^l=−sqrt(1−b²/(initialRadius²+a²))、k^θ=0、k^φ=b/(initialRadius²+a²)。要求根号内严格正；允许负b，保留角运动方向。
- 三层使用相同的float64运算顺序：`den = initialRadius * initialRadius + a * a`、`radial = 1 - (b * b) / den`、`angular = b / den`，要求各中间值有限、den和radial严格正。不能替换为与sqrt或hypot比较，否则在舍入边界会出现预检和执行不一致。
- 这是首批Ellis模板的明确输入接口，不把任意坐标、任意度规或任意初值宣称为已经实现。其他适配器采用独立action与Schema扩展。
- 同一请求内配置冻结，响应必须回传完全相同的config；非法配置、未知action、未知版本和多余字段应返回结构化输入错误。

## 正常响应

响应字段定义如下；表格是类型约束，不提供可误用为科研结果的示例轨迹。

| 字段 | 内容 |
| --- | --- |
| protocolVersion / requestId / type | 1、原请求标识、result |
| config | 原请求的完整冻结配置 |
| trajectories | 按impactParameters原始顺序返回，每条包含impactParameter、termination、samples、events、diagnostics、validation |
| samples | 每点包含affine、t、l、theta、phi、kt、kl、kTheta、kPhi；均为实际计算的有限数值 |
| events | 每项包含kind、affine、radius；radius是有符号坐标l，时刻由事件根确定 |
| diagnostics | maxEnergyError、maxAngularMomentumError、maxNullError、maxEquatorialError为非负有限数；turningRadiusError、radialAnalyticError、azimuthReferenceError为非负有限数或null |
| validation | status以及checks；每项检查有中文name、有限非负actual和threshold、与actual≤threshold相符的passed |
| environment | 实际pythonVersion、numpyVersion、scipyVersion |

samples从affine=0开始，包含实际终止时刻，按sampleCount等距采样且时刻严格递增。events按时刻排列，不能晚于本条轨迹终点。through必须以exit_negative终止，returned必须以return_positive终止，budget_exhausted必须到达请求仿射预算，solver_failed可保留实际部分终点。不能以绘图离散最小值代替事件根。

termination为through、returned、budget_exhausted或solver_failed。事件kind为throat、turning、exit_negative、return_positive。返回正侧事件必须径向向外，避免把初始点当终止。临界abs(b)=a可在预算结束时保持未判定，不伪报穿越。

validation.status为passed、failed或inconclusive；checks每项为{"name":"名称","actual":0,"threshold":0,"passed":true}，值与阈值均有限。实际角动量误差用a×abs(E0)归一，禁止b=0时除以L0。SCI07门槛和误差定义按PRD执行，常量集中命名；网格、收敛和独立求积不能通过减精度绕过。

passed要求非空检查且全部通过，并且termination不能为budget_exhausted或solver_failed。临界长时间积分可能出现守恒残差较小但偏离临界分离支的数值轨迹；保留实际数据，分别检查分离关系、入射方向和正侧位置，预算终止保持inconclusive，不能把这些数据展示为已验证物理轨迹。

## 错误响应

```json
{"protocolVersion":1,"requestId":"原标识或unidentified","type":"error","error":{"code":"invalid_request","message":"中文可读错误","details":{}}}
```

错误code为invalid_request、unsupported_version、unsupported_action、computation_failed、internal_error。错误消息不得包含凭据、任意文件内容或完整内部异常堆栈。求解器的未收敛状态不是协议异常：保留可用轨迹、termination和验证状态；无法产生合法有限轨迹时返回computation_failed。

## 环境探测

describe请求为{"protocolVersion":1,"requestId":"唯一标识","action":"describe"}，不带config，不执行数值计算。返回{"protocolVersion":1,"requestId":"原标识","type":"capabilities","engineVersion":"0.1.0","engineSourceHash":"实际引擎源文件组合SHA256","actions":["traceEllis"],"environment":{"pythonVersion":"实际版本","numpyVersion":"实际版本","scipyVersion":"实际版本"}}。源文件哈希按模块相对路径排序后计算，供宿主检测环境在预检后是否改变；不得填固定伪哈希。非法或额外字段仍拒绝。

## 启动与测试

模块入口：`python -m gravity_engine`，以stdin输入一行请求；程序从stdin读完一行，校验后计算并输出响应，不启动HTTP服务。Python包位于engine/src/gravity_engine。

必须先完成非法输入、b=0解析轨迹、b=0.5穿喉、b=2转向、b=a预算终止、负b方向、失败状态和JSON读写协议测试，再实现。相同公开请求由Python集成测试和Rust协议测试共同消费；夹具不用于生产界面。
