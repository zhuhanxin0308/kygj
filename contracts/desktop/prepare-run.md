# prepare_run

输入request：{projectId,modelVersionId,pythonExecutable}。返回PreflightReport。

验证模型归属、配置、环境身份及项目写入条件，提供执行限额和阻断项。预检不启动科学计算，不生成数值通过状态。

测试：真实环境就绪、模型跨项目、非法配置、缺依赖、环境探测失败、限额公开且与执行策略相同。
