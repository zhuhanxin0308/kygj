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
- 初始事件为(0, initialRadius, π/2, 0)，E=1，L=b；k^t=1、k^l=−sqrt(1−b²/(initialRadius²+a²))、k^θ=0、k^φ=b/(initialRadius²+a²)。要求根号内严格正；允许负b，保留角运动方向。
- 这是首批Ellis模板的明确输入接口，不把任意坐标、任意度规或任意初值宣称为已经实现。其他适配器采用独立action与Schema扩展。
- 同一请求内配置冻结，响应必须回传完全相同的config；非法配置、未知action、未知版本和多余字段应返回结构化输入错误。

## 正常响应

```json
{
  "protocolVersion": 1,
  "requestId": "原请求标识",
  "type": "result",
  "config": {},
  "trajectories": [{
    "impactParameter": 0.5,
    "termination": "through",
    "samples": [{"affine": 0, "t": 0, "l": 10, "theta": 1.5707963267948966, "phi": 0, "kt": 1, "kl": -0.9987616097609354, "kTheta": 0, "kPhi": 0.0049504950495049506}],
    "events": [{"kind": "throat", "affine": 0, "radius": 0}],
    "diagnostics": {
      "maxEnergyError": 0,
      "maxAngularMomentumError": 0,
      "maxNullError": 0,
      "maxEquatorialError": 0,
      "turningRadiusError": null,
      "radialAnalyticError": null,
      "azimuthReferenceError": null
    },
    "validation": {"status": "passed", "checks": []}
  }],
  "environment": {"pythonVersion": "实际版本", "numpyVersion": "实际版本", "scipyVersion": "实际版本"}
}
```

上例只描述响应结构，config必须填实际冻结值，事件、诊断和样本必须由实际计算填入，不得将示例数值作为结果返回。samples包含实际终止时刻，按sampleCount等距采样；events包含用事件根求得的时刻，而不是取离散最小值。

termination为through、returned、budget_exhausted或solver_failed。事件kind为throat、turning、exit_negative、return_positive。返回正侧事件必须径向向外，避免把初始点当终止。临界abs(b)=a可在预算结束时保持未判定，不伪报穿越。

validation.status为passed、failed或inconclusive；checks每项为{"name":"名称","actual":0,"threshold":0,"passed":true}，值与阈值均有限。实际角动量误差用a×abs(E0)归一，禁止b=0时除以L0。SCI07门槛和误差定义按PRD执行，常量集中命名；网格、收敛和独立求积不能通过减精度绕过。

## 错误响应

```json
{"protocolVersion":1,"requestId":"原标识或unidentified","type":"error","error":{"code":"invalid_request","message":"中文可读错误","details":{}}}
```

错误code为invalid_request、unsupported_version、unsupported_action、computation_failed、internal_error。错误消息不得包含凭据、任意文件内容或完整内部异常堆栈。求解器的未收敛状态不是协议异常：保留可用轨迹、termination和验证状态；无法产生合法有限轨迹时返回computation_failed。

## 启动与测试

模块入口：`python -m gravity_engine`，以stdin输入一行请求；程序从stdin读完一行，校验后计算并输出响应，不启动HTTP服务。Python包位于engine/src/gravity_engine。

必须先完成非法输入、b=0解析轨迹、b=0.5穿喉、b=2转向、b=a预算终止、负b方向、失败状态和JSON读写协议测试，再实现。相同公开请求由Python集成测试和Rust协议测试共同消费；夹具不用于生产界面。
