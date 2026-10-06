# execute_verification

用途：对冻结运行的实际证据执行独立检查，不启动新的科学计算。

请求：`{projectId,request:{runId,ruleVersionId,clientRequestId,previousRecordId}}`，`previousRecordId`可为null。与桌面命令统一外层`request`参数相区分，内部`request`是执行请求对象。

响应：完整`VerificationRecord`。请求意图先事务提交，检查完成后另行追加不可变记录。相同`clientRequestId`与相同内容返回原记录；不同内容报冲突；中断请求不会因重试而自动重新计算。同运行旧规则的记录可作为重检前驱，跨项目/跨运行拒绝。

执行状态与数值结论分离。结论枚举为`not_run/passed/failed/missing_artifact/not_applicable/inconclusive`。缺产物不填零；规则不适用不算通过；缺独立求积误差等必要证据标证据不足。检查使用稳定diagnostics、保存样本和事件，并保存实际范围及JSON证据位置，不读取中文旧checks的passed。

每项都保留实际值、门槛、单位、依据、状态、原因。来源保存冻结请求/环境/结果SHA-256、哈希格式及来源。旧产物迁移身份为`observed_at_migration`，不证明生成以来完整性。原运行、原数值状态和旧记录不改写。

错误：`verification_run_active`、`verification_request_conflict`、`verification_pending`、`verification_busy`、`verification_history_conflict`、`result_integrity_failed`等。`propagation_completion`检查初态、分支、事件、方向与末端一致性；临界轨道只按已保存有限区间检查，不表示稳定性或有限时间完成传播。
