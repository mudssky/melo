# Melo 一期设计文档

日期：2026-04-09

## 1. 项目定位

Melo 是一个受 beets 启发的本地音乐工具，但一期目标不是自动标签整理，而是围绕以下闭环构建稳定能力：

- 本地音乐库扫描与入库
- 元数据、歌词、封面读取
- 静态歌单与智能歌单
- 文件组织规则
- daemon 常驻播放
- TUI 终端遥控

Melo 的核心不是替代 beets 的 autotag 能力，而是形成 `library + playlist + organize + playback` 的一体化工作流。

## 2. 一期架构总览

一期采用 `daemon + client` 架构。

- `daemon` 是唯一长期存活进程，也是唯一稳定播放端
- `tui` 是纯遥控客户端，不包含播放引擎和数据库逻辑
- `cli` 同时承担高频遥控和结构化管理入口
- `web` 后续支持 `daemon/browser` 双输出，但不进入一期实现

高层职责边界如下：

- `playlist` 决定有哪些歌
- `queue` 决定现在按什么顺序播放
- `organize` 决定文件放在哪
- `player` 决定怎么播放
- `tui` 决定怎么控制

### 2.1 一期依赖落地要求

一期不要求 `Cargo.toml` 中的所有依赖都已经被接入，但要求“核心运行链路”上的依赖必须真正投入使用，不能只停留在声明状态。

#### 2.1.1 一期必须真正落地的依赖

- `sea-orm`
- `sea-orm-migration`
- `lofty`
- `rodio`
- `tokio-tungstenite`
- `tower-http`
- `unicode-width`
- `mime_guess`
- `tracing`
- `tracing-subscriber`

这些依赖分别对应：

- `sea-orm` / `sea-orm-migration`
  一期数据库访问与 schema migration 的正式实现。`rusqlite` 不应继续作为生产路径上的主持久化层。
- `lofty`
  音频文件元数据、歌词、封面读取的正式实现。
- `rodio`
  daemon 内真实播放后端，而不是仅保留空实现或测试替身。
- `tokio-tungstenite`
  TUI 作为 Rust 客户端连接 daemon WebSocket 的正式实现。
- `tower-http`
  daemon 的 trace、基础中间件、必要的跨域与通用 HTTP 层配置。
- `unicode-width`
  TUI 中 CJK 歌名、专辑名、歌词等文本宽度计算。
- `mime_guess`
  封面 sidecar 和 artwork 响应的 MIME 推断。
- `tracing` / `tracing-subscriber`
  结构化日志入口、过滤与输出配置。

#### 2.1.2 已经进入主链路、无需额外强调的依赖

- `axum`
- `clap`
- `config`
- `crossterm`
- `reqwest`
- `minijinja`
- `serde`
- `serde_json`
- `thiserror`
- `tokio`
- `walkdir`

#### 2.1.3 可以保留但不作为一期必须项的依赖

- `inquire`
- `utoipa`

这两者可以保留在依赖清单中，但不要求在一期完成前必须有正式接入。

#### 2.1.4 持久化层收敛原则

一期的数据库层必须收敛到 `SeaORM` 体系：

- repository 层不再继续扩展新的 `rusqlite` 直连实现
- schema 和 migration 以 `sea-orm-migration` 为主
- 若测试或脚本仍暂时使用 `rusqlite`，其职责应局限于测试辅助或独立迁移脚本，而不是业务主链路

## 3. CLI 信息架构

一期 CLI 采用“少量一级快捷命令 + 领域二级命令”的混合结构。

### 3.1 一级快捷命令

- `melo play`
- `melo pause`
- `melo toggle`
- `melo next`
- `melo prev`
- `melo stop`
- `melo status`
- `melo tui`

这些命令只承担高频遥控职责，保持短路径和脚本友好。

### 3.2 一级领域命令

- `melo daemon`
- `melo library`
- `melo queue`
- `melo playlist`
- `melo db`
- `melo config`

### 3.3 建议命令面

```bash
melo play
melo pause
melo next
melo prev
melo status
melo tui

melo daemon start|stop|restart|status

melo library scan
melo library list
melo library search <query>
melo library show <id>
melo library stats
melo library organize --dry-run
melo library organize --apply

melo queue show
melo queue add <song-id...>
melo queue remove <index>
melo queue move <from> <to>
melo queue clear
melo queue play <index>

melo playlist list
melo playlist show <name>
melo playlist create <name>
melo playlist delete <name>
melo playlist rename <old> <new>
melo playlist add <name> <song-id...>
melo playlist remove <name> <song-id...>
melo playlist load <name> [--append|--replace] [--play]
melo playlist preview <name>
melo playlist sync [name]

melo db path
melo db init
melo db migrate
melo db doctor
melo db vacuum
melo db backup [dest]

melo config show
melo config edit
melo config validate
```

### 3.4 关键边界

- `queue` 是 daemon 运行态，不是用户资产
- `playlist` 是用户资产，分 `static` 和 `smart` 两类
- `smart playlist` 定义源在配置文件中，不通过 CLI 创建
- `organize` 是独立能力，不从属于歌单

### 3.5 CLI 帮助文档要求

一期 CLI 不仅要有命令，还要提供可用的帮助文档。要求如下：

- 顶层命令和每个一级领域命令都必须提供明确的 `about`
- 对于行为复杂的命令，应补充 `long_about`
- 关键参数和子命令必须有可读的帮助文本，避免只暴露裸名字
- `melo --help`、`melo <command> --help`、`melo <command> <subcommand> --help` 都应输出结构化帮助
- 帮助文本中要明确区分：
  - `queue` 是运行态
  - `playlist` 是用户资产
  - `organize` 是文件组织能力
- 对用户最常用的命令，应在帮助文本中加入简短示例，例如：
  - `melo status`
  - `melo library scan`
  - `melo playlist load Favorites --play`

## 4. `melo db` 的职责

`melo db` 是低频维护命名空间，不是日常使用主入口。

原则如下：

- 日常入库走 `melo library scan`
- 数据库初始化和迁移默认由 daemon 启动时自动执行
- `melo db` 只负责维护、诊断、备份和手动修复

保留命令：

- `melo db path`
- `melo db init`
- `melo db migrate`
- `melo db doctor`
- `melo db vacuum`
- `melo db backup [dest]`

不做内建命令：

- 不做 `melo db import-beets`
- 不做 `melo db query`
- 不做高风险 `melo db reset`

beets 迁移通过仓库脚本提供，例如：

- `scripts/beets_to_melo.py`

该脚本负责从 beets SQLite 数据库迁移基础库数据到 Melo 数据库，不作为长期稳定 CLI API。

## 5. 数据库模型

Melo 一期数据库参考 beets 的 `items/albums` 思路，但不照搬全字段。只保留一期真正要使用的核心字段。

### 5.1 核心表

#### `artists`

- `id`
- `name`
- `sort_name`
- `search_name`
- `created_at`
- `updated_at`

#### `albums`

- `id`
- `title`
- `album_artist_id`
- `year`
- `source_dir`
- `created_at`
- `updated_at`

#### `songs`

- `id`
- `path`，唯一
- `title`
- `artist_id`
- `album_id`
- `track_no`
- `disc_no`
- `duration_seconds`
- `genre`
- `lyrics`
- `lyrics_source_kind`，`none | embedded | sidecar`
- `lyrics_source_path`
- `lyrics_format`，`plain | lrc`
- `lyrics_updated_at`
- `format`
- `bitrate`
- `sample_rate`
- `bit_depth`
- `channels`
- `file_size`
- `file_mtime`
- `added_at`
- `scanned_at`
- `organized_at`
- `last_organize_rule`
- `updated_at`

#### `playlists`

- `id`
- `name`，唯一
- `description`
- `created_at`
- `updated_at`

#### `playlist_entries`

- `id`
- `playlist_id`
- `song_id`
- `position`
- `added_at`

说明：

- 允许同一首歌在同一歌单中重复出现
- 使用独立 `id`，避免成员记录过于刚性

#### `artwork_refs`

- `id`
- `owner_kind`，`album | song`
- `owner_id`
- `source_kind`，`embedded | sidecar`
- `source_path`
- `embedded_song_id`
- `mime`
- `cache_path`
- `hash`
- `updated_at`

### 5.2 一期明确不入库的内容

- queue 持久化表
- smart playlist 定义表
- beets 的大量 `mb_*`、`discogs_*`、flex attributes
- 原始封面 BLOB
- 结构化 LRC 时间轴

### 5.3 索引建议

- `songs(path)` 唯一索引
- `songs(artist_id)`
- `songs(album_id, disc_no, track_no)`
- `artists(search_name)`
- `albums(album_artist_id, title)`
- `playlist_entries(playlist_id, position)`

## 6. 媒体资源模型

### 6.1 歌词

歌词统一入库，便于：

- 基于数据库搜索歌词
- TUI 直接展示
- 后续 Web 统一读取

数据库中保留：

- 规范化后的歌词文本
- 歌词来源类型
- sidecar 路径
- 歌词格式

对于 `.lrc`：

- 一期将纯文本内容入库到 `songs.lyrics`
- 保留 `lyrics_format = "lrc"`
- 保留 `lyrics_source_path`
- 不做时间轴结构化入库
- 播放时如需同步效果，可运行时解析原始 sidecar 文件

### 6.2 封面

封面不存原始 BLOB，而是存来源引用和缓存元数据。

封面来源支持：

- 音频内嵌封面
- 文件外 sidecar 封面

缓存策略：

- daemon 负责生成并维护缩略图缓存
- TUI 和未来 Web 统一走 daemon 获取封面

### 6.3 来源优先级

歌词和封面的来源优先级均为可配置，不写死。

```toml
[library.assets]
cover_priority = ["sidecar", "embedded"]
lyrics_priority = ["sidecar_lrc", "sidecar_txt", "embedded"]
cover_names = ["cover", "folder", "front", "album", "art"]
cover_extensions = ["jpg", "jpeg", "png", "webp"]
lyrics_patterns = ["{stem}.lrc", "{stem}.txt"]
```

### 6.4 sidecar 行为

- 与歌曲同名的歌词 sidecar 会跟随歌曲移动和重命名
- 专辑目录共享封面不会因单曲移动而被强制一起移动
- 当按专辑单位执行组织时，可一并处理专辑目录级封面

## 7. 歌单模型

歌单分为两类：

- `static playlist`
- `smart playlist`

### 7.1 static playlist

- 定义和成员均存数据库
- 支持 CRUD
- 支持加载到当前 queue

### 7.2 smart playlist

- 定义源在配置文件
- 运行时解析成查询结果
- 支持 `list/show/preview/load`
- 不在一期中作为 `organize` 的物理路径决策依据

### 7.3 对外统一视图

虽然定义源不同，但 CLI、TUI、API 均统一暴露：

- `name`
- `kind`
- `count`
- `preview`
- `load`

这使得用户可以把 static 和 smart playlist 视作统一的“歌单对象”。

## 8. 文件组织 `organize`

`organize` 是独立于歌单的文件组织能力。歌单只可能作为规则匹配条件之一。

### 8.1 配置结构

```toml
[library.organize]
enabled = true
base_dir = "D:/MusicLibrary"
conflict_policy = "first_match"
dry_run_by_default = true

[[library.organize.rules]]
name = "anime-songs"
priority = 100
match = { static_playlist = "AnimeSongs" }
template = "AnimeSongs/{{ title|sanitize }}{% if artist %} - {{ artist|sanitize }}{% endif %}"

[[library.organize.rules]]
name = "karaoke"
priority = 90
match = { static_playlist = "Karaoke" }
template = "Karaoke/{{ title|sanitize }} - {{ duration_seconds|mmss }}"

[[library.organize.rules]]
name = "singleton"
priority = 10
match = { singleton = true }
template = "Non-Album/{{ artist|sanitize }}/{{ title|sanitize }}"

[[library.organize.rules]]
name = "default"
priority = 0
match = {}
template = "{{ artist|sanitize }}/{{ album|default('Unknown Album')|sanitize }}/{{ track_no|pad(2) }} - {{ title|sanitize }}"
```

### 8.2 匹配层与模板层分离

规则拆成两层：

- `match` 决定命中哪条规则
- `template` 决定如何渲染目标相对路径

避免把业务规则塞进模板语言中。

### 8.3 一期支持的匹配条件

- `static_playlist`
- `singleton`
- `artist`
- `album`
- `genre`
- `has_lyrics`

规则按 `priority` 从高到低匹配，命中首条后停止。

### 8.4 模板引擎方案

一期使用 `MiniJinja`，不照搬 beets 模板语言，也不引入 JS 运行时。

理由：

- Rust 原生实现，依赖和部署复杂度低
- 能力足够表达路径渲染需求
- 比引入完整脚本运行时更易维护

只开放受限模板能力：

- 变量：`title` `artist` `album` `track_no` `disc_no` `year` `genre` `duration_seconds`
- 条件：基础 `if`
- filter/function：
  - `sanitize`
  - `default`
  - `pad`
  - `mmss`

不承诺完整 Jinja 能力，也不兼容 beets 全量模板语法。

### 8.5 CLI 行为

```bash
melo library organize --dry-run
melo library organize --apply
melo library organize --song 123 --dry-run
melo library organize --rule anime-songs --dry-run
```

输出至少包含：

- `song_id`
- 原路径
- 目标路径
- 命中规则
- 是否跳过
- 跳过原因

### 8.6 冲突策略

一期只支持：

- `conflict_policy = "first_match"`

目标路径已存在时：

- 默认跳过
- 不自动覆盖
- 不自动复制多份

## 9. 模块结构与职责边界

建议代码结构如下：

```text
src/
  main.rs
  cli/
  daemon/
  tui/
  core/
    config/
    db/
    error/
    model/
  domain/
    library/
    player/
    playlist/
  api/
```

### 9.1 `daemon`

负责：

- 加载配置
- 初始化和迁移数据库
- 托管播放状态
- 托管当前 queue
- 暴露 HTTP 和 WebSocket API
- 编排 `library/player/playlist`

### 9.2 `library`

负责：

- 扫描目录
- 读取元数据、歌词、封面
- 建立和更新库表
- 搜索与统计
- 文件组织
- 文件存在性检查

### 9.3 `player`

负责：

- 播放状态机
- 播放控制
- 模式切换
- queue 消费
- WebSocket 状态快照

### 9.4 `playlist`

负责：

- 静态歌单 CRUD
- 智能歌单配置解析
- 统一歌单视图
- 歌单预览
- 歌单加载到 queue

### 9.5 `tui`

负责：

- 浏览库、歌单、queue
- 拉取 daemon 状态
- 发送控制命令
- 不持有播放引擎
- 不直接访问数据库

## 10. API 设计

一期 API 只覆盖 CLI 和 TUI 所需的最小闭环。

### 10.1 system

- `GET /api/system/health`
- `GET /api/system/config`
- `POST /api/system/shutdown`

### 10.2 player

- `POST /api/player/play`
- `POST /api/player/pause`
- `POST /api/player/toggle`
- `POST /api/player/stop`
- `POST /api/player/next`
- `POST /api/player/prev`
- `POST /api/player/seek`
- `POST /api/player/volume`
- `POST /api/player/mode`
- `GET /api/player/status`
- `WS /api/ws`

`GET /status` 和 WS 推送共用同一状态结构。

### 10.3 queue

- `GET /api/queue`
- `POST /api/queue/add`
- `POST /api/queue/remove`
- `POST /api/queue/move`
- `POST /api/queue/clear`
- `POST /api/queue/play`

### 10.4 library

- `POST /api/library/scan`
- `GET /api/library/songs`
- `GET /api/library/songs/:id`
- `GET /api/library/artists`
- `GET /api/library/albums`
- `GET /api/library/search`
- `GET /api/library/stats`
- `POST /api/library/organize/preview`
- `POST /api/library/organize/apply`
- `GET /api/library/songs/:id/lyrics`
- `GET /api/library/artwork/:owner_kind/:owner_id`

### 10.5 playlists

- `GET /api/playlists`
- `GET /api/playlists/:name`
- `POST /api/playlists`
- `DELETE /api/playlists/:name`
- `POST /api/playlists/:name/rename`
- `POST /api/playlists/:name/add`
- `POST /api/playlists/:name/remove`
- `POST /api/playlists/:name/load`
- `POST /api/playlists/:name/preview`
- `POST /api/playlists/sync`

## 11. 错误处理

HTTP 错误统一返回：

```json
{
  "error": {
    "code": "playlist_not_found",
    "message": "Playlist 'AnimeSongs' not found",
    "details": null
  }
}
```

建议稳定错误码：

- `daemon_unavailable`
- `song_not_found`
- `playlist_not_found`
- `playlist_conflict`
- `invalid_query`
- `invalid_template`
- `organize_conflict`
- `file_missing`
- `db_migration_failed`

WebSocket 一期只负责播放状态推送，不承载库和歌单变更广播。

## 12. TUI 设计

TUI 是纯遥控客户端。

### 12.1 核心视图

- `Songs`
- `Artists`
- `Albums`
- `Queue`
- `Playlists`
- `Search`
- `Settings`
- `Help`

### 12.2 交互原则

- `Tab` 在不同焦点区域循环
- `/` 打开搜索
- `?` 打开帮助
- `q` 退出 TUI
- `Space` 播放或暂停
- `>` / `<` 切歌

### 12.3 封面显示

TUI 封面显示是增强项，不是一期核心可用性依赖。

建议：

- 基于 `ratatui-image`
- 终端支持时显示小尺寸封面
- 不支持时自动降级为文本占位卡片
- 由 daemon 提供缩略图读取接口

## 13. beets 迁移策略

一期提供独立迁移脚本，不提供内建 `import-beets` 命令。

建议脚本：

- `scripts/beets_to_melo.py`

迁移重点：

- 歌曲基础元数据
- 专辑基础数据
- 艺术家
- 歌词
- 外部封面路径

一期可忽略：

- `mb_*`
- `discogs_*`
- 大量 flexible attributes
- 原始封面 BLOB

## 14. 一期范围

### 14.1 必须完成

- daemon 常驻与播放
- TUI 遥控
- 扫描入库
- 元数据、歌词、封面读取
- 静态歌单
- 智能歌单
- queue 管理
- 文件组织预览与执行
- 独立 beets 迁移脚本
- 核心运行链路依赖的正式接入：
  - `sea-orm`
  - `sea-orm-migration`
  - `lofty`
  - `rodio`
  - `tokio-tungstenite`
  - `tower-http`
  - `unicode-width`
  - `mime_guess`
  - `tracing`
  - `tracing-subscriber`
- CLI 帮助文档补全

### 14.2 明确不做

- auto tag
- MusicBrainz 匹配
- Web 前端实现
- 浏览器本地播放实现
- 在线歌词抓取
- LRC 结构化入库
- 完整兼容 beets 查询语法
- 完整兼容 beets 模板语法
- queue 持久化恢复
- 封面原始 BLOB 存储
- 对外插件系统

## 15. 插件与扩展策略

一期不做对外插件系统，但内部代码需要预留扩展边界。

优先保留的内部扩展点：

- metadata reader adapter
- smart playlist provider
- organize matcher 和 filter
- import/export hooks

如果未来确实开放插件：

- 优先考虑 Wasm 边界
- 不采用 Rust 原生 ABI 动态库插件作为一期方向

## 16. 测试策略

### 16.1 Rust 单元测试

覆盖：

- 元数据映射
- organize 规则匹配
- MiniJinja 自定义 filter
- playlist 到 queue 的转换
- player 状态机

### 16.2 Rust 集成测试

覆盖：

- 扫描临时目录入库
- 静态歌单 CRUD
- 智能歌单 preview
- organize preview 和 apply
- API handler 与 SQLite 联动

### 16.3 迁移脚本测试

为 `scripts/beets_to_melo.py` 准备样例 beets 数据库，验证：

- 单曲
- 专辑
- 缺失字段
- 歌词
- 外部封面路径

### 16.4 TUI 测试

至少验证：

- daemon 不在线时的连接流程
- 键位到 HTTP 调用的映射
- WebSocket 状态更新到 TUI app state

### 16.5 前端测试

当 Web 前端开发恢复时，继续采用 `Vitest` 作为前端测试基线。

## 17. 总结

Melo 一期的最终目标是：

一个以本地音乐库管理为核心、带 daemon 播放和 TUI 遥控的 beets-inspired 音乐工具，重点完成 `library + playlist + organize + playback` 的稳定闭环，而不是先追求自动标签和 Web 能力。
