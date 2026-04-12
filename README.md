# Melo

Melo 是一个以本地音乐库为核心的 Rust 命令行播放器，采用 `daemon + remote TUI/CLI` 架构：

- `daemon` 是唯一的播放与运行态真相来源
- `CLI` 既可以做远程控制，也可以直接打开文件或目录
- `TUI` 是连接 daemon 的遥控前端，而不是独立播放器

当前仓库已经具备可运行的第一阶段闭环：本地数据库、播放控制、目录/文件直开、临时歌单、后台补扫、TUI 聚合状态流、OpenAPI 文档与 daemon 管理命令。

## 当前已实现

- daemon 生命周期管理：`start / stop / restart / status / logs / doctor / ps / docs`
- 本地播放器控制：播放、暂停、切换、上下曲、音量、静音、循环、随机
- 文件/目录 direct-open：`melo <音频文件>`、`melo <目录>`、裸 `melo`
- 目录直开两阶段扫描：先预热少量曲目进入 TUI，剩余曲目后台补扫
- ephemeral playlist：目录/文件直开会生成可复用的临时歌单
- TUI 远程控制：通过 WebSocket 持续接收播放器状态和后台任务状态
- API 文档：daemon 暴露 `/api/openapi.json` 与 `/api/docs/`
- 配置文件、数据库路径与 daemon 注册文件的用户级管理

## 架构概览

```text
CLI / TUI
    |
    v
 Melo daemon
    |
    +-- PlayerService        播放、队列、进度、模式
    +-- OpenService          文件/目录直开
    +-- RuntimeTaskStore     后台扫描任务状态
    +-- HTTP / WebSocket API 状态与控制面
    |
    v
 SQLite + 本地音频文件
```

几个关键设计约束：

- 播放队列是 daemon 运行态内存数据，不是用户长期资产
- direct-open 会把当前上下文物化为 ephemeral playlist，便于复用与提升
- TUI 读取的是聚合快照 `/api/ws/tui`，不是自己维护播放语义
- 目录直开默认会先同步预热，再后台继续扫描并稳定追加到队列

## 快速开始

### 1. 环境要求

- Rust 工具链
- `pnpm` 10+
- 可用音频后端
  - 默认可使用 `rodio`
  - 如需 `mpv`，请保证 `mpv` 可执行文件在 PATH 中，或在配置里指定 `player.mpv.path`

### 2. 安装

开发态推荐：

```bash
pnpm install
pnpm setup:dev
```

只想本地构建一个可执行文件时：

```bash
cargo install --path . --force
```

### 3. 准备配置

把 [config.example.toml](./config.example.toml) 复制到你的 Melo 配置路径，并按需修改。

默认配置文件位置：

- Windows: `%APPDATA%/melo/config.toml`
- 其他平台: `~/.config/melo/config.toml`

也可以通过环境变量覆盖：

- `MELO_CONFIG_PATH`
- `MELO_CONFIG`

### 4. 启动与使用

启动受管 daemon：

```bash
melo daemon start
```

打开当前目录并进入 TUI：

```bash
melo
```

打开指定目录并进入 TUI：

```bash
melo D:/Music
```

打开单个音频文件：

```bash
melo D:/Music/Aimer/Ref-rain.flac
```

查看当前状态：

```bash
melo status
```

查看 daemon 文档入口：

```bash
melo daemon docs --print
```

## 常用命令

### 播放控制

```bash
melo play
melo pause
melo toggle
melo next
melo prev
melo stop
```

### 模式与音量

```bash
melo player volume 55
melo player mute
melo player unmute
melo player mode show
melo player mode repeat all
melo player mode shuffle on
```

### 队列操作

```bash
melo queue show
melo queue play 0
melo queue remove 3
melo queue move 5 1
melo queue clear
```

### daemon 管理

```bash
melo daemon start
melo daemon status
melo daemon status --json
melo daemon status --verbose
melo daemon stop
melo daemon restart
melo daemon logs --tail 50
melo daemon logs --snapshot --tail 50
melo daemon doctor --json
melo daemon ps
melo daemon docs --print
melo daemon docs --print --openapi
```

### 临时歌单维护

```bash
melo playlist promote D:/Music/blue-bird.mp3 Favorites
melo playlist cleanup
```

## direct-open 行为说明

### 裸 `melo`

当 `open.scan_current_dir = true` 时，裸 `melo` 会：

1. 自动确保 daemon 已运行
2. 把当前目录按 `cwd_dir` 方式 direct-open
3. 先预热少量曲目
4. 输出一条 CLI 扫描提示
5. 进入 TUI，剩余文件继续后台补扫

### `melo <目录>`

- 目录会按 `path_dir` 模式处理
- 会生成或复用对应的 ephemeral playlist
- 后台补扫期间，TUI 顶部会显示单行任务条

### `melo <文件>`

- 文件会按 `path_file` 模式处理
- 当前队列与播放上下文会切换到该文件对应的临时歌单

## 配置重点

完整示例见 [config.example.toml](./config.example.toml)。

几个最常用的配置段：

```toml
[daemon]
host = "127.0.0.1"
base_port = 38123
port_search_limit = 32
docs = "local"

[player]
backend = "auto"
volume = 100
restore_last_session = true
resume_after_restore = false

[open]
scan_current_dir = true
max_depth = 2
prewarm_limit = 20
background_jobs = 4

[tui]
show_footer_hints = true

[templates.runtime.scan]
cli_start = "Scanning {{ source_label }}..."
cli_handoff = "Launching TUI, background scan continues..."
tui_active = "Scanning {{ source_label }}... {{ indexed_count }} / {{ discovered_count }} · {{ current_item_name }}"
tui_done = "Scan complete: {{ queued_count }} tracks indexed"
tui_failed = "Scan failed: {{ error_message }}"
```

说明：

- `daemon.docs`
  - `disabled`：不暴露 API 文档
  - `local`：仅 loopback 可访问
  - `network`：允许网络访问
- `open.prewarm_limit`
  - 进入 TUI 前同步预热的曲目数
- `open.background_jobs`
  - 后台补扫并发度
- `templates.runtime.scan.*`
  - 覆盖扫描阶段的高信息量运行时提示

数据库路径也可以通过环境变量覆盖：

- `MELO_DB_PATH`

## 运行时文件

Melo 会维护几类用户级运行文件：

- 配置文件：`config.toml`
- 数据库：`melo.db`
- daemon 注册：`daemon.json`
- daemon 日志：`daemon.log`

其中 daemon 运行文件路径由 `src/daemon/registry.rs` 决定：

- Windows: `%LOCALAPPDATA%/melo/`
- 其他平台: `~/.local/share/melo/`

也可以用环境变量覆盖 daemon 状态文件位置：

- `MELO_DAEMON_STATE_FILE`

如果你希望所有 CLI 都连接到某个固定地址，也可以直接设置：

- `MELO_BASE_URL`

## API 与文档

daemon 运行后可以访问：

- OpenAPI JSON: `/api/openapi.json`
- Swagger UI: `/api/docs/`

常用方式：

```bash
melo daemon docs --print
melo daemon docs --print --openapi
pnpm docs:api
pnpm docs:api:serve
```

## 开发工作流

安装开发依赖并把本地 `melo` 链接到全局：

```bash
pnpm setup:dev
```

默认假设你已经执行过上面的 `pnpm install`。

常用脚本：

```bash
pnpm qa
pnpm qa:rs
pnpm qa:ts
pnpm lint:rs:fix
pnpm docs:api
```

说明：

- `pnpm qa` 会同时跑 TS 与 Rust 检查
- Rust 侧包含 `cargo fmt`、`clippy -D warnings`、`cargo test -q`
- 仓库当前约定在提交代码前先跑一次 `pnpm qa`

## 项目文档

设计与计划文档主要放在：

- [docs/superpowers/specs](./docs/superpowers/specs)
- [docs/superpowers/plans](./docs/superpowers/plans)

如果你想快速了解最近这批功能，可以先看：

- [2026-04-12-melo-direct-open-background-scan-and-runtime-templates-design.md](./docs/superpowers/specs/2026-04-12-melo-direct-open-background-scan-and-runtime-templates-design.md)
- [2026-04-11-melo-daemon-management-surface-design.md](./docs/superpowers/specs/2026-04-11-melo-daemon-management-surface-design.md)
- [2026-04-11-melo-api-docs-and-response-envelope-design.md](./docs/superpowers/specs/2026-04-11-melo-api-docs-and-response-envelope-design.md)

## 当前边界

README 只描述仓库里已经落地、并且从命令行可感知的能力。项目仍在演进中，尤其是：

- `library` / `config` 命令面目前更像保留入口，不是完整用户工作流
- i18n 与更完整的任务中心 UI 还在后续设计范围
- README 优先强调已可运行路径，而不是覆盖所有内部模块
