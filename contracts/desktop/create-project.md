# create_project

输入request：{parentDirectory,name}。返回ProjectState，公共类型见../desktop-v1.md。

在已存在父目录下新建项目目录和事务化数据库；禁止越界目录名、Windows保留名和覆盖已有目录。失败不留下被识别为完整的项目。

测试：合法中文名称、空名、路径穿越、无权限、已有目录、数据库初始化失败回滚；重开后项目身份不变。
