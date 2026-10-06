# get_project

输入request：{projectId}。返回ProjectState。

仅从已打开项目的真实存储读取模型版本和运行列表，按创建时间倒序。未知projectId不解析为任意文件路径。

测试：空项目、多个版本/运行、未知ID、跨项目身份隔离。
