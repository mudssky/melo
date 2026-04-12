# Melo 启动链路 Verbose 与统一日志设计

日期：2026-04-12

## 1. 背景

当前 `melo` 在以下场景中容易给用户造成“卡住了”的感受：

- 裸 `melo`
- `melo <目录>`
- `melo tui`
- 任意需要自动拉起 daemon 的命令

根因不是单一的启动慢，而是当前启动链路缺少可见性：

- 预分发后的默认启动流程会经过 `ensure_running -> direct-open -> TUI 连接`
- CLI 在这些阶段里几乎没有系统化的阶段日志
- daemon 虽然已经有日志文件，但当前命令不会自动把 daemon 侧日志带到终端里
- 当前仓库仅在 `melo daemon status --verbose` 这类观察类命令里提供较多文本信息，而不是统一日志体系
- 一旦启动链路中的某个阶段变慢或失败，用户通常只能看到“没有反应”

与此同时，Melo 已经具备如下基础：

- daemon 运行期文件与日志文件路径约定
- `tracing_subscriber` 初始化
- CLI 与 daemon 的多段启动链路
- `daemon logs`、`daemon status --verbose`
- direct-open、后台补扫、TUI 聚合快照

这说明本次不需要临时补几行 `println!`，而应该把“统一日志内核 + 启动链路 verbose 可见性”正式设计出来。

## 2. 目标

本次设计目标如下：

- 为 Melo 建立统一的日志配置模型，覆盖 `cli` 与 `daemon`
- 提供面向最终用户也可接受的全局 `--verbose` 行为
- 让 `melo --verbose` 在终端中可见地显示启动链路阶段
- 默认情况下把当前命令日志和 daemon 日志一起展示，方便排查跨进程问题
- 保留 daemon 日志文件作为权威留档来源
- 让终端输出保持可读，日志文件保持结构化 JSON
- 允许日志前缀开关与前缀文本配置
- 让日志等级可配置，并支持命令级临时覆盖

## 3. 非目标

本次明确不做：

- 不引入完整的日志聚合服务
- 不把日志实时推送 WebSocket 作为主通道
- 不实现日志可视化 UI
- 不在这一轮引入复杂的日志采样、上报或远端传输
- 不把所有现有错误文案重写成日志系统的一部分
- 不在这一轮完成完整日志轮转策略实现细节以外的运维平台化能力

## 4. 方案结论

本次采用“统一日志内核型”方案：

### 4.1 文件是真相源，终端是优先排查入口

- `cli` 与 `daemon` 都写各自的结构化 JSON 日志文件
- 当前命令终端默认输出人类可读格式
- 当用户执行 `melo --verbose` 时，终端默认把 daemon 日志也一起展示

### 4.2 CLI 与 daemon 不共用同一个长期日志文件

- daemon 使用长期 `daemon.log`
- cli 使用长期 `cli.log`
- 两边通过 `session_id / command_id` 等结构化字段建立关联
- 终端按来源和时间合并展示，不依赖把两类日志永久混写到一个文件中

### 4.3 WebSocket 不是主日志通道

本次不把“CLI 获取 daemon 日志”建立在 WebSocket 主通道上，原因如下：

- daemon 最容易卡住的阶段往往是“尚未健康”之前
- 这时最关键的日志仍然来自文件
- 单独为日志再造一条实时推送协议，收益不如先把文件与终端打通

因此本次主方案是：

- daemon 日志写文件
- CLI 在 `--verbose` 时附带读取 / follow daemon 日志
- 后续若要增强实时通道，可作为附加能力，而不是替代文件兜底

## 5. 用户侧行为

### 5.1 全局日志参数

新增全局日志控制参数：

- `--verbose`
- `--log-level <error|warning|info|debug|trace>`
- `--no-log-prefix`
- `--daemon-log-level <error|warning|info|debug|trace>`

### 5.2 `--verbose` 语义

`--verbose` 的语义不是“略微多一点日志”，而是：

- 当前命令终端进入最详细排查视图
- 当前 CLI 命令自身日志提升到最细粒度
- 默认把 daemon 日志一并带到当前终端
- 但不自动修改 daemon 的长期默认日志等级

### 5.3 `--daemon-log-level` 语义

`--daemon-log-level` 是显式增强开关：

- 用于本次排障时临时要求 daemon 输出更细日志
- 默认不启用
- 优先级高于 daemon 配置默认值
- 只应作用于本次命令触发的排障会话，不应永久污染 daemon 常规运行配置

### 5.4 命令行风格

本次不采用 `-v / -vv / -vvv` 风格，而采用更接近日常用户 CLI 的方案：

- 主入口为 `--verbose`
- `--log-level` 作为精确覆盖
- 不引入多层级短参数语义

### 5.5 前缀行为

前缀仅作用于终端的人类可读输出，不写入 JSON 日志文件。

默认行为：

- 终端前缀默认开启
- `--no-log-prefix` 可关闭
- 配置中可调整前缀开关与前缀文本

示例：

```text
[cli] loading_settings config_path=C:\Users\...\config.toml
[daemon] preparing_database path=C:\Users\...\melo.db
[cli] opening_cwd_directly cwd=D:\Music
```

## 6. 启动链路可见性

### 6.1 需要显式暴露的阶段

对于裸 `melo`、`melo <目录>`、`melo tui` 这类高风险启动链路，终端应至少暴露以下阶段：

- `loading_settings`
- `resolving_base_url`
- `starting_daemon`
- `waiting_for_daemon_health`
- `opening_cwd_directly`
- `opening_explicit_target`
- `connecting_tui_stream`
- `entering_tui`

### 6.2 设计原则

- `--verbose` 只增强可见性，不改变命令语义
- 没带 `--verbose` 时，不把常规终端输出污染成开发者日志面板
- 即使 daemon 日志无法附带，命令本身也不能因此失败

### 6.3 direct-open 场景

对 direct-open 链路要特别强调：

- 进入 direct-open 前显示阶段日志
- 进入 TUI 前的 CLI 文案与 verbose 日志是两条平行通道
- 用户看到的运行时提示与结构化日志应分别承担不同职责

其中：

- 运行时提示负责“面向普通用户的简洁反馈”
- verbose 日志负责“面向排障的详细上下文”

## 7. 配置模型

### 7.1 配置结构

采用三层配置结构：

```toml
[logging]
level = "warning"
terminal_format = "pretty"
file_format = "json"
prefix_enabled = true
cli_prefix = "cli"
daemon_prefix = "daemon"

[logging.cli]
file_enabled = true
file_path = "logs/cli.log"

[logging.daemon]
file_enabled = true
file_path = "logs/daemon.log"
allow_runtime_level_override = true
```

### 7.2 配置含义

全局层 `[logging]`：

- 统一默认等级
- 统一终端输出格式
- 统一文件输出格式
- 统一前缀开关与默认前缀文本

组件层 `[logging.cli]` / `[logging.daemon]`：

- 覆盖文件开关
- 覆盖文件路径
- 覆盖组件级特殊行为，例如 daemon 是否允许运行时提级

### 7.3 优先级

配置优先级建议如下：

1. 命令行参数
2. 组件级 override
3. 全局 `[logging]`
4. 内置默认值

### 7.4 默认值

建议默认值：

- 默认日志等级：`warning`
- 默认终端格式：`pretty`
- 默认文件格式：`json`
- 默认前缀：开启

## 8. 文件布局

### 8.1 文件分离策略

建议默认路径：

- `cli.log`
- `daemon.log`

不建议默认把两类日志长期写到同一个文件中，原因如下：

- daemon 是长生命周期服务
- cli 是短生命周期命令
- 两者保留策略、噪音水平、并发行为都不同

### 8.2 路径解析

日志文件路径应遵循与现有配置路径一致的解析规则：

- 绝对路径直接使用
- 相对路径相对于配置文件目录解析

这样 `logs/cli.log` 与 `logs/daemon.log` 的行为在不同工作目录下都稳定。

## 9. 日志格式

### 9.1 终端格式

终端输出采用人类可读格式，重点保证：

- 一眼可读
- 能快速看到卡在哪个阶段
- 可通过前缀区分 `cli` 与 `daemon`

### 9.2 文件格式

文件输出采用结构化 JSON。

JSON 文件中不写文本前缀，而是写结构化字段，例如：

- `component`
- `level`
- `timestamp`
- `session_id`
- `command_id`
- `target`
- `message`
- `fields`

### 9.3 关联字段

为方便把 `cli.log` 与 `daemon.log` 对齐，建议引入稳定关联字段：

- `session_id`
- `command_id`

原则：

- 当前 CLI 命令启动时生成新的 `command_id`
- 若该命令与 daemon 交互，则把关联标识传递给 daemon 侧日志上下文
- 终端合并展示时优先按时间顺序与来源输出

## 10. daemon 日志如何进入当前终端

### 10.1 主方案

主方案是“文件 follow + 终端合并展示”：

- CLI 自己的 verbose 日志直接实时打印
- daemon 日志从 `daemon.log` 附带进入当前终端
- 终端按来源显示 `[cli]` / `[daemon]`

### 10.2 为什么不是 WebSocket 主方案

不建议把 WebSocket 作为主日志通道，原因如下：

- daemon 最关键的失败阶段可能发生在尚未健康之前
- 这类日志只能稳定从文件中读取
- 单独引入日志实时协议会扩大当前任务范围

### 10.3 增强空间

后续如果需要更强实时体验，可以在 daemon 健康后增加专门实时通道，但这只应作为增强项，不替代文件兜底。

## 11. 错误处理

### 11.1 终端附带 daemon 日志失败

若 CLI 无法成功附带 daemon 日志：

- 不应导致命令本身失败
- 应输出一条简短说明
- 保留原有命令主流程行为

### 11.2 daemon 尚未健康

当 daemon 尚未健康或卡在启动阶段时：

- CLI 仍应尽量输出最近相关 `daemon.log` 片段
- 重点帮助用户判断卡在数据库、端口、健康检查还是 direct-open 阶段

### 11.3 运行时提级限制

若 daemon 配置不允许运行时等级覆盖：

- `--daemon-log-level` 不应强制破坏该策略
- CLI 应告知用户当前未生效的原因

## 12. 参数解析与预分发修正

当前预分发逻辑会把不认识的第一个参数当成路径处理，因此：

- `melo --verbose` 可能被误判为 direct-open

本次设计要求：

- 全局日志参数必须在默认启动 / direct-open / Clap 预分发前被识别
- 预分发逻辑应能正确处理：
  - `melo --verbose`
  - `melo --log-level debug`
  - `melo --verbose D:/Music`
  - `melo --no-log-prefix`

这是本次实现的硬性修复项。

## 13. 测试策略

### 13.1 参数解析测试

必须覆盖：

- `melo --verbose`
- `melo --log-level debug`
- `melo --verbose D:/Music`
- 确保全局日志参数不会被当成 direct-open 路径

### 13.2 配置加载测试

必须覆盖：

- `[logging]`
- `[logging.cli]`
- `[logging.daemon]`
- 前缀开关与前缀文本
- 相对日志路径解析

### 13.3 CLI 行为测试

必须覆盖：

- 裸 `melo --verbose` 会打印启动阶段日志
- direct-open 失败时终端可见卡点
- `--no-log-prefix` 生效

### 13.4 daemon 行为测试

必须覆盖：

- daemon 默认按自身配置等级写文件
- `--daemon-log-level` 仅在允许时生效
- daemon 未健康时仍可通过文件看到关键日志

### 13.5 文件格式测试

必须覆盖：

- `cli.log` / `daemon.log` 是合法 JSON 行
- 至少包含 `component`、`level`、`message`
- 关联字段在跨进程链路中可见

## 14. 验收标准

完成后应满足：

- `melo --verbose` 不再表现为黑盒等待
- 用户能看见当前命令进行到哪个阶段
- daemon 日志可默认并入当前终端排障视图
- 终端输出可读，文件输出结构化
- 前缀默认开启，但可关闭且可配置
- 默认日志等级可配置，默认值为 `warning`
- `--log-level` 与 `--daemon-log-level` 可精确覆盖
- 预分发不会再把全局日志参数误判成路径

## 15. 总结

本次设计不是单纯“给 `melo` 增加一个 `--verbose`”，而是为 Melo 建立一套正式的统一日志基础：

- 配置可管理
- 终端可排障
- 文件可留档
- CLI 与 daemon 可协同观察

它优先解决当前最痛的用户问题：`melo` 卡住时，用户和开发者都应该明确知道它卡在了哪一步。
