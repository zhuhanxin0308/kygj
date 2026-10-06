# list_verification_rules

用途：读取当前项目的独立验证规则版本，不改变运行时引擎检查。

请求：`{projectId, offset, limit}`，外层使用桌面统一`request`参数。`offset`从0开始，`limit`为1至100。

响应：`VerificationRulePage {rules, total, nextOffset}`。按追加顺序升序分页，新建规则不会改变已有索引。规则完整字段以`crates/workbench-core/src/verification_types.rs`的`VerificationRuleVersion`为准，包括精确版本、父版本、方法、检查门槛、依据及内容SHA-256。

错误：`project_not_open`、`invalid_pagination`、`corrupt_verification`或`invalid_verification_rule`。规则读取核验本身及完整祖先链，不以名称替代身份。默认规则为Ellis SciPy诊断重评，不表示SCI07整体完成。
