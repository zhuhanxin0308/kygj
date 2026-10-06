# describe

用途：探测用户选定Python进程是否能加载本产品引擎，取得能力和真实环境身份；不运行数值任务。

输入为UTF-8单行JSON：`{protocolVersion:1,requestId,action:"describe"}`，不允许config或其他多余字段。

成功输出：`{protocolVersion:1,requestId,type:"capabilities",engineVersion,engineSourceHash,actions:["traceEllis"],environment:{pythonVersion,numpyVersion,scipyVersion}}`。

engineSourceHash按引擎包内模块相对路径排序，对每个UTF-8相对路径、NUL分隔符、源码原字节、NUL分隔符依次做SHA256，返回64位十六进制值。宿主补入实际pythonExecutable，提交运行前再次探测并与预检身份比较。

失败使用[共享协议](../engine-v1.md)的error结构。不能用固定哈希、安装目录存在或Python退出码代替能力校验。
