# probe_environment

输入request：{pythonExecutable}。返回EnvironmentInfo。

通过固定程序参数及JSON describe请求核验已选Python和gravity_engine，不接收任意Shell文本。版本与源文件身份必须来自实际执行。

测试：合法引擎、路径不存在、缺包、非协议输出、requestId错配、输出超限、超时；错误不得标记就绪。
