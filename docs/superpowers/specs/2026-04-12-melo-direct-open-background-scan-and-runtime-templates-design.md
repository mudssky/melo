# Melo 目录直开后台补扫与运行时高信息量模板设计

日期：2026-04-12

## 1. 背景

当前 `melo` 在音乐目录中直接执行时，用户容易感觉“卡住了”。

根因不是单一的慢，而是当前实现把多个阶段都串在进入 TUI 之前同步完成：

- 裸 `melo` 会先把当前目录作为 `cwd_dir` 走 direct-open
- `OpenService::open()` 会在进入 TUI 前等待 `ensure_scanned_paths()` 完成
- `ensure_scanned_paths()` 虽然已有 `prewarm_limit` 与 `background_jobs` 配置，但当前实现仍然把剩余文件同步扫完后才返回
- TUI 启动后，底部状态栏已经承载了播放状态、后端、队列、音量、循环、随机、来源和快捷键提示，不适合继续承接长进度信息

这与已有设计预期不一致。

在 [2026-04-11-melo-direct-open-and-ephemeral-playlists-design.md](./2026-04-11-melo-direct-open-and-ephemeral-playlists-design.md) 中，目录型打开原本就被定义为“两阶段”：

- 先预热最小可播放集合
- 再后台继续补扫

但该设计尚未真正落地。

与此同时，用户希望高信息量运行时提示可以按自己的偏好调整显示方式，例如：

- 是否带当前文件名
- 信息顺序如何排列
- 不同提示语句的词序与详细程度

这类文案与普通短标签不同，更适合模板化，但又不能与未来 i18n 设计冲突。

## 2. 目标

本次设计目标如下：

- 让 `melo` 与 `melo <目录>` 在大目录下不再表现为“卡死”
- 让 direct-open 目录路径真正落地为“前台预热 + 后台补扫”
- 进入 TUI 前提供最小但明确的 CLI 启动提示
- 在 TUI 顶部提供固定单行任务条，显示扫描进度
- 任务条默认支持当前处理文件名，完成后自动收起
- 仅为“高信息量运行时提示”引入可覆盖模板，不扩大到所有用户可见文案
- 与既有 i18n spec 保持兼容，不重复发明 locale 机制

## 3. 非目标

本次明确不做：

- 不把所有 CLI / TUI 文案统一改造成模板
- 不改造 API 错误契约、短状态名、按钮名、帮助键位名等固定短文案
- 不一次性实现完整 i18n 基础设施
- 不引入完整“后台任务中心 UI”
- 不实现多任务并列展示、任务历史列表、任务管理命令
- 不为 discovery 阶段引入增量流式目录发现 UI；本次先解决“全量元数据补扫阻塞进入 TUI”的主问题

## 4. 方案结论

本次采用以下组合方案：

### 4.1 目录直开采用正式的两阶段扫描

- 前台只完成轻量发现与同步预热前 `N` 首
- 剩余文件在 daemon 后台继续补扫
- TUI 在预热完成后立即启动

### 4.2 运行时进度作为独立任务状态，而不是塞进 `PlayerSnapshot`

- 播放状态仍归播放器快照负责
- 扫描进度属于后台工作状态，独立建模
- TUI 同时消费播放器状态与任务状态

### 4.3 高信息量运行时提示支持模板 override

- 仅覆盖 CLI 启动提示、TUI 顶端任务条、完成/失败通知
- 用户 override 为全局层，不区分 locale
- 默认模板先由代码内置 catalog 提供
- 未来 i18n 实施后，再把默认模板迁移到 locale 资源层

## 5. 文案分层原则

为避免模板机制滥用，本次将用户可见文案分为三层：

### 5.1 固定短文案

例如：

- 按钮名
- 固定状态名
- 常见短标签
- 快捷键动作名

这类文案不纳入模板 override，未来优先接入 i18n 资源。

### 5.2 高信息量运行时提示

例如：

- 扫描启动提示
- 扫描进行中任务条
- 扫描完成摘要
- 扫描失败摘要

只有这类文案纳入 `templates.runtime.*`。

### 5.3 稳定错误契约

例如：

- `open_target_not_found`
- `unsupported_open_format`
- `open_target_empty`

这类继续保持稳定 code 和默认语义，不开放为用户模板。

## 6. 目录直开数据流

### 6.1 前台阶段

当用户执行以下入口时：

- `melo`
- `melo <目录>`

若最终进入目录型 direct-open，前台阶段执行顺序如下：

1. CLI 先输出扫描启动提示
2. daemon 对目录做轻量发现，只收集候选音频路径
3. 若发现结果为空：
   - 裸 `melo`：保留原有“仍进入 TUI”的语义
   - 显式 `melo <目录>`：维持既有 `open_target_empty` 处理
4. 建立扫描任务并记录总候选数
5. 按发现顺序同步预热前 `open.prewarm_limit` 首
6. 仅用已预热条目创建 / 刷新临时播放列表和当前 queue
7. 返回可播放结果，CLI 输出 handoff 提示并进入 TUI

### 6.2 后台阶段

前台预热完成后：

- daemon 在后台继续补扫剩余路径
- 补扫成功后，按原始发现顺序把新条目追加到：
  - 临时播放列表
  - 当前播放队列
- 当前正在播放的队列项与索引不因补扫而回退或重置

### 6.3 顺序保证

`open.background_jobs` 允许并发读取元数据，但最终提交顺序必须保持与目录发现顺序一致。

推荐做法：

- 每个候选路径带稳定序号
- 后台 worker 可并发处理
- coordinator 负责按序提交连续已完成结果

这样可以同时满足：

- 提高后台补扫吞吐
- 保持用户看到的歌单顺序稳定

## 7. 运行时任务模型

本次不做完整任务中心，但需要一个正式的运行时任务模型，供 TUI 顶端任务条使用。

建议引入以下概念：

### 7.1 `RuntimeTaskKind`

当前至少包含：

- `library_scan`

### 7.2 `RuntimeTaskPhase`

当前至少包含：

- `discovering`
- `prewarming`
- `indexing`
- `completed`
- `failed`

### 7.3 `RuntimeTaskSnapshot`

建议字段：

- `task_id`
- `kind`
- `phase`
- `source_label`
- `discovered_count`
- `indexed_count`
- `queued_count`
- `current_item_name`
- `last_error`

说明：

- `discovered_count` 在 discovery 完成后稳定
- `indexed_count` 表示已成功完成入库的数量
- `queued_count` 表示已实际进入当前上下文队列的数量
- `current_item_name` 仅用于高信息量提示展示，应优先存 basename，而不是完整路径

### 7.4 可见任务策略

本次 TUI 只展示“最近激活的一个可见任务”。

不做：

- 多任务列表
- 历史任务切换
- 任务堆叠显示

这样能聚焦当前问题，避免 scope 膨胀。

## 8. TUI 状态传输方案

当前 `/api/ws/player` 只传 `PlayerSnapshot`，而且 TUI 真实运行循环尚未持续消费 websocket 更新。

本次推荐：

### 8.1 保留现有 `/api/ws/player`

- 继续服务播放器快照消费方
- 不破坏已有测试与兼容性

### 8.2 新增面向 TUI 的聚合流

新增：

- `/api/ws/tui`

返回新的聚合快照，例如：

- `player`
- `active_task`

建议结构：

```text
TuiSnapshot
- player: PlayerSnapshot
- active_task: Option<RuntimeTaskSnapshot>
```

理由：

- 不污染播放器领域模型
- TUI 只维护一个 websocket 连接
- 后续若要增加更多 TUI 专用状态，也有稳定承载位

### 8.3 TUI 客户端切换到聚合流

`TuiClient` 后续应使用 `/api/ws/tui`，并在运行循环中持续接收更新，而不是只在启动时取一次 `status`。

推荐行为：

- 建立 websocket 后先收到一条完整初始快照
- 后续任何播放器变更或任务状态变更，都推送新的 `TuiSnapshot`

## 9. TUI 呈现设计

### 9.1 顶端任务条

TUI 主布局新增顶端单行区域，仅在存在活动或最近完成任务时占位。

布局原则：

- 有任务时固定占一行
- 无任务时完全收起
- 不挤占底部状态栏职责

### 9.2 任务条显示策略

用户已确认采用：

- 顶端固定一行
- 扫描期间持续显示
- 完成后自动收起

默认活动态文案示例：

- `Scanning D:\Music\Aimer... 12 / 240 · Ref:rain.flac`

默认完成态文案示例：

- `Scan complete: 240 tracks indexed`

默认失败态文案示例：

- `Scan failed: permission denied`

### 9.3 小终端适配

为避免挤压内容区：

- 顶端任务条只允许单行
- `source_label` 与 `current_item_name` 均允许截断
- `current_item_name` 优先显示 basename
- 不在底栏重复展示扫描信息

## 10. CLI 呈现设计

CLI 只负责“用户不要误以为程序没响应”，不承担持续实时进度面板职责。

推荐行为：

### 10.1 启动提示

目录型 direct-open 开始时输出一条提示，例如：

- `Scanning D:\Music\Aimer...`

### 10.2 handoff 提示

前台预热完成、即将进入 TUI 时输出一条提示，例如：

- `Launching TUI, background scan continues...`

### 10.3 不做持续刷屏

本次不在 CLI 中持续打印每一步进度，不引入复杂 spinner 或多行动态覆盖。

理由：

- 跨平台终端行为更稳定
- 用户真正停留的位置是 TUI
- CLI 只需解决“黑洞等待”问题

## 11. 运行时模板设计

### 11.1 只覆盖高信息量提示

本次只为以下文案提供模板 override：

- `cli_start`
- `cli_handoff`
- `tui_active`
- `tui_done`
- `tui_failed`

建议配置结构：

```toml
[templates.runtime.scan]
cli_start = "Scanning {{ source_label }}..."
cli_handoff = "Launching TUI, background scan continues..."
tui_active = "Scanning {{ source_label }}... {{ indexed_count }} / {{ discovered_count }} · {{ current_item_name }}"
tui_done = "Scan complete: {{ queued_count }} tracks indexed"
tui_failed = "Scan failed: {{ error_message }}"
```

### 11.2 上下文变量

本次允许的模板变量固定为：

- `source_label`
- `discovered_count`
- `indexed_count`
- `queued_count`
- `current_item_name`
- `error_message`

推荐支持的有限 filter：

- `default`
- `truncate`
- `basename`

不支持：

- include
- import
- 模板继承
- 在模板内写复杂业务逻辑

### 11.3 模板错误容错

override 模板写错时：

- 不能中断 `melo` 启动
- 不能导致 direct-open 失败

本次实现中的固定回退链：

1. 用户 override
2. 代码内置默认模板
3. 最后硬编码兜底

未来 i18n 接入后的扩展回退链：

1. 用户 override
2. 当前 locale 的内置默认模板
3. `en` 内置默认模板
4. 最后硬编码兜底

## 12. 与 i18n spec 的关系

仓库已经存在 [2026-04-11-melo-help-i18n-design.md](./2026-04-11-melo-help-i18n-design.md)，但对应基础设施尚未开发。

本次方案不等待完整 i18n 落地才解决目录直开体验问题，而是采用“先兼容、后接线”的方式：

### 12.1 本次实现

- 默认模板先由代码内置 catalog 提供
- 用户 override 为全局层，不区分 locale
- 本次不引入 `ui.locale` 的新行为

### 12.2 与未来 i18n 的兼容约束

后续 i18n 基础设施落地后：

- 内置默认模板迁移到 locale 资源中
- key 命名遵循现有 i18n key 结构
- 用户 override 仍保持最高优先级

建议未来使用的 i18n key：

- `cli.runtime.scan.start`
- `cli.runtime.scan.handoff`
- `tui.runtime.scan.active`
- `tui.runtime.scan.done`
- `tui.runtime.scan.failed`

这样本次实现不会与既有 i18n spec 冲突，也不会把“用户模板 override”变成 i18n 的替代品。

## 13. 配置设计

本次建议新增配置项：

```toml
[templates.runtime.scan]
cli_start = "..."
cli_handoff = "..."
tui_active = "..."
tui_done = "..."
tui_failed = "..."
```

并保留现有：

```toml
[open]
prewarm_limit = 20
background_jobs = 4
```

语义说明：

- `prewarm_limit`
  - 进入 TUI 前同步预热数量
- `background_jobs`
  - 后台补扫并发度
- `templates.runtime.scan.*`
  - 仅覆盖扫描相关高信息量提示
  - 缺失时回退内置默认值

## 14. 失败语义

### 14.1 目录发现为空

- 裸 `melo`：进入 TUI，并显示已有默认启动提示或空目录提示
- `melo <目录>`：保持既有 `open_target_empty`

### 14.2 后台补扫个别文件失败

- 不中断当前播放
- 不清空已成功加载的队列
- 任务条进入失败态摘要后收起
- 失败详情仅保留在任务状态与日志中

### 14.3 模板渲染失败

- 回退默认模板
- 不让用户主流程失败

## 15. 测试策略

### 15.1 扫描阶段测试

必须覆盖：

- 大目录 direct-open 在预热完成后即可返回，不等待全量补扫结束
- discovery 结果为空时，裸 `melo` 与显式目录打开的行为分流正确
- 后台补扫能最终补全剩余条目

### 15.2 顺序与并发测试

必须覆盖：

- `background_jobs > 1` 时最终追加顺序仍与发现顺序一致
- 当前播放索引不会因后台补扫而错位

### 15.3 CLI 提示测试

必须覆盖：

- 目录型 direct-open 在进入 TUI 前输出启动提示
- handoff 提示在进入 TUI 前输出

### 15.4 TUI 呈现测试

必须覆盖：

- 有活动扫描任务时顶端任务条出现
- 任务完成后自动收起
- 小宽度下任务条文案会截断而不是撑坏布局

### 15.5 模板 override 测试

必须覆盖：

- 用户自定义 `templates.runtime.scan.*` 生效
- 模板中 `current_item_name` 默认可用
- 非法模板会回退默认模板，不导致失败

### 15.6 i18n 兼容测试

本次只需覆盖兼容层，不要求真实多语言资源：

- 运行时模板 key 保持稳定
- 默认模板 catalog 与用户 override 层的回退顺序稳定

## 16. 验收标准

完成后应满足：

- 在大目录中执行 `melo` 或 `melo <目录>` 时，不再表现为长时间无响应
- 进入 TUI 前，CLI 至少给出一条明确扫描提示和一条 handoff 提示
- TUI 顶部在扫描期间显示固定单行任务条
- 默认任务条文案带当前处理文件名
- 任务完成后任务条自动收起
- 底部状态栏不再继续承接扫描进度信息
- 用户可通过 `templates.runtime.scan.*` 覆盖扫描类高信息量提示
- override 为全局层，不区分 locale
- 本次实现与既有 i18n spec 兼容，不引入冲突语义

## 17. 总结

本次设计解决的是两个紧耦合问题：

- 目录直开真正落地为“前台预热 + 后台补扫”
- 高信息量运行时提示建立一条可 override、可兼容未来 i18n 的正式机制

它不是“把所有文案模板化”，也不是“顺手做完完整任务中心和完整 i18n”，而是在控制范围的前提下，把当前最影响体验的卡顿感和信息不可见问题收口为一套能继续扩展的基础能力。
