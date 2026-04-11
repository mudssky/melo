# Melo Daemon 管理面与可观测性设计

日期：2026-04-11

## 1. 目标

本设计聚焦于把 Melo 的 daemon 从“可被动拉起的后台服务”提升成“有完整管理面、可观测、可诊断的受管服务”。

本阶段目标是：

- 提供完整的 daemon 管理命令面
- 完善 daemon 生命周期管理
- 增强 daemon 可观测性与诊断输出
- 用更稳的实例识别机制替代“固定路径匹配”假设

本阶段希望覆盖的命令为：

- `melo daemon start`
- `melo daemon status`
- `melo daemon stop`
- `melo daemon restart`
- `melo daemon logs`
- `melo daemon doctor`
- `melo daemon ps`

并补充：

- `status --json`
- `status --verbose`
- `doctor --json`

一句话概括：

把 daemon 从“隐藏在业务命令后面的基础设施”升级成“开发者和用户都能明确管理、观察和诊断的服务对象”。

## 2. 当前现状与问题

当前已经有的能力包括：

- daemon 自动拉起
- daemon 注册文件
- 高位端口自动避让
- `health` API
- `shutdown` API
- `melo daemon stop`
- `melo daemon status`

但当前仍然缺少完整的管理面，主要问题有：

- `start` / `restart` 还没有显式命令
- `status` 只能看注册文件，不能表达更细的运行态
- `logs` 不存在稳定入口
- `doctor` 不存在统一诊断结论
- `ps` 不存在“注册状态与实际进程”的对照输出
- daemon 识别如果继续依赖固定路径，会不适应开发态、安装态和多种启动方式

## 3. 受管 daemon 识别策略

### 3.1 核心要求

“这是 Melo 自己正在管理的 daemon”不能再依赖固定安装路径。

本阶段采用双保险：

- `instance_id` 握手
- `pid + started_at` 进程侧校验

### 3.2 注册文件字段扩展

现有 daemon 注册文件在本阶段建议至少扩展为：

- `instance_id`
- `pid`
- `started_at`
- `base_url`
- `backend`
- `host`
- `port`
- `log_path`
- `version`

### 3.3 `instance_id` 握手

daemon 每次启动时生成一个新的随机 `instance_id`。

该值需要：

- 写入注册文件
- 出现在 `health/status` 响应里

这样 CLI 可以判断：

- 注册文件里的 daemon
- 当前通过 HTTP 命中的 daemon

是不是同一个实例。

### 3.4 `pid + started_at` 双保险

除了 `instance_id`，还保留：

- `pid`
- `started_at`

作为进程侧确认信息。

原因是：

- daemon 可能异常退出，注册文件残留
- pid 未来可能被系统复用
- 单看 pid 不够稳

所以进程匹配逻辑应当是：

- pid 存在
- 进程创建时间与 `started_at` 对得上

### 3.5 结论

本阶段的“受管 daemon”定义为：

- 注册文件存在
- 注册文件中的 `instance_id`、`pid`、`started_at` 可用
- 如果能连通 daemon，则 HTTP 返回的 `instance_id` 与注册文件一致
- 如果不能连通，则进程侧至少还能以 `pid + started_at` 对上

## 4. daemon 生命周期模型

### 4.1 统一状态枚举

建议统一成以下状态：

- `NotRunning`
- `RegisteredButUnhealthy`
- `Running`
- `Stopping`

### 4.2 统一观察结果

建议内部统一一个“daemon 观察结果”结构，至少包含：

- `state`
- `registration_exists`
- `registration_path`
- `base_url`
- `instance_id`
- `pid`
- `started_at`
- `backend`
- `host`
- `port`
- `process_exists`
- `process_start_time_matches`
- `health_ok`
- `log_path`

后续：

- `status`
- `doctor`
- `ps`
- `restart`

都基于这个统一观察结果工作。

## 5. 命令面设计

### 5.1 `melo daemon start`

语义：

- 若 daemon 已在 `Running` 状态：
  - 不重复启动
  - 返回当前实例信息
- 若为 `RegisteredButUnhealthy`：
  - 清理陈旧注册
  - 启动新实例
- 若为 `NotRunning`：
  - 正常启动

要求：

- 启动后必须等到健康检查通过才算成功

### 5.2 `melo daemon status`

语义：

- 默认输出人类可读摘要
- `--json` 输出完整结构化状态
- `--verbose` 增加更多字段

默认展示至少包括：

- state
- pid
- base_url
- backend
- started_at
- healthy / unhealthy

### 5.3 `melo daemon stop`

语义：

- 若 `Running`：
  - 发送优雅关闭请求
  - 等待退出
  - 清理注册
- 若 `NotRunning`：
  - 返回明确“未运行”结果
- 若 `RegisteredButUnhealthy`：
  - 清理陈旧注册并返回结果

### 5.4 `melo daemon restart`

语义：

- 不是简单命令串接
- 应共享同一套观察、停止、等待、重启逻辑

要求：

- stop 后确认旧实例退出
- start 后确认新实例健康

### 5.5 `melo daemon logs`

语义：

- 默认读取 daemon 日志文件尾部
- 默认最近 100 行
- 支持 `--tail N`

本阶段不强制要求实现 `--follow`

### 5.6 `melo daemon doctor`

语义：

- 输出“结论 + 证据”
- 默认输出人类可读诊断
- `--json` 输出结构化诊断项

### 5.7 `melo daemon ps`

语义：

- 从“进程/注册对照”视角展示 daemon

建议重点显示：

- registered pid
- actual pid
- process path
- instance_id
- whether they match

## 6. 可观测性与日志

### 6.1 用户级运行目录

建议 daemon 运行期文件统一放到用户级目录，例如：

- `%LOCALAPPDATA%/melo/daemon.json`
- `%LOCALAPPDATA%/melo/daemon.log`

### 6.2 日志文件

daemon 启动后，日志同时输出到：

- 控制台
- 固定日志文件

### 6.3 `logs` 的最小目标

`melo daemon logs` 本阶段至少做到：

- 读取最近 N 行
- 支持 `--tail`

## 7. `doctor` 的检查项

本阶段建议 `doctor` 至少检查：

1. 注册文件是否存在
2. pid 是否存在
3. pid 的启动时间是否与注册一致
4. health check 是否通过
5. `instance_id` 是否匹配
6. 日志文件是否存在、是否可读

输出建议分级：

- `OK`
- `WARN`
- `FAIL`

## 8. 自动拉起边界

本阶段建议明确区分：

- 观测命令
- 带副作用命令

### 8.1 不自动拉起

以下命令默认不自动拉起 daemon：

- `daemon status`
- `daemon logs`
- `daemon doctor`
- `daemon ps`

### 8.2 可自动恢复

以下命令可触发自动恢复：

- `daemon start`
- `daemon restart`
- 业务入口如 `play` / `tui` / direct-open

## 9. 测试策略

### 9.1 生命周期测试

必须覆盖：

- start 在已运行时不重复拉起
- stop 在未运行时返回清晰结果
- restart 会等待旧实例退出并确认新实例健康

### 9.2 状态与识别测试

必须覆盖：

- `instance_id` 一致时识别为同一实例
- pid 存在但 `started_at` 不一致时判定为不匹配
- 注册存在但 health 不通过时判定为 `RegisteredButUnhealthy`

### 9.3 观测命令测试

必须覆盖：

- `daemon status`
- `daemon status --json`
- `daemon status --verbose`
- `daemon doctor`
- `daemon doctor --json`
- `daemon ps`
- `daemon logs --tail`

## 10. 验收标准

本阶段完成时，应满足以下条件：

- daemon 管理命令面完整可用
- `start/status/stop/restart/logs/doctor/ps` 都有明确语义
- daemon 识别不再依赖固定安装路径
- `instance_id + pid/started_at` 双保险生效
- `status`、`doctor`、`ps` 输出彼此有明确分工
- `logs` 能读取固定日志文件

## 11. 后续扩展点

本设计完成后，可以自然继续推进：

- `logs --follow`
- 多实例 daemon 显式管理
- daemon profile / channel 机制
- 更丰富的 `doctor` 修复建议
- 服务指标与性能采样
