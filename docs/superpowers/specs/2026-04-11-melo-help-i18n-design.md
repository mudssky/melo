# Melo Help I18n 设计文档

日期：2026-04-11

## 1. 背景与目标

当前 Melo 的 CLI 帮助文案主要直接写在 `src/cli/args.rs` 的 `clap` 属性中，TUI 帮助提示则直接写在 `src/tui/ui/popup.rs` 中。

现状问题如下：

- `melo --help` 及各级子命令帮助仅支持英文
- TUI 帮助弹窗与快捷键提示仅支持英文
- 帮助文本分散在代码中，不利于维护和扩展
- 后续如果要扩展到更多 TUI 文案，没有现成的语言选择与翻译资源基础设施

本次设计目标：

- 为 CLI help 和 TUI help 建立可扩展的 i18n 基础设施
- 首批支持 `zh-CN`、`en`、`ja`
- 默认跟随系统环境变量
- 支持通过配置文件覆盖语言
- 允许 locale 归一化与 fallback
- 确保后续扩展到更多 TUI 界面文案时不需要推翻本次设计

## 2. 范围与非目标

### 2.1 本次范围

- `melo --help`
- `melo <command> --help`
- `melo <command> <subcommand> --help`
- CLI 参数说明、命令说明、示例说明等帮助文案
- TUI 帮助弹窗、快捷键提示等 help 文案
- locale 选择、归一化与 fallback 机制
- i18n 资源目录与 key 组织规范
- 面向 CLI/TUI help 的测试补全

### 2.2 本次明确不做

- 不本地化命令名、子命令名、参数名
- 不一次性迁移所有 TUI 普通界面文案
- 不修改 API 返回错误文本、日志文本、内部调试文本
- 不支持在线热更新翻译资源
- 不支持用户自定义语言包

## 3. 方案选择

本次考虑过三类方案：

### 3.1 保留 `clap derive` 静态字符串，仅做局部 i18n 补丁

优点：

- 初期改动最小

缺点：

- 运行时多语言接入困难
- 文案会继续散落在 `derive` 属性中
- 后续扩展到更多 help 与 TUI 文案时成本持续上升

结论：

- 不作为正式方案

### 3.2 `rust-i18n + YAML + 统一 help 文案模型`

优点：

- 适合当前 CLI/TUI 帮助文案体量
- 资源文件可读性高，维护门槛低
- 支持 fallback locale 与运行时切换
- 适合后续逐步扩展到更多 TUI 文案

缺点：

- 需要从当前静态 `clap` 文案结构中抽出一层本地化 help 元数据

结论：

- 作为本次推荐方案

### 3.3 Fluent / FTL 路线

优点：

- 复杂自然语言表达能力更强

缺点：

- 学习成本和接入复杂度更高
- 当前 Melo 的 help 场景尚不足以证明需要更重的语义能力

结论：

- 暂不采用

## 4. i18n 库与资源格式

本次选型：

- i18n 库：`rust-i18n`
- 资源格式：`YAML`

选择理由：

- `rust-i18n` 支持编译期加载资源、运行时设置 locale、fallback locale 链与 locale territory fallback
- YAML 对当前这种“短标签 + 短句 + 示例说明”为主的内容最友好
- Melo 团队后续维护翻译资源时，不需要额外学习 Fluent 语法
- 对 CLI/TUI 这种终端产品而言，键值型资源足以覆盖当前阶段需求

## 5. 语言选择与优先级

### 5.1 支持语言

正式支持以下 3 种语言：

- `en`
- `zh-CN`
- `ja`

### 5.2 语言来源优先级

当前正式优先级定义为：

1. 未来 CLI 显式参数
2. 配置文件
3. 系统环境变量
4. 默认英文 `en`

本次实现中至少需要落地：

- 配置文件覆盖
- 系统环境变量检测
- 默认英文兜底

即使当前还未提供 CLI 显式参数，也要在设计上保留这一优先级位置，避免未来扩展时改变语义。

### 5.3 配置文件约定

语言配置统一放入新的 `[ui]` 配置段，字段名为 `locale`。

示例：

```toml
[ui]
locale = "zh-CN"
```

允许值：

- `"system"`：显式表示跟随系统环境变量
- `"en"`
- `"zh-CN"`
- `"ja"`

默认值：

- `system`

如果配置项缺失，也等价于 `system`。

### 5.4 系统环境变量来源

为保证跨平台行为可预测且易于测试，本次仅通过环境变量读取系统 locale，不接入平台原生区域设置 API。

环境变量读取顺序固定为：

1. `LC_ALL`
2. `LC_MESSAGES`
3. `LANG`

取第一个非空值参与 locale 归一化。

### 5.5 locale 归一化

运行时不直接使用原始环境变量值，而是统一做 locale 归一化，将各种系统形式映射到稳定的内部语言标识。

建议映射规则：

- `en`、`en_US`、`en-US`、`en_US.UTF-8` -> `en`
- `zh`、`zh_CN`、`zh-CN`、`zh_CN.UTF-8` -> `zh-CN`
- `ja`、`ja_JP`、`ja-JP`、`ja_JP.UTF-8` -> `ja`

无法识别的值：

- 不报错
- 静默回退到 `en`

## 6. 资源目录与 Key 结构

建议从一开始就按领域拆分资源文件，而不是所有语言只放单文件。

推荐目录结构：

```text
locales/
├── en/
│   ├── cli.yml
│   └── tui.yml
├── zh-CN/
│   ├── cli.yml
│   └── tui.yml
└── ja/
    ├── cli.yml
    └── tui.yml
```

拆分原则：

- `cli.yml` 仅存 CLI help 相关文案
- `tui.yml` 仅存 TUI help 以及未来 TUI UI 文案

### 6.1 Key 命名规则

统一采用“领域.对象.属性”风格，避免把自然语言原文当 key。

示例：

- `cli.root.about`
- `cli.root.long_about`
- `cli.root.examples.status`
- `cli.command.library.about`
- `cli.command.library.long_about`
- `cli.command.playlist.examples.cleanup`
- `cli.command.queue.subcommand.show.about`
- `cli.command.player.subcommand.mode.repeat.about`
- `tui.help.action.toggle`
- `tui.help.action.next`
- `tui.help.action.previous`
- `tui.help.action.quit`

### 6.2 资源维护原则

- 英文 `en` 作为基准语言
- `zh-CN` 与 `ja` 允许运行时 key 级 fallback 到 `en`
- 测试中仍要尽量发现非英文资源缺失
- key 必须稳定，不因文案改写而频繁重命名

## 7. 整体架构

本次 i18n 设计拆成 4 层：

### 7.1 locale 决策层

职责：

- 读取配置文件中的语言配置
- 读取系统环境变量中的 locale 信息
- 应用优先级与归一化规则
- 决定最终生效的 locale

输出：

- 稳定的内部 locale 值，只允许为 `en`、`zh-CN`、`ja`

### 7.2 i18n 资源层

职责：

- 通过 `rust-i18n` 加载 YAML 资源
- 提供按 key 获取翻译文本的统一入口
- 结合 fallback 机制，在翻译缺失时回退英文

### 7.3 help 文案适配层

职责：

- 为 CLI help 提供结构化说明文本
- 为 TUI help 提供结构化动作文本
- 把“文案是什么”从“界面如何渲染/命令如何解析”中分离出来

### 7.4 验证层

职责：

- 保证不同语言下 help 输出正确
- 保证 locale 决策逻辑稳定
- 尽早发现资源缺失与 fallback 异常

## 8. CLI Help 接入设计

### 8.1 目标

保留现有 CLI 的命令面和解析行为，不改变：

- 命令名
- 子命令名
- 参数名
- 参数语义

只重构“帮助文案从哪里来”。

### 8.2 当前问题

`src/cli/args.rs` 目前把大量帮助文本直接写在 `#[command(...)]` 属性里，例如：

- `about`
- `long_about`
- `after_help`

这会导致：

- 运行时多语言切换困难
- 文案散落在多个枚举与字段声明上
- 测试只能围绕英文原句断言

### 8.3 建议实现方向

CLI 结构继续使用 `Parser` / `Subcommand` 负责参数解析，但 help 文案本身抽出为独立的本地化元数据。

设计上建议形成两部分：

- 命令解析结构：保留在现有 CLI 参数定义中
- 帮助展示文案：由新的本地化 help 元数据层提供

这意味着：

- 解析仍依赖 clap
- 展示文案不再完全依赖静态 `derive` 字符串

### 8.4 CLI help 的本地化范围

本次应本地化：

- 顶层命令 `about`
- 顶层命令 `long_about`
- 各一级命令和二级命令的 `about`
- 复杂命令的 `long_about`
- 帮助中的示例文本
- 参数说明文本

本次不本地化：

- 命令 token 本身，如 `playlist`、`queue`
- 选项 token 本身，如 `--preview`
- 路径、示例命令中的实际 CLI 标识符

### 8.5 clap 结构词的处理

对于 `Usage:`、`Commands:`、`Options:` 这类 clap 自动生成的结构词，本次设计分层处理：

- 第一阶段：优先确保业务 help 文案完成本地化
- 第二阶段：如有必要，再评估是否接管更深的 help 模板，使 clap 框架词也可本地化

这样可以降低首轮改造风险，先把对用户最关键的说明文本落地。

## 9. TUI Help 接入设计

### 9.1 当前范围

本次只覆盖 TUI 中的帮助相关文案，例如 `src/tui/ui/popup.rs` 当前的快捷键提示。

### 9.2 设计原则

TUI help 文案采用“按键标签与动作描述分离”的方式建模：

- 按键标签由代码维护
- 动作描述由 i18n 资源提供

示例：

- `Space: Toggle`
- `>: Next`
- `<: Previous`
- `q: Quit`

资源层只保存：

- `tui.help.action.toggle`
- `tui.help.action.next`
- `tui.help.action.previous`
- `tui.help.action.quit`

运行时渲染时，由代码将按键和动作描述拼接为最终显示文本。

### 9.3 这样设计的原因

- 键位变更不会影响翻译资源结构
- 同一动作描述可以在多个 UI 位置复用
- 后续更多 TUI 文案接入时，可继续沿用同一套 locale 与资源读取机制

### 9.4 后续扩展约束

虽然本次不迁移全部 TUI 文案，但从规范上应开始约束：

- 后续新增正式用户可见 TUI 文案，优先接入 i18n
- 现有硬编码英文文案逐步迁移

这样本次实现不会成为 help 专用孤岛，而是未来 TUI 文案国际化的正式基础设施。

## 10. Fallback 与容错策略

整体原则：

- help 文案国际化失败不能导致命令不可用

具体规则：

### 10.1 语言值无效

- 如果配置文件或环境变量指定的语言不受支持
- 不报错
- 静默回退到 `en`

### 10.2 单个翻译 key 缺失

- 仅该 key 回退到 `en`
- 不影响其他已存在翻译的文本

### 10.3 英文资源也缺失

这是开发缺陷，应通过测试尽早发现。

运行时行为要求：

- 尽量退化为稳定的占位输出或可识别文本
- 不因为 help 文案缺失直接 panic 或中断 CLI/TUI 启动

## 11. 配置与数据流

本次需要在配置模型中增加语言配置入口，用于覆盖系统环境变量。

建议新增：

- `UiSettings`
- `Settings.ui`
- `ui.locale`

其中 `ui.locale` 采用可选字符串或等价枚举形式，语义固定为：

- 缺失或 `system`：跟随系统环境变量
- `en` / `zh-CN` / `ja`：强制使用指定语言

数据流如下：

1. 启动时读取配置文件
2. 读取 `ui.locale`
3. 若 `ui.locale` 为缺失或 `system`，再读取系统环境变量 locale
4. 应用语言优先级
5. 执行 locale 归一化
6. 设置当前运行时 locale
7. CLI help / TUI help 从同一套 i18n 资源读取文本

这样可以保证：

- CLI help 与 TUI help 使用同一语言
- 用户通过配置文件设置语言后，CLI 与 TUI 表现一致

## 12. 测试策略

### 12.1 locale 决策测试

覆盖内容：

- 配置文件优先于环境变量
- 环境变量生效
- 默认回退到英文
- `zh_CN.UTF-8`、`ja_JP.UTF-8`、`en_US.UTF-8` 等映射正确
- 非法 locale 值回退英文

### 12.2 CLI help 集成测试

扩展 `tests/cli_help.rs` 或拆分出对应测试文件，覆盖：

- `melo --help`
- `melo library --help`
- `melo playlist --help`
- `melo db --help`

每类至少验证：

- 英文输出
- 中文输出
- 日文输出
- fallback 行为
- 命令 token 未被翻译

### 12.3 TUI help 单元测试

覆盖内容：

- 不同 locale 下的快捷键帮助文本渲染
- 按键标签保持不变
- 动作描述随 locale 变化

### 12.4 资源完整性测试

覆盖内容：

- `en` 资源作为基线必须完整
- `zh-CN` 与 `ja` 至少覆盖本次纳入范围的 key
- 对缺失 key 给出清晰测试失败信息

## 13. 实施边界与迁移顺序

建议实施顺序如下：

1. 引入 `rust-i18n` 依赖
2. 建立 `locales/` 目录与三种语言资源骨架
3. 增加 locale 决策与归一化层
4. 重构 CLI help 文案来源
5. 接入 TUI help 文案
6. 补齐 locale、CLI help、TUI help、资源完整性测试

## 14. 验收标准

完成后应满足：

- 在未配置语言时，help 默认跟随系统环境变量
- 在配置文件中设置语言后，help 使用配置值覆盖系统环境变量
- `melo --help` 与关键子命令 help 支持英文、中文、日文输出
- TUI help 提示支持英文、中文、日文输出
- 不支持的 locale 自动回退到英文
- 缺失翻译 key 不会导致程序报错
- 测试可以稳定覆盖语言选择、help 输出与 fallback 逻辑

## 15. 总结

本次 Melo help i18n 的核心不是单纯“把英文换成多语言”，而是为 CLI help 与 TUI help 建立一条统一、可扩展的国际化链路。

通过采用 `rust-i18n + YAML`、固定 locale 选择优先级、引入资源目录和稳定 key 结构、并将 CLI/TUI help 接入同一套 locale 决策与 fallback 机制，可以在控制本次范围的前提下，为后续 TUI 全文案国际化打下稳定基础。
