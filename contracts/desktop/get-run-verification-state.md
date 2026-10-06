# get_run_verification_state

用途：读取某运行、某精确规则版本的独立验证状态。

请求：`{projectId,runId,ruleVersionId}`。

响应：`{runId,ruleVersionId,conclusion,latestRecord,recordCount,pendingRequestId}`。

无记录返回`not_run`及空`latestRecord`。存在未完成请求时返回`inconclusive`及`pendingRequestId`，同时保留上次完成记录；界面应显示“检查进行中/待核实”，不能覆盖历史。无未完成请求时，结论等于最新完成记录的结论。运行时旧`validationStatus`独立显示。

所有运行及验证记录读取均核验来源身份与SHA-256。重开时只有确认验证执行锁无人持有，才追加中断事实，不自动重检。

错误：`record_not_found`、`result_integrity_failed`、`corrupt_verification`等；缺少完整产物与完整性损坏不能混为通过。
