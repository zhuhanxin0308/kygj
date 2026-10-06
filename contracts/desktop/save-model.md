# save_model

输入request：{projectId,label,config}。返回ModelVersion。

配置按引擎契约验证，计算内容身份并追加版本；历史版本及其运行不改写。

测试：相同内容身份稳定、参数改变形成新版本、非法数值/字段拒绝、旧运行仍引用原版本。
