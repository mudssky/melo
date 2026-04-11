# Melo Daemon CLI 观测体验与文档访问控制设计

日期：2026-04-12

## 1. 目标

本设计聚焦于改善 Melo 在 daemon 未启动或尚未稳定时的 CLI 体验，并补全日志跟随与 API 文档入口的运行态可用性。

本阶段目标是：

- 让 `melo daemon logs` 默认持续跟随日志
- 为 daemon 未启动时的观察类命令提供统一、友好的提示
- 让 `melo status` 在合适时显示 API 文档 URL
- 增加用于查看文档地址的 CLI 命令
- 为在线文档增加可配置的访问范围控制
- 默认让文档仅本机可访问，同时保留可配置能力

一句话概括：

把 Melo 的 daemon 相关 CLI 从“能用但报错生硬、入口分散”提升成“默认安全、提示友好、文档可发现”的运行态工具面。

## 2. 当前现状与问题

当前已经有以下基础：

- `melo daemon logs` 能读取日志尾部
- `melo status`、`melo queue show`、`melo player mode show` 等命令能通过 HTTP API 查询状态
- daemon 已支持在线 API 文档页面与 OpenAPI JSON
- `package.json` 已有 `docs:api:serve` 之类的开发脚本

当前主要问题是：

- `melo daemon logs` 只能一次性打印，不支持持续跟随
- `melo status` 在 daemon 未启动时会暴露底层接口或发现错误
- 其他观察类命令也存在类似问题，用户看到的是技术细节而不是操作建议
- 当前没有专门的 CLI 命令用于查看运行中 daemon 的文档 URL
- `status` 也不会主动告诉用户文档入口
- 在线文档虽已存在，但缺少明确的运行态访问控制策略

## 3. 命令行为分层

### 3.1 设计原则

不同命令对 daemon 的依赖性质不同，因此“不启动 daemon 时该怎么办”不能一刀切。

本阶段采用“按命令类型分层”的处理方式。

### 3.2 观察类命令

观察类命令默认不自动拉起 daemon。

包括但不限于：

- `melo status`
- `melo queue show`
- `melo player mode show`
- `melo daemon status`
- `melo daemon logs`
- `melo daemon doctor`
- `melo daemon ps`
- `melo daemon docs`

当 daemon 未启动时，这些命令不应直接把底层接口错误暴露给用户，而应输出稳定、友好的提示。

### 3.3 控制类命令

控制类命令允许自动拉起 daemon。

包括但不限于：

- `melo play`
- `melo pause`
- `melo toggle`
- `melo next`
- `melo prev`
- `melo stop`
- `melo player volume`
- `melo player mute`
- `melo player unmute`
- `melo queue clear`
- `melo queue play`

这类命令本身就代表“我要操作播放器”，自动拉起更符合用户预期。

### 3.4 入口类命令

入口类命令继续保持自动拉起行为。

包括：

- 默认启动
- `melo tui`
- direct-open

### 3.5 统一不可用结果

观察类命令需要共享一套 daemon 不可用的解释模型，而不是各自临时拼接字符串。

建议内部统一抽象成类似结果：

- `Running { base_url, docs_url }`
- `Unavailable { reason, hint }`

其中：

- `reason` 用于描述当前不可用原因
- `hint` 用于给出下一步建议，例如 `melo daemon start`

## 4. `melo daemon logs` 设计

### 4.1 默认行为

`melo daemon logs` 默认进入持续跟随模式。

行为应类似：

- 先输出已有的最后 N 行
- 再持续等待日志文件追加内容
- 有新行时即时输出

这比当前“一次性 tail”更符合用户对 logs 命令的直觉。

### 4.2 一次性查看模式

仍需保留一次性查看能力，但不作为默认行为。

建议新增显式开关：

- `--snapshot`

示例：

- `melo daemon logs`
- `melo daemon logs --tail 200`
- `melo daemon logs --snapshot`

### 4.3 daemon 未启动时的行为

当 daemon 未启动时，日志命令不应直接失败。

建议行为如下：

- 如果日志文件存在：
  - 允许读取已有内容
  - 在默认跟随模式下，继续等待文件变化
- 如果日志文件不存在：
  - 输出明确提示，例如当前还没有 daemon 日志，并建议先执行 `melo daemon start`

### 4.4 本阶段边界

本阶段目标是“持续跟随文件变化”，不要求实现更复杂的：

- 日志级别过滤
- 多文件聚合
- 彩色高亮

## 5. 友好提示设计

### 5.1 不暴露底层接口错误

当观察类命令遇到以下情况时，不应原样输出 HTTP/解析/发现层报错：

- 没找到 daemon 注册信息
- 注册存在但 daemon 不可访问
- daemon 健康检查失败

用户看到的应是稳定、面向操作的 CLI 文案。

### 5.2 提示结构

建议友好提示至少包含：

- 当前状态结论
- 简短原因
- 推荐下一步命令

例如可以表达为：

- daemon 未运行
- 当前无法读取播放器状态
- 可执行 `melo daemon start`

### 5.3 覆盖范围

本阶段至少统一到以下代表命令：

- `melo status`
- `melo queue show`
- `melo player mode show`
- `melo daemon docs`

其他观察类命令在实现中可复用同一层逻辑，不要求为每个命令设计完全不同的文案体系。

## 6. 文档 URL 暴露设计

### 6.1 `melo status`

`melo status` 在 daemon 运行且文档功能可访问时，应显示文档 URL。

如果文档被禁用，则应显示：

- `docs: disabled`

如果 daemon 未启动，则使用统一友好提示，不显示无意义的失效 URL。

### 6.2 新增 `melo daemon docs`

建议新增专门命令：

- `melo daemon docs`

语义为：

- 默认直接打开当前 daemon 文档页面
- 若文档禁用，则明确输出禁用状态
- 若 daemon 未启动，则输出统一友好提示

默认打开目标为：

- `/api/docs/`

同时保留脚本化能力，建议支持显式参数：

- `--print`：只打印文档 URL
- `--openapi`：面向 OpenAPI JSON 地址

这样既能减少日常使用时的用户操作，也能保留自动化和复制链接的场景。

### 6.3 与 `docs:api:serve` 的关系

`package.json` 中的 `docs:api:serve` 保留，用于开发脚本层面。

`melo daemon docs` 是运行态 CLI 入口，两者服务的对象不同，不冲突。

## 7. 文档访问控制设计

### 7.1 设计原则

在线文档默认应兼顾：

- 易发现
- 安全保守
- 可配置放开

本阶段默认策略为：

- 文档功能启用
- 但仅允许本机访问

### 7.2 配置模型

不建议使用多个松散布尔值。

建议以单一枚举配置表达文档访问范围，例如：

- `disabled`
- `local`
- `network`

推荐配置形态：

- `daemon.docs = "local"`

其中：

- `disabled`：完全关闭文档入口
- `local`：仅允许 loopback 访问
- `network`：文档跟随 daemon 当前监听地址对外开放

### 7.3 `local` 模式

`local` 模式下，即使 daemon 绑定到了局域网地址，文档也不应自动对局域网开放。

实现目标是：

- 来自本机 loopback 的请求可访问
- 非 loopback 请求拒绝访问文档

这样可以把文档默认暴露面控制在最小范围。

### 7.4 `network` 模式

`network` 模式下，文档随 daemon 服务一起开放。

适用于：

- 局域网调试
- 多设备联调
- 前端在其他机器上访问当前 daemon

### 7.5 `disabled` 模式

`disabled` 模式下：

- `/api/docs`
- `/api/openapi.json`

都不可访问。

同时：

- `melo status` 显示 `docs: disabled`
- `melo daemon docs` 输出禁用提示

## 8. 配置与状态输出

### 8.1 配置来源

文档访问范围建议进入现有 daemon 配置体系，由 `Settings` 统一读取。

本阶段只要求支持静态配置，不要求：

- 动态热更新
- 运行时开关切换

### 8.1.1 路径锚点策略

本阶段同时收敛配置文件路径和数据库路径的解析策略，避免 direct-open / autostart 等场景下因为当前工作目录不同而解析出错误路径。

设计原则如下：

- 配置文件路径和数据库路径都必须支持单独覆盖
- 一旦 `Settings` 加载完成，关键运行时路径应尽量被解析为绝对路径
- 配置文件中的相对路径应相对“配置文件所在目录”解析，而不是相对当前 shell 工作目录

建议覆盖优先级如下：

- 配置文件路径：
  - CLI 参数
  - 环境变量 `MELO_CONFIG_PATH`
  - 平台默认 Melo 根目录中的 `config.toml`
- 数据库路径：
  - CLI 参数
  - 环境变量 `MELO_DB_PATH`
  - 配置文件中的 `database.path`
  - Melo 根目录中的默认数据库文件

### 8.1.2 默认 Melo 根目录

默认不使用 `$HOME/.melo` 或平台目录下的 `.melo`。

建议所有平台都使用“平台标准位置中的 `melo` 根目录”：

- Linux：平台标准配置位置下的 `melo`
- macOS：平台标准应用支持目录下的 `melo`
- Windows：平台标准用户应用目录下的 `melo`

在该根目录下，默认放置：

- `config.toml`
- `melo.db`

设计上不强制把配置和数据库拆到两个不同目录，目标是优先保持：

- 易理解
- 易排查
- 易迁移

### 8.1.3 相对路径解析规则

如果配置文件中写了相对路径，例如：

- `database.path = "melo.db"`
- 或其他未来文件路径配置

则这些相对路径应一律相对配置文件所在目录解析。

这样可以保证：

- 前台运行
- 后台 daemon 运行
- direct-open 自动拉起

都使用同一套稳定路径解析结果，而不受当前命令执行目录影响。

### 8.2 状态输出

`melo status` 中的文档相关输出建议至少体现：

- 文档状态
- 文档 URL（若可访问）

例如：

- `docs: http://127.0.0.1:PORT/api/docs/`
- 或 `docs: disabled`

### 8.4 文档命令的 URL 策略

`melo daemon docs` 默认应打开 Swagger 文档页面，而不是 OpenAPI JSON。

原因是：

- 页面更适合人工浏览
- 更符合“减少用户操作”的目标
- OpenAPI JSON 更适合脚本或工具链消费

因此默认行为为：

- 打开 `/api/docs/`

只有在显式指定 `--openapi` 时，才面向 `/api/openapi.json`。

### 8.3 URL 选择原则

在 `local` 模式下，即使 daemon 当前注册地址不是 loopback，也建议对 CLI 展示一个本机访问形式的文档 URL。

这样用户看到的地址应与“仅本机可访问”的安全策略一致。

## 9. 本阶段实现边界

### 9.1 本阶段必须完成

- `melo daemon logs` 默认持续跟随
- 增加一次性查看模式
- 观察类命令在 daemon 不可用时输出友好提示
- `melo status` 显示文档 URL 或文档状态
- 新增 `melo daemon docs`
- 为在线文档增加 `disabled` / `local` / `network` 配置
- 默认文档策略为 `local`

### 9.2 本阶段不做

- 不改 HTTP API 协议
- 不改 WebSocket 协议
- 不引入认证或 token
- 不实现更细粒度的文档访问控制
- 不重做整个 CLI 输出风格

## 10. 测试策略

本阶段应至少覆盖以下验证：

- `melo daemon logs` 默认进入跟随模式
- `melo daemon logs --snapshot` 只输出一次
- daemon 未启动时 `melo status` 输出友好提示
- daemon 未启动时至少一个其他观察类命令复用同一提示逻辑
- `melo daemon docs` 在 `local` 模式下输出本机文档 URL
- `melo daemon docs` 默认会打开 Swagger 文档页面
- `melo daemon docs` 在 `disabled` 模式下输出禁用提示
- 文档路由在 `local` 模式下拒绝非本机访问
- `melo status` 在 daemon 运行时显示 docs URL 或 docs 状态

## 11. 验收标准

本阶段完成时，应满足以下条件：

- 用户在 daemon 未启动时，不再看到生硬的接口错误
- `melo daemon logs` 默认可持续查看日志
- 文档默认启用但仅本机可访问
- 文档访问范围可通过配置切换
- `melo status` 能直接告诉用户文档地址或禁用状态
- 存在独立命令查看文档地址

## 12. 后续扩展点

本设计完成后，可以自然继续推进：

- `melo daemon logs` 支持更丰富的筛选和高亮
- 文档访问增加更细粒度鉴权
- 观察类命令增加统一 JSON 输出模式
- 将 daemon 可用性提示进一步抽象成可复用的 CLI 诊断层
