# Melo Playlist 视图与当前播放上下文设计

日期：2026-04-12

## 1. 背景

当前 `melo` 在 direct-open 目录或文件后，后台已经会创建或复用对应的 ephemeral playlist，但 TUI 并没有把这个“当前来源 playlist”继续带入首页浏览上下文。

现状问题主要有三类：

- direct-open 只把 `source_label` 传给 TUI，没有把 `playlist_name` 作为当前浏览上下文继续传递
- TUI 聚合快照只有 `player` 与 `active_task`，没有 playlist 浏览态，也没有“当前播放来源”概念
- 当前 TUI 首页仍是一个非常早期的占位布局，只有简单的 sidebar、queue 文本和 status 文本，无法承载 playlist 浏览、来源提示、模式切换等真实交互

这导致用户在执行：

- `melo`
- `melo <目录>`
- `melo <文件>`

后，虽然底层已经形成了临时歌单与当前播放上下文，但界面上看不到“现在正在播放哪个 playlist、我当前正在浏览哪个 playlist、从 playlist 某首开始播放会怎么影响当前播放列表”这些关键语义。

同时，现有内部领域模型把“当前播放真相”建模为 `queue`。这个建模在领域层是合理的，但在 UI 层直接暴露 `queue` 概念不够自然，更容易让用户误解为严格 FIFO 队列，而不是“当前播放会话的物化列表”。

因此，本次设计目标不是简单补一个列表组件，而是把 TUI 首页正式收敛为“playlist 为中心的浏览与播放入口”，同时保留当前播放器以 `queue` 作为领域内部播放真相的分层。

## 2. 目标

本次设计目标如下：

- 让 TUI 首页正式支持 playlist 浏览
- 让 direct-open 后默认落在当前生成的临时歌单上
- 明确区分“当前正在播放的 playlist 来源”和“当前正在浏览的 playlist”
- 让用户可以：
  - 从左侧 playlist 直接播放整个歌单
  - 从右侧预览中的某首开始播放整个歌单
- 让 `repeat` / `shuffle` 作为全局播放器状态，在 TUI 中可视并可直接切换
- 保持领域层 `playlist` 与 `queue` 分层，但 UI 不直接暴露 `queue` 术语
- 为未来继续补齐 `Songs / Search / Queue` 等视图保留清晰骨架，而不是继续堆叠临时状态

## 3. 非目标

本次明确不做：

- 不迁移到 `dioxus-tui` 或其他新 UI 库
- 不替换现有 `ratatui + crossterm` 运行栈
- 不在 TUI 内实现 playlist 管理操作，例如提升临时歌单、重命名、删除、加入/移除歌曲
- 不把 `queue` 暴露为用户可编辑对象；本次不支持在 TUI 中删除、重排当前播放列表
- 不做 queue 窗口化或只保留几十首的轻量缓存设计
- 不实现完整分页、虚拟滚动或大列表专项优化
- 不把所有 TUI 文案做成可配置模板

## 4. 术语与核心语义

为避免后续实现与 UI 表达混乱，本次固定以下语义：

### 4.1 Playlist

`playlist` 表示来源定义或来源集合，包含：

- `static playlist`
- `smart playlist`
- `ephemeral playlist`

它回答的问题是：“有哪些歌可供本次播放上下文使用？”

### 4.2 Queue

`queue` 只保留为领域层内部术语，不直接作为 TUI 主文案对外暴露。

在本项目语义中，`queue` 不是严格的 FIFO 队列，而是：

- 当前播放会话的物化列表
- 某个 playlist 或其他来源在某个时刻展开后的播放副本
- 带当前索引、播放模式和后续导航语义的实际播放真相

它回答的问题是：“这一次实际怎么播？”

### 4.3 当前播放来源

“当前播放来源”表示当前正在播放的 `queue` 是从哪个 playlist 物化出来的。

这和“当前浏览中的 playlist”不是同一个概念。

### 4.4 当前浏览选择

“当前浏览选择”表示左侧列表当前高亮的 playlist，以及右侧预览当前高亮的歌曲。

浏览选择只影响预览和后续动作目标，不会自动改变当前播放来源。

## 5. 方案结论

本次采用“现有栈上的中等重构方案”：

- 保留 `ratatui + crossterm`
- 把 TUI 首页重构为 playlist 为中心的正式视图
- 把播放状态与浏览状态拆分建模，但通过统一的 TUI 聚合快照下发
- 保持 `queue` 作为领域层播放真相
- UI 使用“播放列表 / 当前播放来源 / 歌单预览 / Now Playing”等更自然文案，不直接使用 `queue`

相比“只补一个列表”的最小补丁方案，此方案虽然需要补一层聚合数据与本地状态机，但能让这次 playlist 视图真正具备可扩展的基础。

## 6. 总体架构

本次改动分为三层：

### 6.1 Daemon 聚合层

daemon 负责向 TUI 下发：

- 当前播放器快照
- 当前活动运行时任务
- playlist 浏览首页所需的聚合数据

其中：

- 播放状态继续属于 `PlayerSnapshot`
- playlist 浏览态属于新的 TUI 聚合浏览快照

### 6.2 TUI 本地状态层

TUI 本地负责维护：

- 当前活动视图
- 当前焦点区域
- 左侧 playlist 高亮
- 右侧歌曲高亮
- 已拉取的 playlist 预览缓存

这些状态属于单个 TUI 会话的浏览状态，不属于 daemon 全局状态。

### 6.3 交互命令层

围绕 playlist 视图新增两个核心动作：

- 从 playlist 第 1 首开始播放整个歌单
- 从 playlist 某首开始播放整个歌单

无论用户从左栏还是右栏发起播放，落点都统一为：

- 把该 playlist 完整物化为当前播放列表
- 设置当前播放索引
- 立即开始播放
- 更新“当前播放来源”

## 7. 数据模型

### 7.1 `PlayerSnapshot`

`PlayerSnapshot` 继续只负责播放器领域快照，不增加 playlist 浏览字段。

理由：

- 避免把浏览状态和播放状态耦合进同一个契约
- 避免未来 `Songs / Search / Queue` 等视图不断往 `PlayerSnapshot` 塞前端专用字段

### 7.2 `TuiSnapshot`

`TuiSnapshot` 扩展为真正的首页聚合快照，建议包含：

- `player`
- `active_task`
- `playlist_browser`

### 7.3 `PlaylistBrowserSnapshot`

建议新增结构：

- `default_view`
- `default_selected_playlist`
- `current_playing_playlist`
- `visible_playlists`

字段语义如下：

- `default_view`
  - 当前阶段固定为 `playlist`
  - 用于让 TUI 首页默认进入 playlist 视图
- `default_selected_playlist`
  - 当前应默认高亮的 playlist 名称
  - direct-open 时等于当前创建或复用的 playlist
- `current_playing_playlist`
  - 当前播放来源摘要；若当前没有来源上下文则为 `None`
- `visible_playlists`
  - 左侧播放列表区域可展示的统一摘要列表

### 7.4 `PlaylistListItem`

建议包含：

- `name`
- `kind`
- `count`
- `is_current_playing_source`
- `is_ephemeral`

说明：

- `kind` 取值至少包含 `ephemeral / static / smart`
- `count` 用于列表中直观显示歌曲数量
- `is_current_playing_source` 用于左侧列表标记“当前正在播放来源”
- `is_ephemeral` 用于界面差异化样式或标签

### 7.5 预览数据不进 websocket 主快照

右侧“歌单预览”不建议进入 websocket 主快照。

原因：

- 预览可能很长
- 当前预览哪个 playlist 是 TUI 本地浏览态，不是 daemon 全局状态
- 若把预览塞进 websocket 聚合快照，会把本地高亮与远端聚合绑定得过紧

因此右侧预览采用按需 HTTP 拉取。

## 8. API 与服务设计

### 8.1 TUI 首页快照接口

建议新增：

- `GET /api/tui/home`

职责：

- 返回进入首页所需的完整聚合数据
- 让 TUI 在 websocket 首帧之外，也能明确拉到首页初始化状态

### 8.2 Playlist 预览接口

建议新增：

- `GET /api/playlists/:name/preview`

职责：

- 按 playlist 名称返回歌曲预览
- 供右侧“歌单预览”区域按需拉取

### 8.3 Playlist 播放接口

建议新增：

- `POST /api/playlists/:name/play`

请求体建议包含：

- `start_index`

语义：

- 左侧 playlist 列表按 `Enter`
  - `start_index = 0`
- 右侧歌曲预览按 `Enter`
  - `start_index = 当前高亮歌曲索引`

接口执行后应：

- 取出该 playlist 的完整歌曲列表
- 一次性构造新的当前播放列表
- 设置播放索引为 `start_index`
- 立即播放
- 更新当前播放来源
- 返回最新聚合状态或至少返回最新播放器快照

### 8.4 内部播放器能力

播放器服务建议新增一次性替换当前播放列表的内部能力，例如：

- `replace_queue(items, start_index)`

本次不要求对外暴露这个名字，只要求内部能力具备。

设计要求：

- 不用低效的“清空 + 多次 append + 多次发布”散操作模拟
- 替换后应保留播放器模式状态，例如 `repeat / shuffle`
- 替换后立即切换到目标索引并开始播放

### 8.5 Direct-open 与默认落点

direct-open 在成功创建或复用 playlist 后，除了继续返回 `source_label`，还应确保当前 playlist 名称进入 TUI 首页聚合数据：

- `default_selected_playlist = 当前临时歌单`
- `current_playing_playlist = 当前临时歌单`
- `default_view = playlist`

这样裸 `melo` 和 `melo <目录>` 进入 TUI 时，首页才能自动落到正确来源。

## 9. TUI 信息架构

本次首页采用三段式信息架构，对应用户已确认的 `B` 方向。

### 9.1 左栏：播放列表

左栏展示统一 playlist 列表，组织方式采用“混合列表”：

- 顶部固定一个“当前播放来源”小节
- 下面显示其余可见 playlist 的扁平列表

设计要求：

- 如果当前播放来源本身也在列表中，不重复渲染两份实体，只在列表项中加当前来源标记
- direct-open 后默认高亮当前临时歌单
- 高亮变化只更新右侧预览，不自动改变当前播放来源

### 9.2 右上：当前播放来源 + 模式

右上区域展示：

- 当前播放来源名称
- 当前来源类型
- `repeat` 状态
- `shuffle` 状态

这是状态区，不承载长列表。

### 9.3 右下：歌单预览

右下区域展示当前左侧高亮 playlist 的歌曲预览。

设计要求：

- 支持独立焦点
- 支持上下移动高亮
- 不自动播放
- 当前右侧高亮仅表示“若此时按 Enter，将从这首开始播放整个歌单”

### 9.4 底部：Now Playing

底部播放栏继续承载：

- 当前歌曲
- 播放状态
- 进度与时长
- 核心快捷键提示

`Now Playing` 只用于底部播放栏，不用于给长列表面板命名。

## 10. 文案约定

为避免实现时文案反复摇摆，本次固定如下标题：

- 左栏标题：`播放列表`
- 右上标题：`当前播放来源`
- 模式区标题：`播放模式`
- 右下标题：`歌单预览`
- 底部播放栏：`Now Playing`

UI 不直接把列表面板命名为 `Queue`。

## 11. 视图与焦点模型

### 11.1 视图定位

本次保留多视图框架，但 playlist 成为默认首页。

即：

- 当前 TUI 架构不收敛成“永远只有一个 playlist 页面”
- 但首页默认进入 `playlist` 视图
- 后续 `Songs / Search / Queue` 等视图仍有独立扩展空间

### 11.2 默认落点

direct-open 进入后，默认状态如下：

- 默认视图：`playlist`
- 默认焦点：左栏 playlist 列表
- 默认高亮 playlist：当前 direct-open 生成或复用的临时歌单
- 右侧预览：自动加载该歌单

### 11.3 焦点切换

采用混合型键位模型：

- `Tab`
  - 在左栏 playlist 列表与右侧歌曲预览之间切换焦点
- 方向键
  - 在当前焦点区域内移动

## 12. 交互语义

### 12.1 左栏交互

左栏高亮变化：

- 只更新右侧歌单预览
- 不自动改变当前播放来源
- 不自动播放

左栏按 `Enter`：

- 从该 playlist 第 1 首开始播放整个歌单
- 把该 playlist 物化为当前播放列表
- 更新当前播放来源

### 12.2 右栏交互

右栏高亮变化：

- 只改变当前“准备从哪首开始播放”的浏览选择
- 不自动播放

右栏按 `Enter`：

- 从当前高亮歌曲开始播放整个歌单
- 把该 playlist 完整物化为当前播放列表
- 播放索引设为右侧高亮歌曲索引
- 更新当前播放来源

### 12.3 播放模式交互

第一版直接在 TUI 中支持切换播放模式：

- `r`
  - 循环切换 `off -> all -> one -> off`
- `s`
  - 切换 `shuffle on/off`

### 12.4 现有播放控制键

保留现有键位：

- `Space`
  - 播放 / 暂停
- `>`
  - 下一首
- `<`
  - 上一首
- `?`
  - 帮助
- `q`
  - 退出

## 13. 播放模式语义

播放模式属于全局播放器状态，而不是 playlist 局部状态。

即用户切换到别的 playlist 后：

- 当前 `repeat` 继续沿用
- 当前 `shuffle` 继续沿用

本次固定以下语义：

- 顺序播放
  - `repeat = off`
  - `shuffle = false`
- 列表循环
  - `repeat = all`
  - `shuffle = false`
- 单曲循环
  - `repeat = one`
- 随机播放
  - `shuffle = true`

其中随机播放只改变导航顺序，不改写当前播放列表实体顺序。

也就是说：

- 当前播放列表仍保留稳定顺序
- `next / prev / 自然播完` 时使用随机导航投影顺序
- 用户从 playlist 某首开始播放时，当前首仍是明确选择的歌曲

## 14. Queue 与 UI 的关系

本次设计明确：

- `queue` 是领域内部模型
- UI 不必出现 `queue` 一词
- 当前播放列表可以理解为某个来源 playlist 的物化副本

但为了未来扩展能力，仍保留 `playlist` 与 `queue` 分层：

- smart playlist 可以先展开再播放
- direct-open 的临时歌单可以统一落到这套模型
- 以后如果需要支持“临时插播、重排、追加”，可只作用在当前播放列表，不污染来源 playlist

本次不开放 TUI 内部对当前播放列表的删除或重排，因此用户不会直接面对“编辑 queue”的语义负担。

## 15. 错误处理与边界行为

### 15.1 默认 playlist 不存在

若 `default_selected_playlist` 指向的 playlist 已不存在：

- TUI 不崩溃
- 左栏回退到第一个可见 playlist
- 右侧显示 fallback playlist 的预览
- 顶部或底部显示一次性回退提示

### 15.2 预览加载失败

若某个 playlist 预览加载失败：

- 右侧预览区显示错误态
- 左栏列表仍可继续使用
- 当前播放来源与播放状态不受影响

### 15.3 播放失败

若用户从左栏或右栏触发播放失败：

- 不修改当前播放来源显示
- 保留当前浏览高亮
- 底部状态栏继续通过播放器错误信息展示失败原因

### 15.4 当前播放来源变为不可见

若当前播放来源是后来变得不可见的 ephemeral playlist，但记录仍存在：

- “当前播放来源”小节继续显示它
- 不因它不在常规列表中可见就丢失当前播放上下文

### 15.5 Smart playlist

smart playlist 在左栏与其他 playlist 一样作为统一来源展示。

其特点是：

- 预览通过查询动态生成
- 从某首开始播放时，也要先完整展开为当前播放列表再播放

## 16. 性能与范围约束

本次不做 queue 窗口化。

理由：

- 当前播放器内部已经以完整 `Vec<QueueItem>` 建模
- 对常见音乐使用场景下的几千首规模，完整物化仍是可接受的第一阶段方案
- 若此时引入“只保留几十首”的窗口设计，会显著增加 `next / prev / repeat / shuffle / 会话恢复` 的复杂度

本次真正需要优化的是：

- 避免低效的逐首散操作重建当前播放列表

而不是：

- 改变“完整当前播放列表”的领域语义

若后续发现大列表性能不足，再单独设计：

- 批量查询
- 列表分页 / 虚拟化
- 后端预加载窗口

这些都不纳入本次范围。

## 17. 测试策略

本次测试分四层：

### 17.1 单元测试

覆盖 TUI 本地状态机：

- 焦点切换
- 左栏 `Enter` 语义
- 右栏 `Enter` 语义
- `r / s` 模式切换

测试规范遵循项目约定：

- 源文件底部只保留 `#[cfg(test)] mod tests;`
- 单元测试放到同名目录下的 `tests.rs`

### 17.2 服务层测试

覆盖 playlist 播放语义：

- 从 playlist 第 1 首开始播放是否正确
- 从 playlist 指定索引开始播放是否正确
- 当前播放来源是否更新
- `repeat / shuffle` 是否保留为全局状态

### 17.3 API / 聚合快照测试

覆盖：

- `TuiSnapshot` 是否包含 playlist 浏览首页需要的字段
- direct-open 后默认高亮是否落到当前临时歌单
- 当前播放来源是否能正确进入首页聚合快照

### 17.4 集成测试

覆盖关键用户路径：

- direct-open 启动后默认进入 playlist 视图
- 左栏切换只更新预览，不改变当前播放来源
- 右栏从指定歌曲开始播放，会把整个歌单装载为当前播放列表并从该曲开始

## 18. 分阶段实现建议

建议按以下顺序实现：

1. 扩展 TUI 聚合快照与首页初始化数据
2. 增加 playlist 预览与 playlist 播放接口
3. 增加播放器内部一次性替换当前播放列表能力
4. 重构 TUI 首页为 playlist 为中心的布局
5. 加入焦点切换、左右栏播放语义、模式键位
6. 补齐 direct-open 当前 playlist 默认落点与集成测试

## 19. 结论

本次设计把 TUI 首页正式收敛为 playlist 为中心的浏览与播放入口：

- 保留现有技术栈
- 保留 `playlist / queue` 分层
- 让 direct-open 当前临时歌单真正进入首页上下文
- 把“当前播放来源”和“当前浏览选择”明确拆开
- 把“从 playlist 某首开始播放整个歌单”定为统一且稳定的行为

这样既能解决当前“临时歌单已经创建但首页没加载上下文”的问题，也能为后续继续补齐 TUI 多视图能力打下清晰基础。
