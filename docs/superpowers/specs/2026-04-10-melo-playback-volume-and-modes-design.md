# Melo 下一阶段：音量与播放模式设计

日期：2026-04-10

## 1. 目标

在自动下一首、进度同步和会话恢复之后，为播放器补齐最核心的运行时控制项。

本阶段的核心目标是：

- 暴露统一的音量控制契约
- 暴露统一的播放模式契约
- 让自然结束、手动切歌和模式行为保持一致
- 让 CLI / HTTP / WebSocket / TUI 共享同一份模式状态

一句话概括：

把播放器从“能稳定播放”推进到“能稳定按用户偏好播放”。

## 2. 当前现状与问题

当前播放器仍缺少两个关键控制面：

- 音量
- 播放模式

这会带来以下限制：

- daemon 无法提供稳定的远端音量控制
- 自动下一首在队尾只能线性结束，不能表达 repeat 语义
- TUI 无法正确展示用户当前的播放偏好

## 3. 备选方向与推荐

### 3.1 方案 A：由 backend 独自维护音量和模式

优点：

- service 改动表面上较少

缺点：

- 快照无法稳定表达这些运行时状态
- 不同 backend 会导致语义漂移
- TUI / CLI 很难看到统一契约

### 3.2 方案 B：由 service 拥有契约，backend 只负责执行音量

优点：

- 继续保持 service 是唯一状态写入口
- repeat / shuffle 这种纯语义控制可以留在 service
- 快照与 API 更容易稳定

缺点：

- 需要扩展 session 结构
- shuffle 需要额外设计导航策略

### 3.3 方案 C：直接通过改写 queue 顺序实现 shuffle

优点：

- 初看实现简单

缺点：

- 会污染用户看到的显式 queue 顺序
- 难以和 move / remove / current index 修正共存
- 恢复和显示都更混乱

### 3.4 推荐结论

本阶段选择 **方案 B：由 service 拥有契约，backend 只负责执行音量**。

## 4. 本阶段范围

### 4.1 纳入本阶段

- 音量值与静音状态
- repeat 模式
- shuffle 开关
- 快照、HTTP、CLI、TUI 的统一展示
- 自然结束和手动切歌在模式下的一致语义

### 4.2 不纳入本阶段

- EQ
- 每设备音量
- crossfade
- 智能推荐式 autoplay
- 真正修改用户 queue 排序的 shuffle

## 5. 状态模型

### 5.1 音量

建议在 `PlayerSession` 中新增：

- `volume_percent`
- `muted`

语义建议：

- `volume_percent` 范围为 `0..=100`
- `muted = true` 时，实际输出音量为 0，但不丢失原始音量值

### 5.2 播放模式

建议新增：

- `repeat_mode`
- `shuffle_enabled`

其中 `repeat_mode` 建议为：

- `off`
- `one`
- `all`

### 5.3 快照契约

建议在 `PlayerSnapshot` 中新增：

- `volume_percent`
- `muted`
- `repeat_mode`
- `shuffle_enabled`

这样所有外部消费面都能直接读到当前偏好。

## 6. 运行时语义

### 6.1 volume

语义如下：

- 修改音量应立即作用到 backend
- 修改成功后发布新快照
- 重复设置相同音量不应重复 bump `version`

### 6.2 mute

语义如下：

- `mute` 只改变有效输出，不覆盖保存的 `volume_percent`
- `unmute` 恢复到最近一次非静音音量

### 6.3 repeat one

语义如下：

- 当前曲目自然结束时，重新播放当前曲目
- 手动 `next / prev` 仍按显式导航语义执行

这样可以避免 repeat one 把所有手动控制都变得反直觉。

### 6.4 repeat all

语义如下：

- 当前曲目自然结束且已在队尾时，回绕到第 0 首继续播放
- 手动 `next` 在队尾时也回绕到第 0 首
- 手动 `prev` 在队首时回绕到最后一首

### 6.5 shuffle

本阶段建议把 shuffle 定义为“导航策略”，而不是修改可见 queue 顺序。

建议语义：

- 开启 shuffle 后，service 维护一份会话内播放顺序投影
- `next / prev / TrackEnded` 都沿这份投影导航
- `queue` 对外展示顺序保持原样

这样可以避免“用户看到的队列顺序”和“实际播放顺序”互相污染。

## 7. 模块边界

### 7.1 `src/domain/player/service.rs`

负责：

- 保存音量与模式状态
- 决定 `next / prev / TrackEnded` 在不同模式下如何导航
- 生成统一快照

### 7.2 `src/domain/player/backend.rs`

新增音量执行能力，例如：

- `set_volume`

backend 不负责 repeat / shuffle 语义。

### 7.3 `src/domain/player/queue.rs`

仍然只负责显式 queue 顺序及索引修正规则。

shuffle 的运行时投影不应直接塞进 `PlayerQueue` 的基础语义中。

## 8. CLI / API 设计

### 8.1 HTTP

建议新增或补齐：

- `POST /api/player/volume`
- `POST /api/player/mode`

其中：

- `/volume` 负责设置音量与静音
- `/mode` 负责设置 repeat / shuffle

### 8.2 CLI

建议新增结构化命令，而不是继续堆在现有顶层快捷命令上。

推荐形式：

- `melo player volume <0-100>`
- `melo player mute`
- `melo player unmute`
- `melo player mode show`
- `melo player mode repeat off|one|all`
- `melo player mode shuffle on|off`

理由：

- 这些操作不属于最高频“单击式遥控”
- 更适合放进结构化命名空间

### 8.3 TUI

TUI 本阶段至少应能展示：

- 当前音量
- 静音状态
- repeat 模式
- shuffle 状态

是否在本阶段提供复杂交互面板，可在实现计划中再细化。

## 9. 会话恢复关系

本阶段的音量与模式状态应设计成可被后续会话恢复纳入。

但实现上是否在这一阶段立刻持久化，可在计划中细分。

推荐目标：

- 音量与模式字段进入 session 结构
- 恢复层能够无缝接入这些字段

## 10. 测试策略

### 10.1 service 单元测试

必须覆盖：

- 设置音量后快照更新
- 重复设置相同音量不 bump `version`
- `mute / unmute` 语义稳定
- `repeat one` 下自然结束重播当前曲目
- `repeat all` 下队尾回绕
- `shuffle` 下导航沿投影顺序进行

### 10.2 backend 测试

必须覆盖：

- `set_volume` 能作用于真实后端
- 越界音量输入被 service 拦截

### 10.3 集成测试

必须覆盖：

- HTTP 设置后，`status` 和 WebSocket 看到一致的音量与模式
- CLI 输出的新字段与 TUI 显示一致

## 11. 验收标准

本阶段完成时，应满足以下条件：

- 音量与模式成为统一快照契约的一部分
- repeat / shuffle 对自然结束与显式导航的行为可预测
- backend 只负责执行音量，不持有模式语义
- CLI / HTTP / WebSocket / TUI 看到同一份音量与模式状态

## 12. 与整体路线的关系

本阶段是运行时增强路线的最后一块控制面收口。

完成后，Melo 的播放器将同时具备：

- 自然结束事件
- 进度同步
- 会话恢复
- 音量与模式控制

到这里，播放器才算真正从“稳定基础设施”进入“可长期日常使用的运行时系统”。
