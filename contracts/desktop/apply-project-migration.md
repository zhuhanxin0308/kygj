# apply_project_migration

用途：执行用户已审阅的本会话迁移计划。

请求：`{directory,planId}`；前端不能传自造计划内容。计划一次性消费。

响应：`ProjectMigrationReceipt {planId,directory,projectId,fromVersion,toVersion,backupPath,backupSha256,migratedAt,legacyResultCount}`。成功后可重新打开目录。

流程：取得写事务锁→复核路径、项目身份与完整源指纹→排他创建备份→SQLite备份API取得包含WAL状态的完整v1数据库→逻辑内容及完整性校验、刷盘、SHA-256→事务新增验证表和默认规则→给旧结果保存“迁移时所见”身份→确认原模型/预检/运行字节未变→更新格式与迁移记录→提交。

Windows保留禁止删除共享的目录及备份文件句柄，SQLite备份目标拒绝链接；已有备份目标不覆盖。完整备份不等于迁移成功。提交前任何失败回滚原v1；完成的备份保留可恢复，失败半包删除。源改变返回`migration_plan_stale`要求重新预览。

错误：`migration_plan_stale`、`migration_backup_failed`、`migration_path_lock_failed`、`migration_source_changed`、`migration_storage_error`等。界面只有收到成功回执才能标记升级完成。该接口实现v1到v2本次增量迁移，不代表全部B31的大文件迁移范围已经完成。
