# Melo 下一阶段：播放结束事件与自动下一首设计

日期：2026-04-10

## 1. 目标

在已经稳定的播放器控制面之上，补齐“自然播放结束”这条运行时主链路。

本阶段的核心目标是：

- 让 `PlayerService` 能感知当前歌曲自然播放结束
- 在存在下一首时自动切到下一首并继续播放
- 在队列播放到结尾时稳定收敛到可解释状态
- 为后续进度同步和会话恢复建立运行时事件入口

一句话概括：

把播放器从“只能被命令驱动切歌”推进到“能感知真实播放结束并自主推进”。

## 2. 当前现状与问题

当前播放器已经具备：

- 统一的 `PlayerSession`
- 明确的 `PlayerSnapshot`
- `next / prev / play_index` 等控制语义
- WebSocket 对快照的统一广播

但仍存在以下缺口：

- `PlaybackBackend` 只有命令接口，没有运行时事件接口
- `RodioBackend` 能播放文件，但不会把“播放自然结束”回传给 `PlayerService`
- `PlayerService` 的状态变化仍全部依赖显式命令
- 如果用户不手动 `next`，队列播放无法自然向前推进

这意味着当前播放器虽然控制面稳定，但运行时仍停留在半闭环状态。

## 3. 备选方向与推荐

### 3.1 方案 A：由 service 主动轮询 backend 是否结束

优点：

- 改动集中在 service 侧
- 不需要立即改造太多 API 表面

缺点：

- 轮询延迟不可避免
- service 会被迫了解 backend 的更多内部状态
- 后续继续扩展进度和错误事件时，轮询会越来越臃肿

### 3.2 方案 B：由 backend 直接回调 service 推进状态

优点：

- 响应速度快
- 表面上实现路径最短

缺点：

- backend 与 service 双向耦合
- 测试替身更难实现
- 后续替换播放后端时边界不清晰

### 3.3 方案 C：backend 发出运行时事件，service 统一消费

优点：

- 边界最清晰
- 最适合作为后续进度同步与恢复机制的基础
- fake backend 和真实 backend 都容易做一致测试

缺点：

- 需要新增事件模型与订阅通道
- 需要处理旧事件、过期事件和并发切歌场景

### 3.4 推荐结论

本阶段选择 **方案 C：backend 发出运行时事件，service 统一消费**。

原因：

- 这条边界最适合后续继续扩展为完整运行时模型
- 不会把自动下一首的语义塞进 `RodioBackend`
- 可以保持 `PlayerService` 继续作为唯一状态写入口

## 4. 本阶段范围

### 4.1 纳入本阶段

- 新增 backend 到 service 的运行时事件通道
- 表达“当前播放项自然结束”的稳定事件模型
- 当存在下一首时自动推进并播放
- 当不存在下一首时稳定收敛到 `stopped`
- 为过期事件提供忽略机制
- 为自然结束路径补齐单元测试与集成测试

### 4.2 不纳入本阶段

- 播放进度同步
- 会话持久化恢复
- 音量控制
- 播放模式
- 跨设备切换
- gapless / crossfade

## 5. 事件模型

### 5.1 新增运行时事件

建议在 `player` 域中新增运行时事件模型，例如：

- `TrackEnded`
- `BackendFailed`

本阶段强依赖的是 `TrackEnded`，`BackendFailed` 只作为模型预留，不要求本阶段必须完整接通所有错误细分。

### 5.2 事件必须带播放代次

本阶段必须引入“播放代次”或“播放令牌”概念。

原因：

- 用户可能在上一首快结束时手动切到下一首
- backend 可能在旧播放器停止后稍晚才发出结束事件
- 如果不区分代次，旧事件会污染当前状态机

建议做法：

- `PlayerSession` 增加 `playback_generation`
- 每次真正发起 `load_and_play` 时递增
- backend 发出的运行时事件必须携带 generation
- service 只处理与当前 generation 一致的事件

## 6. 模块边界

### 6.1 `src/domain/player/backend.rs`

继续保留现有命令接口，并新增运行时事件订阅能力。

建议职责：

- `load_and_play / pause / resume / stop`
- `subscribe_runtime_events`

这一层只负责把后端运行事实告诉 service，不负责决定是否自动下一首。

### 6.2 `src/domain/player/rodio_backend.rs`

负责：

- 监听当前 `rodio` 播放实例何时自然结束
- 在结束时发出带 generation 的 `TrackEnded`
- 在切歌或 stop 后避免重复发出已失效事件

不负责：

- 修改队列
- 直接触碰 `PlayerSession`
- 推导“应该播放哪一首”

### 6.3 `src/domain/player/service.rs`

负责：

- 启动或持有事件消费任务
- 忽略过期 generation 事件
- 在当前项结束后决定是否自动推进到下一首
- 在队尾结束后把状态收敛到 `stopped`
- 生成并广播新快照

## 7. 运行时语义

### 7.1 当前曲目自然结束且存在下一首

语义如下：

1. service 收到当前 generation 的 `TrackEnded`
2. 计算下一首索引
3. 将 `current_index` 推进到下一首
4. 调用与手动 `next` 一致的播放切换流程
5. 发布 `playing` 状态的新快照

这意味着“自动下一首”和“手动 next”应共享同一套切歌语义，而不是两套平行规则。

### 7.2 当前曲目自然结束且已经是队尾

语义如下：

- 不自动回绕
- 不报错
- 保留当前 `queue_index`
- `playback_state` 收敛为 `stopped`
- `last_error` 清空
- 发布最终快照

这样 CLI / TUI / WebSocket 都能看到“队列播放完毕，但当前会话还存在”的稳定结果。

### 7.3 自动推进时的失败收敛

如果自动推进到下一首时发生：

- 文件缺失
- 解码失败
- backend 不可用

则语义应与显式 `next` 一致：

- `queue_index` 已切到目标项
- `playback_state = error`
- `last_error` 写入稳定错误码
- 广播错误快照

不允许出现“实际上没在播放，但快照还显示上一首 playing”的脏状态。

## 8. 对外契约影响

本阶段不强制新增 HTTP 或 CLI 命令。

对外变化主要体现在已有契约上：

- `GET /api/player/status` 会在自然结束后看到自动推进结果
- `WS /api/ws/player` 会在自然结束后自动收到新快照
- TUI 不需要自己推导自动切歌，只消费快照
- CLI `status` 直接读取自然推进后的状态

也就是说，本阶段重点是让已有控制面开始反映真实运行时。

## 9. 测试策略

### 9.1 service 单元测试

必须覆盖：

- 收到当前 generation 的 `TrackEnded` 后自动切到下一首
- 队尾 `TrackEnded` 后收敛到 `stopped`
- 旧 generation 的结束事件被忽略
- 手动 `stop` 后旧结束事件不污染状态
- 自动推进失败时进入 `error`

### 9.2 backend 测试

必须覆盖：

- `RodioBackend` 在自然播放结束时发出事件
- `stop` 或重新 `load_and_play` 后不会错误复用旧事件

### 9.3 集成测试

必须覆盖：

- WebSocket 在自然切歌后能收到更新快照
- HTTP `status` 和 WebSocket 看到同一结果

## 10. 验收标准

本阶段完成时，应满足以下条件：

- 播放器能够感知歌曲自然结束
- 当存在下一首时能够自动播放下一首
- 当队列播放完毕时收敛为 `stopped`
- 旧事件不会污染当前播放状态
- 自动推进与手动切歌共享同一套状态规则
- HTTP / WebSocket / CLI / TUI 都能观察到一致结果

## 11. 与后续阶段的关系

本阶段完成后，会直接为后续三件事铺路：

- 进度同步可以复用同一条 backend -> service 运行时通道
- 会话恢复可以复用更完整的播放会话语义
- 播放模式中的 repeat / shuffle 可以复用自然结束分支
