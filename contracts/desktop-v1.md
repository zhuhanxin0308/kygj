# 桌面宿主接口 v1

以下是本批实现的固定接口，前端通过Tauri invoke调用，参数使用单个request对象，JSON字段camelCase。IPC失败返回{code,message}，message为可读中文，不暴露内部堆栈。前端不允许直接读写SQLite或运行Shell。业务库与桌面命令分离，便于无WebView单测。

## 公共对象

- ProjectSummary：{id,name,path,createdAt,schemaVersion}，时间为UTC ISO8601字符串；schemaVersion=1。
- EllisConfig：字段与engine-v1.md相同。完整配置保存为模型版本，不散落默认参数。
- ModelVersion：{id,projectId,label,createdAt,config,contentHash}。追加写入，历史不覆盖。
- RunRecord：{id,projectId,modelVersionId,createdAt,startedAt,finishedAt,state,validationStatus,request,environment,result,error}。
- state：queued、running、completed、failed、cancelling、cancelled、unknown；validationStatus：not_run、passed、failed、inconclusive，二者独立。未发生的时间、结果和错误为null。
- RunRecord.request是冻结的引擎请求，result是合法引擎result响应；environment记录实际Python路径、版本、引擎与依赖版本。error为null或{code,message}。
- ProjectState：{project,models,runs}；列表按创建时间倒序，所有值来自真实存储，无生产假数据。

## 命令

| 命令 | request参数 | 返回 |
| --- | --- | --- |
| create_project | {parentDirectory,name} | ProjectState |
| open_project | {directory} | ProjectState |
| get_project | {projectId} | ProjectState |
| save_model | {projectId,label,config} | ModelVersion |
| probe_environment | {pythonExecutable} | EnvironmentInfo |
| prepare_run | {projectId,modelVersionId,pythonExecutable} | PreflightReport |
| start_run | {projectId,preflightId} | RunRecord（真实提交后的queued或running） |
| get_run | {projectId,runId} | RunRecord |
| cancel_run | {projectId,runId} | RunRecord |
| export_run | {projectId,runId,destinationDirectory} | {path,sha256} |

create_project只在用户选定的已存在父目录中新建项目子目录；项目名称须为合法单个目录名，禁止路径穿越、Windows保留名及覆盖已有目录。open_project只打开合法已存在项目，不执行其中代码。内部数据库位于项目目录的.gravity/workbench.sqlite。打开或创建后由宿主管理projectId到目录的映射，后续业务不接收任意数据库路径。

probe_environment只检查用户指定的Python可执行文件及本产品gravity_engine，不允许提交任意命令文本。通过固定JSON describe请求取得EnvironmentInfo={pythonExecutable,engineVersion,pythonVersion,numpyVersion,scipyVersion,engineSourceHash}；不可取得时抛出明确环境错误，不标为就绪。

PreflightReport={id,projectId,modelVersionId,config,environment,createdAt,status,issues,executionLimits}，status为ready或blocked；issues为{code,message}列表。executionLimits包含maxWallTimeSeconds、maxOutputBytes、maxTotalSamples，本批默认分别300、67108864、100000，必须在预检中向用户展示；实现使用可注入的命名ProcessLimits，测试可降低耗时预算而不改生产配置。标准错误捕获上限512 KiB，不能为收集日志耗尽内存。预检验证配置、环境及写入条件，不代表计算或数值检查通过。start_run只能消费本项目尚未消费、仍有效的ready报告；模型/环境身份变化时拒绝，不能盲用旧预检。开始前事务保存请求快照与运行记录，再启动独立Python进程。

start_run快速返回真实运行身份，计算在后台受控进程进行；界面通过get_run轮询或run_changed事件刷新。取消先记cancelling，执行端实际退出后才记cancelled；完成后取消不能把已完成结果改写。退出/崩溃时未知进程不得自动重跑。

export_run导出可独立读取的JSON研究记录：冻结请求、环境、完整结果与状态、版本/来源。目标文件必须新建，禁止覆盖已有文件，返回真实校验值；这不是把首发完整研究包功能宣称已完成，ZIP64/RO-Crate导入导出继续按PRD的W10实现。

## 前端与验证边界

浏览器没有桌面宿主时，返回明确宿主不可用错误，不能用生产mock替代文件或运行能力。模型几何可由配置公式在本地显示；轨迹、事件和残差必须来自实际RunRecord.result。运行中的草稿变化不改写旧结果。3D展示播放/暂停只操作视图时间轴，与后端取消分开。

本批创建、持久化、版本、预检、真实执行、查询、取消及导出均要真实业务测试。未实现的PRD领域工具不注册为可用能力；开发阶段不宣称正式发行或六平台均已验证。
