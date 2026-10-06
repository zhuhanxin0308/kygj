# export_run

输入request：{projectId,runId,destinationDirectory}。返回{path,sha256}。

导出冻结请求、环境、结果及独立状态的JSON记录，使用新文件，不覆盖已有内容；校验值来自实际写入字节。此接口不冒充完整ZIP64/RO-Crate研究包。

测试：独立解析可读、内容身份一致、已有同名文件不覆盖、无权限/磁盘写入错误、运行归属检查、不导出凭据。
