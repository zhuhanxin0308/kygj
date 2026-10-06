# prepare_project_migration

用途：为v1项目生成只读迁移预览，不能据此开始修改项目。

请求：`{directory}`。

响应：`ProjectMigrationPlan {id,directory,projectId,projectName,fromVersion,toVersion,createdAt,sourceFingerprint,backupPath,changes,warnings,requiresConfirmation}`。

计划保存在当前宿主会话，绑定规范目录、文件身份、项目身份及SQLite一致性逻辑快照（包含WAL中的已提交状态）。预览不写数据库、不创建备份；项目仍按v1保留。界面必须显示变更、备份路径和警告，并在用户确认后调用apply接口。

警告包含：旧结果仅锚定迁移时所见内容；不证明历史完整性；不补写验证通过记录；此次只迁移数据库元数据，不搬移外部大文件。其他格式拒绝，不静默降级或升级。

错误：`migration_not_supported`、`corrupt_project`、`unsafe_project_path`、`file_access_error`、`migration_storage_error`。普通`open_project`对v1返回`migration_required`。
