# Melo `libmpv` 默认优先与 `auto` fallback 设计

日期：2026-04-13

## 1. 背景

`mpv-ipc` 一期已经完成以下产品语义收敛：

- `mpv-ipc` 默认进入 headless client mode
- 退出 TUI 默认停播，但 daemon 保留
- 自然 EOF、主动 stop、手动关闭后端、异常退出已有结构化 stop reason
- 当前播放曲目、歌词和封面上下文已经回到 TUI 内部闭环

这意味着 Melo 已经从“遥控外部播放器”的形态，收敛成：

- `TUI` 是主客户端
- 播放后端是可替换引擎
- 上层产品语义应独立于底层播放实现

接下来的二期目标不是重新定义产品语义，而是在不破坏一期体验的前提下，把 `libmpv` 接入为新的优先后端，并让默认 `auto` 策略优先使用它。

同时，`auto` 不能因为单个后端不可用就直接失败。它应表达成：

- 优先使用更符合产品定位的后端
- 如果首选不可用，则自动回退到仍可播放的兜底后端
- 回退行为对用户可见，而不是静默发生

## 2. 目标

本次设计目标如下：

- 新增 `libmpv` 播放后端
- 将播放器运行时抽象重构为“服务层 + 会话句柄”模型，提升代码可读性
- 保持 `PlayerService` 继续作为唯一真相源
- 让 `player.backend = "auto"` 的解析顺序固定为：
  - `mpv_lib -> mpv_ipc -> rodio`
- 让显式 backend 保持严格语义：
  - `mpv_lib` 只尝试 `libmpv`
  - `mpv_ipc` 只尝试 `mpv-ipc`
  - `rodio` 只尝试 `rodio`
- 当 `auto` 发生回退时：
  - 播放继续
  - 当前实际 backend 对用户可见
  - fallback 提示对用户可见
- 保持一期已有产品语义不回退：
  - 自然 EOF 才切歌
  - 退出 TUI 默认停播但 daemon 保留
  - 当前曲目、歌词、封面上下文继续可用

## 3. 非目标

本次明确不做：

- 不重做当前 TUI 信息架构
- 不新增歌词逐行跟随或时间轴高亮
- 不重写封面渲染体系，只保留现有增强/降级模式
- 不移除 `mpv_ipc`
- 不移除 `rodio`
- 不把播放器抽象扩展成大型 capability framework
- 不把后端分发体验做到“零外部依赖安装”
- 不引入新的第三条主播放后端路线

## 4. 方案结论

本次采用“会话句柄化”的平衡方案：

- 不做大规模三层或四层架构重写
- 只把“单次播放生命周期”从 `PlayerService` 中单独抽出来
- 保持 `PlayerService` 仍然持有产品状态与播放语义
- 新增一个专门的 `BackendResolver` 负责 backend 选择与 fallback

这样做的核心收益是：

- 短期不会引入过大的阅读成本
- 中期能明显降低 `PlayerService` 与具体后端之间的耦合
- 接入 `libmpv` 时，不需要把现有一期语义重新做一遍

## 5. 核心抽象

### 5.1 `PlayerService`

`PlayerService` 继续承担以下职责：

- 队列管理
- `repeat` / `shuffle` / `queue_index` 计算
- 当前快照发布
- 运行时 stop reason 的产品语义解释
- TUI / daemon 语义保持

`PlayerService` 不再直接控制某个 backend 实现细节，而是管理“当前活动播放会话”。

### 5.2 `PlaybackBackend`

`PlaybackBackend` 表示某一种后端实现，例如：

- `LibmpvBackend`
- `MpvIpcBackend`
- `RodioBackend`

它的职责不再只是“直接执行播放命令”，而是：

- 报告自身可用性
- 在被选中后创建一份新的播放会话

### 5.3 `PlaybackSessionHandle`

新增 `PlaybackSessionHandle` 作为单次播放生命周期句柄。

它负责：

- `pause`
- `resume`
- `stop`
- `current_position`
- 运行时事件订阅

它不负责：

- 队列
- `repeat` / `shuffle`
- 切歌逻辑
- TUI 语义

这部分由 `PlayerService` 继续负责。

### 5.4 `ActivePlaybackSession`

`PlayerService` 内部维护：

- `generation`
- `resolved_backend`
- `session_handle`

可命名为 `ActivePlaybackSession`。

它表示“当前这一次播放实例”，用于把上层产品状态和底层 session 生命周期绑定起来。

### 5.5 `BackendResolver`

新增 `BackendResolver`，负责：

- 读取 `player.backend`
- 根据配置值决定是否允许 fallback
- 在 `auto` 模式下按固定顺序探测 backend
- 返回本次实际选中的 backend 与可选提示

它是唯一负责 fallback 链的地方，避免把解析逻辑散落在：

- `factory`
- `PlayerService`
- 各个 backend 实现

## 6. 后端解析语义

### 6.1 配置值

支持以下后端名：

- `auto`
- `mpv_lib`
- `mpv_ipc`
- `rodio`
- `mpv`

兼容规则：

- `mpv` 继续映射为 `mpv_ipc`

### 6.2 `auto` 解析顺序

`auto` 固定按以下顺序尝试：

1. `mpv_lib`
2. `mpv_ipc`
3. `rodio`

语义说明：

- `mpv_lib` 是默认首选，因为它更接近产品定位中的“无窗口主客户端后端”
- `mpv_ipc` 是成熟回退选项
- `rodio` 是最终兜底 backend，保证在缺少 `mpv` 体系时仍有机会播放

### 6.3 显式 backend 语义

显式 backend 不自动回退：

- `player.backend = "mpv_lib"`：只尝试 `libmpv`
- `player.backend = "mpv_ipc"`：只尝试 `mpv-ipc`
- `player.backend = "rodio"`：只尝试 `rodio`

如果显式 backend 不可用，应直接返回明确错误，而不是继续尝试其他后端。

## 7. 会话生命周期

### 7.1 创建

调用 `play()` 时流程调整为：

1. `PlayerService` 决定当前应播放的队列项
2. `BackendResolver` 返回一个本次应使用的 backend 决议
3. 对应 backend 创建新的 `PlaybackSessionHandle`
4. `PlayerService` 把它保存为当前 `ActivePlaybackSession`
5. `PlayerService` 开始订阅这一 session 的运行时事件

### 7.2 替换

当发生以下动作时，旧 session 必须先 teardown，再创建新 session：

- 重新 `play`
- `next`
- `prev`
- `play_index`
- 因自然 EOF 自动切歌

这样能保证：

- `generation` 边界清晰
- stale event 更容易忽略
- 不同后端的内部状态不会污染上层服务层

### 7.3 控制

`pause` / `resume` / `stop` / `current_position` 全部通过当前 `session_handle` 调用。

这样 `PlayerService` 不需要知道：

- `libmpv` 如何暂停
- `mpv-ipc` 如何发命令
- `rodio` 如何维护播放器对象

它只需要消费统一的 session 接口。

## 8. 运行时事件与 stop reason

一期已经定义的 `PlaybackStopReason` 继续保持稳定：

- `NaturalEof`
- `UserStop`
- `UserClosedBackend`
- `BackendAborted`

二期规则如下：

- 各后端内部事件先在自己的 session 层完成适配
- `PlayerService` 只处理统一的运行时事件
- `PlayerService` 不再区分事件来自哪个具体 backend

也就是说：

- `libmpv` 需要把自身回调/事件流翻译成统一 stop reason
- `mpv_ipc` 继续使用现有 stop reason 语义
- `rodio` 继续只在自然播完时发送 `NaturalEof`

## 9. 快照与用户可见性

### 9.1 实际 backend

`PlayerSnapshot.backend_name` 继续表示：

- 当前实际正在使用的 backend

它不表示配置值，而表示解析后的真实结果。

### 9.2 fallback 提示

当 `auto` 发生回退时，快照层需要有用户可见提示。

本次设计推荐新增轻量字段：

- `backend_notice: Option<String>`

示例：

- `auto selected mpv_lib`
- `mpv_lib unavailable, fell back to mpv_ipc`
- `mpv_lib and mpv_ipc unavailable, fell back to rodio`

不推荐本次直接引入复杂结构如：

- `backend_resolution { configured, resolved, attempted, failures }`

原因是这会增加上层消费复杂度，不利于二期最小可交付。

### 9.3 展示位置

`backend_notice` 至少应出现在：

- CLI 输出
- TUI 状态栏或等效可见位置
- API 快照响应

这样用户能明确知道：

- 当前使用的是哪个 backend
- 是否发生了 fallback

## 10. `libmpv` 后端边界

### 10.1 二期要求

`libmpv` 二期至少需要支持：

- `play`
- `pause`
- `resume`
- `stop`
- `current_position`
- 统一 stop reason 映射

### 10.2 不要求

二期不要求：

- 一开始就做最完整的 capability 系统
- 一开始就支持所有高级 `libmpv` 事件
- 一开始就替换全部观测与日志体系

只要它能成为符合一期产品语义的一等后端即可。

## 11. 代码可读性原则

本次设计以代码可读性为主要约束之一，具体原则如下：

- `PlayerService` 继续做产品状态机，不拆成更大的 coordinator 树
- fallback 链只出现在 `BackendResolver`
- backend 细节只出现在各 backend / session 文件中
- session handle 只表示“当前播放实例”，不混入队列语义
- 新增抽象数量控制在最小必要范围内

预期阅读路径应保持直观：

1. 看产品行为：读 `PlayerService`
2. 看 backend 选择：读 `BackendResolver`
3. 看单次播放实例：读 `PlaybackSessionHandle`
4. 看具体实现：读 `LibmpvBackend` / `MpvIpcBackend` / `RodioBackend`

## 12. 实施范围

本次实施只覆盖以下四个切片：

### 12.1 切片一：抽象重构

- 引入 `BackendResolver`
- 引入 `PlaybackSessionHandle`
- 引入 `ActivePlaybackSession`
- 调整 `PlayerService` 以管理当前 session

### 12.2 切片二：接入 `libmpv`

- 新增 `LibmpvBackend`
- 对齐统一 session 契约
- 对齐 stop reason 语义

### 12.3 切片三：`auto` fallback

- `auto` 优先 `mpv_lib`
- 失败回退 `mpv_ipc`
- 再失败回退 `rodio`
- 显式 backend 不回退

### 12.4 切片四：提示与观测

- 快照层新增 `backend_notice`
- TUI / CLI 能看到实际 backend 和 fallback 提示
- 为 fallback 路径补齐回归测试

## 13. 测试策略

### 13.1 Resolver 测试

覆盖：

- `auto -> mpv_lib`
- `auto -> mpv_ipc`
- `auto -> rodio`
- 显式 `mpv_lib` 不回退
- 显式 `mpv_ipc` 不回退

### 13.2 Service / session 测试

覆盖：

- 新 session 创建与旧 session teardown
- `generation` 与 stale event 忽略
- 各后端 session 都走统一控制路径

### 13.3 `libmpv` 契约测试

覆盖：

- 播放控制
- 位置读取
- stop reason 映射
- 异常退出处理

### 13.4 TUI / CLI 测试

覆盖：

- 实际 backend 名显示
- fallback notice 显示
- fallback 后仍可播放

## 14. 验收标准

满足以下条件即可视为二期最小可交付完成：

- `player.backend = "mpv_lib"` 时，`libmpv` 能完成播放控制与进度同步
- `player.backend = "auto"` 时，优先尝试 `libmpv`
- `libmpv` 不可用时自动回退到 `mpv_ipc`
- `mpv_ipc` 不可用时自动回退到 `rodio`
- 回退后播放继续成立
- `PlayerSnapshot.backend_name` 正确表示实际 backend
- fallback 发生时有明确 `backend_notice`
- 一期产品语义保持稳定：
  - 只有 `NaturalEof` 才触发切歌
  - 退出 TUI 默认停播但 daemon 保留
  - 当前曲目、歌词、封面上下文仍能正常显示

## 15. 结论

本次设计不把“默认切到 `libmpv`”实现成一次大重构，而是用最小必要抽象来支撑二期目标：

- 通过 `PlaybackSessionHandle` 抽离单次播放生命周期
- 通过 `BackendResolver` 集中管理 fallback 链
- 通过 `auto = mpv_lib -> mpv_ipc -> rodio` 实现“默认优先 + 有兜底”
- 通过 `backend_notice` 保证 fallback 对用户可见

这样既能提升代码可读性，也能把 `libmpv` 作为更符合产品定位的默认优先后端引入，同时保留 `mpv_ipc` 和 `rodio` 作为稳定回退路径。
