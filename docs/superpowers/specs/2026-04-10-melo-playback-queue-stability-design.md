# Melo 下一阶段：播放与队列控制面稳定化设计

日期：2026-04-10

## 1. 目标

下一阶段不引入新的产品面，而是把当前 phase-1 已落地的本地播放闭环，从“最小可用”推进到“行为稳定、状态一致、失败可解释”。

本阶段的核心目标是：

- 收敛播放器会话状态模型
- 收敛队列编辑与播放控制语义
- 让 CLI、HTTP、WebSocket、TUI 共享同一套播放器快照契约
- 为后续的自动续播、进度同步、会话持久化打下稳定基础

一句话概括：

把 Melo 的播放器先做成可靠基础设施，再继续叠加更深的运行时能力。

## 2. 当前现状与问题

当前仓库已经具备以下能力：

- `RodioBackend` 已接入生产播放后端
- `PlayerService` 已能在队列首项上执行基本播放
- daemon 已能通过 HTTP 和 WebSocket 暴露播放器快照
- CLI/TUI 已能读取快照并执行最小遥控

但当前播放器仍然停留在“最小 happy path”阶段，主要问题如下：

- `PlayerService` 只有 `enqueue / play / snapshot` 这条最短路径
- 队列仍然是服务内部的裸 `Vec<QueueItem>`，缺少稳定的业务语义
- 对外快照结构过薄，无法稳定表达控制面状态
- 空队列、越界索引、缺文件、backend 失败等边界场景尚未形成统一规则
- 如果此时直接继续做自动下一首、进度同步、重启恢复，容易把尚未稳定的状态模型过早固化

## 3. 备选方向与结论

### 3.1 方案 A：控制面先稳

围绕播放器状态机和队列编辑面收敛业务规则，先补齐控制语义、快照契约与回归测试。

优点：

- 最适合作为后续阶段地基
- 最容易形成可验证、可讨论的行为契约
- 可以显著降低后续功能扩展时的返工成本

缺点：

- 对真实播放运行时的“自然结束、持续进度、设备恢复”增强不会在这一阶段全部完成

### 3.2 方案 B：运行时韧性先稳

优先把真实播放时的体验做扎实，例如自动下一首、播放进度同步、后端失败恢复。

优点：

- 更直接提升“实际听歌”的体感

缺点：

- 容易建立在仍不稳定的状态模型之上
- service 与 backend 的职责边界会更容易缠绕

### 3.3 方案 C：会话持久化先稳

优先让 daemon 重启后恢复队列和播放会话。

优点：

- 对真实用户价值直接

缺点：

- 会把当前尚未稳定的状态和语义提前写入持久层
- 后续如果重构状态模型，成本会更高

### 3.4 推荐结论

本阶段选择 **方案 A：控制面先稳**。

原因：

- 这是当前播放器体系最短、最必要的收敛路径
- 可以先让播放与队列行为形成清晰契约
- 完成后再推进自动续播、进度同步、持久化恢复，边界会更稳

## 4. 本阶段范围

### 4.1 纳入本阶段

- 明确播放器状态模型
- 明确队列编辑操作面
- 明确播放控制操作面
- 扩充播放器快照结构
- 统一 CLI / API / WebSocket / TUI 对播放器状态的消费方式
- 为关键边界行为建立自动化测试

### 4.2 不纳入本阶段

- 播放进度实时同步
- 自然播放结束后的自动下一首
- 音量控制
- 播放模式（单曲循环、列表循环、随机）
- 队列持久化与重启恢复
- 音频设备切换
- 大规模 TUI 交互重做
- Web 前端或浏览器播放

这些内容都保留为后续阶段候选，但不应混入本阶段实现计划。

## 5. 状态模型

### 5.1 内部会话模型

本阶段在 `player` 域内部引入明确的播放器会话概念，建议命名为 `PlayerSession`。

建议其核心字段为：

- `playback_state`
- `queue`
- `current_index`
- `last_error`
- `version`

其中：

- `playback_state` 表示当前播放生命周期状态
- `queue` 保存当前会话的队列项
- `current_index` 指向当前队列位置；空队列时为 `None`
- `last_error` 保存最近一次需要对外呈现的失败信息
- `version` 在每次有效状态变更后递增，用于快照去歧义与 WebSocket 消费

### 5.2 播放状态

建议统一为以下枚举语义：

- `idle`
- `playing`
- `paused`
- `stopped`
- `error`

建议解释如下：

- `idle`：当前无有效播放上下文，通常对应空队列或未开始播放
- `playing`：当前正在播放有效队列项
- `paused`：当前已有有效队列项，但播放被暂停
- `stopped`：当前会话保留队列与索引，但播放被显式终止
- `error`：最近一次播放切换或 backend 调用发生失败，当前状态需要用户感知

### 5.3 对外快照

建议扩充 `PlayerSnapshot`，至少包含以下字段：

- `playback_state`
- `current_song`
- `queue_len`
- `queue_index`
- `has_next`
- `has_prev`
- `last_error`
- `version`

这一阶段刻意不把进度、音量、模式等字段纳入强制契约，避免范围膨胀。

## 6. 命令语义

### 6.1 播放控制面

本阶段统一暴露以下播放命令：

- `play`
- `pause`
- `toggle`
- `stop`
- `next`
- `prev`

建议语义：

- `play`
  当 `current_index` 为空且队列非空时，默认播放第 0 首
- `pause`
  仅在 `playing` 状态下触发实际暂停；其他状态应保持幂等
- `toggle`
  `playing -> paused`，`paused -> playing`，`idle/stopped` 时等价于 `play`
- `stop`
  停止播放，但保留队列和 `current_index`
- `next`
  若存在下一首，则移动索引并尝试播放；否则返回明确错误或 no-op
- `prev`
  若存在上一首，则移动索引并尝试播放；否则返回明确错误或 no-op

### 6.2 队列控制面

本阶段统一暴露以下队列命令：

- `append`
- `insert`
- `remove`
- `move`
- `clear`
- `play_index`

建议语义：

- `append`
  将歌曲追加到队列尾部
- `insert`
  按指定位置插入歌曲，并修正 `current_index`
- `remove`
  删除指定位置项；若删除的是当前项，必须明确索引修正规则
- `move`
  将指定位置项移动到新位置，并修正 `current_index`
- `clear`
  清空队列并将状态重置为 `idle`
- `play_index`
  先设置 `current_index`，再尝试播放对应队列项

### 6.3 索引修正规则

本阶段必须在设计与测试中明确以下规则：

- 删除当前项后，是否优先落到下一项
- 删除当前项且当前项是队尾时，是否退到前一项
- 删除当前项且删除后队列为空时，是否重置为 `idle`
- `move` 当前项前移或后移时，`current_index` 如何修正
- `clear` 后是否清空 `last_error`

这些规则必须集中写在队列层，而不是散落在 service 各个命令分支中。

## 7. 模块边界

### 7.1 `src/core/model/player.rs`

负责放置共享模型：

- `PlaybackState`
- `QueueItem`
- `NowPlayingSong`
- `PlayerSnapshot`
- 必要的错误展示结构

### 7.2 `src/domain/player/queue.rs`

新增队列逻辑模块，负责：

- `append / insert / remove / move / clear / play_index`
- `current_index` 修正
- `has_next / has_prev` 推导

这一层不关心 `rodio`、WebSocket 或 HTTP。

### 7.3 `src/domain/player/service.rs`

负责：

- 维护 `PlayerSession`
- 执行命令状态迁移
- 在合适时机调用 backend
- 生成并返回统一快照

这一层是播放器状态唯一可信来源。

### 7.4 `src/domain/player/backend.rs`

继续作为播放后端抽象边界，仅定义：

- 加载并播放
- 暂停
- 恢复
- 停止

它不负责队列、状态机或快照生成。

### 7.5 `src/domain/player/rodio_backend.rs`

仅负责真实音频输出执行：

- 打开文件
- 创建 `rodio` player
- 执行 pause/resume/stop

不得在此层写入队列业务规则。

### 7.6 daemon / API / TUI

- `daemon/app.rs` 负责订阅并广播统一快照
- `api/player.rs` 与 `api/ws.rs` 只消费 service 结果，不重新推导状态
- `cli` 与 `tui` 只依赖快照契约，不自行维护独立播放器语义

## 8. 错误处理

### 8.1 错误分类

建议分为两类：

- 命令错误
- 播放错误

命令错误示例：

- 空队列播放
- 索引越界
- 删除不存在的队列项
- `next/prev` 不可用

播放错误示例：

- 文件缺失
- 文件不可打开
- 解码失败
- 后端不可用

### 8.2 处理原则

- 队列编辑失败时，不改变当前播放状态
- 播放切换失败时，不得错误推进到 `playing`
- 需要对外返回稳定错误码
- 需要在 `last_error` 中保留最近一次有效错误
- 只有状态真实变化时，才推进 `version`

建议保留稳定错误码，例如：

- `queue_empty`
- `queue_index_out_of_range`
- `queue_no_next`
- `queue_no_prev`
- `track_file_missing`
- `track_decode_failed`
- `backend_unavailable`

### 8.3 失败后的状态收敛

本阶段不追求完整恢复策略，但必须明确：

- backend 调用失败后，快照仍然要可读
- CLI/TUI/WebSocket 可以看到相同的错误上下文
- 状态机不能在失败后进入“看起来在播放、实际上没在播放”的脏状态

## 9. 事件流

本阶段统一采用以下状态流：

`CLI/API/TUI command -> PlayerService -> Queue mutation -> Backend call -> Snapshot broadcast`

原则：

- `PlayerService` 是唯一可写状态入口
- WebSocket 只广播最新快照，不额外推导业务状态
- CLI `status`、HTTP `status`、TUI 实时视图读取同一份契约

这一阶段不引入复杂事件总线，但应预留“backend 事件回到 service”的扩展位，为后续自动下一首做准备。

## 10. 实现批次

### 批次 A：收敛状态机与队列核心

目标：

- 明确 `PlayerSession`
- 引入 `queue.rs`
- 补齐基础播放与队列命令面
- 完成状态迁移与索引修正规则测试

### 批次 B：统一对外契约

目标：

- 扩充 `PlayerSnapshot`
- 补齐 player/queue API 面
- 让 WebSocket 广播完整快照
- 让 CLI/TUI 统一消费新的快照结构

### 批次 C：补运行时稳态保护

目标：

- 强化缺文件、解码失败、backend 异常时的状态收敛
- 提供必要的幂等行为
- 为后续自动续播、进度同步、会话持久化预留稳定接口

## 11. 测试策略

### 11.1 队列层测试

覆盖：

- `append`
- `insert`
- `remove`
- `move`
- `clear`
- `play_index`
- `current_index` 修正

### 11.2 播放服务测试

覆盖：

- `idle -> playing`
- `playing -> paused`
- `paused -> playing`
- `playing -> stopped`
- `next/prev` 索引切换
- backend 失败时的状态保持与错误暴露

### 11.3 API / WebSocket 集成测试

覆盖：

- 命令后快照是否正确变化
- 错误返回是否稳定
- WebSocket 是否收到与 HTTP 一致的快照

### 11.4 本阶段必须补的回归场景

- 空队列时 `play`
- 删除当前项
- 移动当前项
- 文件缺失时 `play_index`
- 重复 `pause`
- 重复 `stop`
- 队尾 `next`
- 队首 `prev`

## 12. 验收标准

本阶段完成时，应满足以下条件：

- `PlayerService` 拥有完整且可测试的播放控制面
- `queue` 拥有完整且可测试的编辑面
- `PlayerSnapshot` 能稳定表达控制面所需状态
- CLI、HTTP、WebSocket、TUI 共享同一套播放器语义
- 关键边界场景均有自动化测试覆盖
- 状态变更后能稳定广播新快照
- 失败不会污染状态机
- `RodioBackend` 生产路径与 fake backend 测试路径都继续可用

## 13. 后续阶段衔接

本阶段完成后，下一轮优先候选方向为：

- 自动下一首与播放结束事件接入
- 播放进度与持续状态同步
- 队列持久化与 daemon 重启恢复
- 音量与播放模式扩展

这些能力都应建立在本设计收敛出的稳定状态模型与快照契约之上。
