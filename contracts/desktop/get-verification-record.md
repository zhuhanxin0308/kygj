# get_verification_record

用途：读取一次独立验证的完整证据，不修改其内容。

请求：`{projectId,recordId}`。

响应：`VerificationRecord`，完整字段见核心共享类型。所有指标提供稳定metricId、实际值、阈值、单位、依据、结论、原因、证据范围及位置；记录提供来源哈希、规则版本、执行者、时间、前驱及请求身份。

成功读取需要项目/运行/规则/请求关联一致，规则祖先与记录哈希有效，记录源身份仍匹配冻结运行。`executionStatus=failed/interrupted`不得有`conclusion=passed`。所有不适用时不能汇总为通过。

错误：`record_not_found`、`result_integrity_failed`、`corrupt_verification`。接口不返回未经校验的历史结果。
