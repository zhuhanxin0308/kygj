# list_verification_records

用途：按运行读取不可变的独立验证历史。

请求：`{projectId,runId,offset,limit}`，`offset>=0`，`1<=limit<=100`。

响应：`{records,total,nextOffset}`，按追加先后升序；新记录不移动已有分页索引。返回每条记录的精确规则身份、前驱记录、请求身份、实际值、证据与内容哈希。

查询核验所属项目、运行、规则祖先链、请求内容身份、当前源产物身份与历史记录内容。旧失败、证据不足和中断记录全部保留，不能只返回最新通过项。

错误：`invalid_pagination`、`record_not_found`、`result_integrity_failed`、`corrupt_verification`。有损坏时不能继续把该批历史显示为有效验证。
