# Melo Daemon 注册、广格式打开与 TUI 帮助设计

日期：2026-04-11

## 1. 目标

本设计聚焦于修复 Melo 当前“能跑但不像一个可直接使用的本地播放器”的几类关键体验问题，并为后续扩展打下稳定基础。

本阶段目标是：

- 让 `melo` / `melo <path>` / `melo play` / `melo tui` 不再依赖固定常用端口
- 让 daemon 使用“可配置首选端口 + 自动避让 + 全局注册发现”的机制稳定运行
- 让 direct-open 正式支持大小写不敏感的音频扩展名，并纳入 `m4a` / `aac`
- 让 direct-open 失败时返回可解释错误，而不是笼统的 HTTP 400
- 让当前目录扫描不再静默吞错
- 让 TUI 不再像“空白页”，至少能看到 queue/来源/状态
- 让 `?` 打开帮助弹层，底部只保留极少数常用快捷键提示，且可配置关闭
- 把当前内建播放能力提升为正式“可选后端”体系，并在本阶段提供一个可运行的 `mpv` 后端

一句话概括：

把 Melo 从“基于固定端口和单后端假设的半成品入口”推进到“具备可发现 daemon、广格式 direct-open、可选播放后端和可理解 TUI 的本地播放器”。

## 2. 当前现状与问题

当前实现已经具备 daemon、direct-open、ephemeral playlist、真实 TUI 运行循环等基础，但仍存在几个影响实际使用的核心缺口：

- daemon 当前仍默认使用常见端口，端口占用时会直接报 `os error 10048`
- 远端命令与 TUI 发现 daemon 的逻辑仍然过于依赖固定地址
- direct-open 与库扫描对扩展名大小写处理不一致
- 当前支持格式只覆盖 `flac/mp3/ogg/wav`
- `m4a/aac` 尚未纳入产品支持范围
- CLI 对 `/api/open` 的失败只暴露成 HTTP 400，缺少稳定错误契约
- 裸 `melo` 当前目录扫描失败时会被静默吞掉，导致用户只看到一个空的 TUI
- TUI 当前主内容区几乎只显示一条“Now Playing”，即使成功打开了多首歌也很难感知
- `?` 与 `/` 已有按键映射，但帮助和搜索还没有形成真实可用的交互
- 当前播放后端虽然已有 `PlaybackBackend` 抽象，但产品层仍然等价于“写死 rodio”

这些问题组合在一起时，用户会明显感受到：

- `melo daemon` 撞端口
- `melo play` 还在连旧端口
- `melo <file>` 明明是支持格式却直接 400
- 目录里很多 `flac` 但 TUI 里看起来“没内容”

## 3. 备选方向与推荐

### 3.1 方案 A：最小修补

做法：

- 仅修扩展名判断和 direct-open 错误正文
- 把默认端口从 `8080` 改成某个高位端口
- 增加一个简单帮助弹层

优点：

- 改动最少
- 实现最快

缺点：

- 仍然依赖单一固定端口
- daemon/TUI/CLI 之间仍缺统一发现机制
- 后端仍然是事实上的“写死 rodio”

### 3.2 方案 B：推荐方案，统一 daemon 注册 + 可选播放后端

做法：

- 为 daemon 引入全局状态注册文件
- 默认使用可配置高位端口，冲突时自动扫描备用端口
- 所有 CLI/TUI 都通过注册文件发现同一个 daemon
- 把 `PlaybackBackend` 从“内部抽象”提升为正式产品能力
- 保留 `rodio` 后端并新增可运行的 `mpv` 后端
- 统一 direct-open、格式判断、错误契约、TUI 帮助和可见性

优点：

- 能同时解决端口冲突、daemon 发现、广格式支持和 TUI 可理解性
- 保持现有结构的连续性，不需要重写控制面
- 为后续继续接入其它后端保留边界

缺点：

- 比最小修补多出 daemon 注册状态与后端装配层

### 3.3 方案 C：彻底改为外部播放器优先架构

做法：

- 直接把 `mpv` 作为默认主后端
- 内建后端退居备选
- 端口、daemon、TUI 等一起围绕外部后端重组

优点：

- 广格式支持最直接
- 可较快获得稳定解码能力

缺点：

- 默认运行体验依赖外部程序安装
- 会把“当前代码渐进升级”做成“运行模型切换”

### 3.4 推荐结论

本阶段选择 **方案 B：统一 daemon 注册 + 可选播放后端**。

## 4. 本阶段范围

### 4.1 纳入本阶段

- daemon 全局注册与发现机制
- 首选高位端口配置与自动避让
- `melo` / `melo <path>` / `melo play` / `melo status` / `melo tui` 统一 daemon 发现
- direct-open 与媒体库扫描的统一扩展名判断
- 大小写不敏感扩展名支持
- `m4a` / `aac` 产品支持
- `mpv` 后端正式接入并可运行
- 裸 `melo` 当前目录扫描错误可见化
- TUI 帮助弹层与底部极简提示
- TUI 主内容区最小可见列表
- 相应配置、测试与错误契约更新

### 4.2 不纳入本阶段

- 搜索弹层的完整实现
- 复杂 TUI 焦点移动与多列浏览
- 图片/封面顶层直接打开
- 多 daemon 实例的显式切换 UI
- 播放历史与最近打开面板

## 5. daemon 注册与端口策略

### 5.1 基本原则

daemon 不再默认假设固定监听 `127.0.0.1:8080`。

新的模型是：

- daemon 启动后注册自己的实际地址
- 客户端通过注册状态发现 daemon
- 若状态失效，则自动恢复

### 5.2 注册文件位置

注册文件应放在 **用户级全局运行时目录**，而不是项目目录。

Windows 推荐路径：

- `%LOCALAPPDATA%/melo/daemon.json`

原因：

- 用户会在不同目录下执行 `melo`
- 若把状态文件放在项目目录或当前目录，不同音乐目录无法共享同一个后台

### 5.3 注册文件内容

建议最小字段：

- `base_url`
- `pid`
- `started_at`
- `version`
- `backend`
- `host`
- `port`

可选扩展字段：

- `config_path`
- `state_file_version`

### 5.4 监听策略

新增 daemon 配置域，例如：

```toml
[daemon]
host = "127.0.0.1"
base_port = 38123
port_search_limit = 32
```

启动时行为为：

1. 尝试绑定 `host + base_port`
2. 若已占用，则继续尝试 `base_port + 1 .. base_port + port_search_limit`
3. 成功后写入注册文件

### 5.5 客户端发现规则

所有 CLI/TUI 入口统一遵循：

1. 读取注册文件
2. 使用其中的 `base_url` 做 health check
3. 若健康，直接复用
4. 若不健康，删除陈旧注册并重新拉起 daemon

适用入口包括：

- `melo`
- `melo <path>`
- `melo play`
- `melo pause`
- `melo status`
- `melo tui`
- 其它遥控命令

### 5.6 `MELO_BASE_URL` 的角色

环境变量 `MELO_BASE_URL` 不再作为日常主发现机制，而是保留为：

- 显式调试覆盖项
- 测试环境注入项

优先级建议为：

1. 显式 `MELO_BASE_URL`
2. 注册文件中的 `base_url`
3. 配置中的 `daemon.host + daemon.base_port`

## 6. 播放后端架构

### 6.1 正式支持多后端

当前已有 `PlaybackBackend` trait，本阶段正式把它提升为产品级抽象。

本阶段支持两个后端：

- `rodio`
- `mpv`

本阶段不纳入实现，但保留为后续候选的后端：

- `libVLC`

本阶段明确不采用的方向：

- 直接把 `ffplay` 作为正式可控后端
- 基于 FFmpeg 低层绑定在本阶段自行拼装完整播放器后端

原因是：

- `mpv` 已能提供更成熟的受控播放器能力与 IPC 控制面
- `rodio` 已能承担内建轻量后备后端
- 在本阶段同时引入第三类正式后端会显著放大范围

### 6.2 配置方式

新增配置项：

```toml
[player]
backend = "auto"
volume = 100
restore_last_session = true
resume_after_restore = false
```

并新增：

```toml
[player.mpv]
path = "mpv"
ipc_dir = "auto"
extra_args = []
```

其中 `backend` 支持：

- `auto`
- `rodio`
- `mpv`

### 6.3 默认后端解析策略

本阶段推荐默认值为：

- `player.backend = "auto"`

解析规则为：

1. 若本机存在可用 `mpv`，优先使用 `mpv`
2. 否则回退到 `rodio`

这样可以同时兼顾：

- 对 `m4a/aac` 的广格式实际可运行支持
- 无外部依赖环境下仍可启动

这意味着本阶段的“默认高兼容性后端”语义是：

- 若可用，则默认使用 `mpv`
- 若不可用，则自动回退到 `rodio`

若用户显式指定：

- `player.backend = "mpv"`

则不允许静默回退到 `rodio`，必须在 `mpv` 缺失时直接报错。

### 6.4 `rodio` 后端职责

`rodio` 后端继续作为：

- 内建后备后端
- 无外部程序依赖时的基础实现

它在本阶段的主要价值是：

- 作为 `auto` 的兜底
- 保障基础格式在无外部依赖环境下仍可运行
- 继续服务测试和最小运行场景

但本设计明确承认：

- 当前依赖配置主要覆盖 `flac/mp3/vorbis/wav`
- `AAC/M4A` 的广格式体验不应强行完全压在本阶段的 `rodio` 上
- 当 `auto` 回退到 `rodio` 时，若用户打开 `m4a/aac` 失败，错误正文必须明确指出当前后端不支持并建议切换到 `mpv`

### 6.5 `mpv` 后端职责

`mpv` 后端在本阶段必须是 **可运行实现**，而不是只做抽象和空壳。

其完成标准为：

- 支持加载并播放文件
- 支持暂停/恢复/停止
- 支持查询当前播放位置
- 支持设置音量
- 支持感知自然播完并向控制层发送结束事件

它在本阶段承担“高兼容性主后端”的职责。

### 6.6 `mpv` 控制方式

建议使用持久化控制连接，而不是一次性 CLI 调用。

原因：

- 需要持续读取播放位置
- 需要稳定接收播完事件
- 需要控制 pause/resume/stop/volume

Windows 下推荐使用 `mpv` 的 IPC 机制，并由 Melo 后端维护会话级控制连接。

### 6.7 缺失后端时的行为

若配置选择 `mpv`，但本机没有可用 `mpv`：

- daemon 启动时直接报错
- 输出明确错误，不静默回退

只有未来显式设计 fallback 时，才允许自动回退。

若用户显式选择：

- `player.backend = "rodio"`

则系统尊重该选择，不主动切换到 `mpv`。

## 7. 格式支持策略

### 7.1 本阶段产品支持格式

direct-open 和目录发现本阶段正式支持：

- `flac`
- `mp3`
- `ogg`
- `wav`
- `m4a`
- `aac`

### 7.2 大小写不敏感

所有扩展名判断统一改为大小写不敏感。

例如：

- `.flac`
- `.FLAC`
- `.FlAc`

都视为同一种格式。

### 7.3 统一判断位置

扩展名支持逻辑必须在以下两层共用同一实现：

- direct-open 目标识别
- library 扫描路径过滤

不允许一层支持、一层漏掉。

### 7.4 后端与格式能力的关系

产品层“允许打开”与具体后端“能否稳定播放”分开表达：

- 入口层：决定某路径是否是支持的音频目标
- 后端层：决定当前选定后端是否能真正播放

若后端层失败，应给出明确错误，而不是回落到 HTTP 400。

对于 `m4a/aac`，本阶段的正式可运行保证由 `mpv` 后端承担。

## 8. direct-open 行为修复

### 8.1 显式路径打开

执行：

- `melo <音频文件>`
- `melo <目录>`

时，若失败应直接退出并打印具体错误。

不再只显示：

- `HTTP status client error (400 Bad Request)`

### 8.2 当前目录扫描

执行裸 `melo` 时：

- 若当前目录没有音频，不是错误，仍进入 TUI
- 若当前目录扫描或打开失败，也允许进入 TUI，但必须在状态区显示错误

### 8.3 裸 `melo` 不能再吞错

当前目录打开动作失败不能再通过 `.ok()` 静默吞掉。

这类错误至少需要被：

- 状态栏展示
- 或帮助/提示区明确描述

### 8.4 错误码契约

建议统一稳定错误码：

- `unsupported_open_format`
- `open_target_not_found`
- `open_target_empty`
- `open_scan_failed`
- `open_backend_unavailable`
- `open_decode_failed`

CLI 应优先打印这些错误的明确正文。

## 9. TUI 可见性与帮助

### 9.1 主内容区最小目标

本阶段 TUI 不再满足于只显示一条 `Now Playing`。

主内容区至少应显示：

- 当前 queue 中的曲目列表
- 当前播放项标记
- 无队列时的显式空状态

### 9.2 状态区

状态区至少显示：

- 当前播放状态
- 当前来源 `source=...`
- 当前后端 `backend=...`
- 最近错误摘要（如存在）

### 9.3 帮助弹层

按 `?` 打开帮助弹层。

帮助弹层至少展示：

- Playback
  - `Space`
  - `>`
  - `<`
- General
  - `?`
  - `q`
- Context
  - 当前来源说明
  - 当前后端说明

再次按 `?` 或按 `Esc` 关闭。

### 9.4 `q` 的语义

- 若帮助弹层打开：先关闭帮助弹层
- 若帮助弹层未打开：退出 TUI

### 9.5 底部极简提示

底部只保留极少数高频提示，例如：

- `Space Play/Pause`
- `? Help`
- `q Quit`

并新增配置：

```toml
[tui]
show_footer_hints = true
```

当关闭时：

- 底部不再显示快捷键提示
- 帮助弹层仍必须可用

### 9.6 `/` 的处理

本阶段不实现真实搜索弹层。

因此建议：

- 不在帮助弹层中展示 `/`
- 不把它当成已可用能力对外宣传

## 10. 配置设计

### 10.1 新增配置项

建议最终配置形态至少包含：

```toml
[daemon]
host = "127.0.0.1"
base_port = 38123
port_search_limit = 32

[player]
backend = "auto"
volume = 100
restore_last_session = true
resume_after_restore = false

[player.mpv]
path = "mpv"
ipc_dir = "auto"
extra_args = []

[open]
scan_current_dir = true
max_depth = 2
prewarm_limit = 20
background_jobs = 4

[tui]
show_footer_hints = true

[playlists.ephemeral]
default_ttl_seconds = 0
```

### 10.2 默认值原则

- daemon 使用用户级本地 loopback，不暴露公网
- 首选端口默认高位且不常见
- 播放后端默认使用 `auto`，优先选择 `mpv`，否则回退到 `rodio`
- TUI 底部提示默认开启
- 会话恢复与音量配置继续沿用当前直觉默认值

## 11. 测试策略

### 11.1 daemon 注册与发现

必须覆盖：

- 首选端口可配置
- 首选端口占用时自动避让
- 注册文件写入与读取
- 注册文件陈旧时自动清理并重启
- `melo play` / `melo status` / `melo tui` 统一走注册发现

### 11.2 格式支持

必须覆盖：

- 大小写不敏感扩展名
- `m4a` / `aac`
- unsupported 格式仍返回稳定错误
- direct-open 与扫描层共享同一格式判断

### 11.3 后端装配

必须覆盖：

- `player.backend = auto`
- `player.backend = rodio`
- `player.backend = mpv`
- `auto` 在有 `mpv` 时优先选择 `mpv`
- `auto` 在无 `mpv` 时回退到 `rodio`
- `mpv` 缺失时报错
- 不同后端在 `PlayerService` 上仍满足统一行为契约

### 11.4 TUI

必须覆盖：

- `?` 打开帮助弹层
- `?` 或 `Esc` 关闭弹层
- `q` 在弹层打开时优先关闭弹层
- 底部快捷键提示可配置关闭
- 主内容区能看到 queue 列表，不再是纯空白

### 11.5 错误展示

必须覆盖：

- 显式 `melo <file>` 打开失败时打印具体错误
- 裸 `melo` 当前目录扫描失败时进入 TUI 且显示错误状态

## 12. 验收标准

本阶段完成时，应满足以下条件：

- `melo daemon` 不再依赖常见固定端口
- 默认首选端口可配置，冲突时自动避让
- daemon 地址通过用户级全局注册文件发现
- `melo` / `melo play` / `melo status` / `melo tui` 能稳定找到同一个后台
- direct-open 支持大小写不敏感的 `flac/mp3/ogg/wav/m4a/aac`
- 当前目录扫描不再静默吞错
- `melo <path>` 失败时能看到明确错误，不再只是 HTTP 400
- `mpv` 后端可实际运行
- `rodio` 与 `mpv` 通过统一后端抽象接入
- TUI 可以看到 queue 列表、来源、后端与错误状态
- `?` 帮助弹层可用
- 底部极简提示可配置关闭

## 13. 后续扩展点

本设计完成后，可自然继续推进：

- TUI 搜索弹层
- 更完整的队列焦点移动与选中播放
- 多 daemon 实例的显式管理
- 更多播放后端
- `libVLC` 作为后续高兼容性候选后端的评估与接入
- 后端能力矩阵展示
- 图片/封面查看能力
