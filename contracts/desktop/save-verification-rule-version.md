# save_verification_rule_version

用途：根据已有规则追加用户规则版本，绝不覆盖旧门槛或旧验证记录。

请求：`{projectId, draft:{baseVersionId,title,changeReason,thresholds:[{metricId,threshold,basis}]}}`。

响应：完整`VerificationRuleVersion`。所有稳定指标必须且只能出现一次；标题、理由、依据必填，长度上限4096 UTF-8字节。理由及依据允许换行、回车和制表，其他控制字符拒绝。阈值为有限非负数；`propagation_completion`固定为0。标题保持单行。

创建版本保存父版本、同一规则族、创建时间、用户变更理由和强类型序列化SHA-256。`builtin=false`；用户门槛通过不成为官方基准通过。规则写入在事务中完成。

错误：`project_not_open`、`record_not_found`、`invalid_verification_rule`、`corrupt_verification`、`storage_error`。跨项目基线和祖先链损坏拒绝。
