# cancel_run

输入request：{projectId,runId}。返回RunRecord。

先记录取消意图，执行端确认退出后才标cancelled。完成后的取消不会改写已完成结果，关闭展示动画不调用该接口。

测试：排队/执行中取消、退出竞态、重复取消、完成后取消、进程无法停止时状态如实保留。
