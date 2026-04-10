# Melo 全局 CLI 调试工作流设计

日期：2026-04-10

## 1. 目标

为 Melo 提供一套接近日常已安装命令行工具的全局调试体验。

本设计的核心目标是：

- 让开发者可以像使用 `cargo install` 安装出来的工具一样，直接在任意终端执行 `melo`
- 保留 `pnpm link` 风格的全局命令暴露方式，便于日常调试和卸载
- 保留 Cargo 官方推荐的本地安装路径，用于验证真实安装行为
- 在源码变更后自动更新全局可执行的 Rust 二进制，减少手动重装成本
- 让全局调试命令默认绑定仓库内开发配置，但允许外部通过 `MELO_CONFIG` 覆盖

一句话概括：

用 `pnpm link` 暴露全局入口，用 `cargo install --path . --force` 维护真实 Rust 二进制，用 `watchexec` 负责自动同步二者。

## 2. 当前现状与问题

当前仓库已经具备名为 `melo` 的 Rust 二进制目标，但还没有一套稳定的全局调试工作流。

现状中的关键约束有：

- Rust 可执行名已经固定为 `melo`
- 仓库已使用 `pnpm` 维护开发脚本
- 当前 `package.json` 没有 `bin` 字段，无法直接通过 `pnpm link` 暴露全局命令
- 配置加载逻辑优先读取 `MELO_CONFIG`，否则读取当前工作目录下的 `config.toml`
- 数据库默认路径是相对路径 `local/melo.db`

这会带来两个直接问题：

- 如果只是简单把 `cargo run` 暴露成全局命令，从不同目录执行时可能会读错配置或在错误目录创建数据库
- 如果只依赖 `cargo install --path .`，每次改代码后都需要手动重新安装，调试体验不够顺手

## 3. 备选方向与推荐

### 3.1 方案 A：纯 Cargo 安装与 watch

做法：

- 仅使用 `cargo install --path . --force`
- 由 `watchexec` 监听源码变化后自动重装

优点：

- 最贴近 Rust 原生工具链
- 运行路径最接近正式安装行为

缺点：

- 没有 `pnpm link` 风格的全局入口管理
- 不够贴合当前仓库已经依赖 `pnpm` 的现实

### 3.2 方案 B：`pnpm link` 暴露入口，入口每次执行 `cargo run`

做法：

- 在 `package.json` 中声明 `bin`
- 用 `pnpm link --global` 暴露 `melo`
- 包装脚本每次执行时转发到 `cargo run --manifest-path ... -- ...`

优点：

- 改完代码后无需重装，下一次执行天然就是最新逻辑
- 全局命令的安装和卸载体验与 `pnpm link` 一致

缺点：

- 启动速度更慢
- 运行路径与真实安装行为不一致
- 不满足“保留 Cargo 官方安装路径”的目标

### 3.3 方案 C：`pnpm link` 暴露入口，Cargo 安装真实二进制，`watchexec` 负责自动重装

做法：

- 在 `package.json` 中声明 `bin`
- 用 `pnpm link --global` 暴露 `melo`
- 包装脚本负责转发到 Cargo 已安装的 `melo` 二进制
- 用 `cargo install --path . --force` 更新真实 Rust 二进制
- 用 `watchexec` 监听改动后自动执行重装

优点：

- 兼顾 `pnpm link` 的使用习惯和 Cargo 官方安装路径
- 全局命令始终接近真实安装行为
- 源码变更后可自动更新，无需手动重装

缺点：

- 链路比纯 Cargo 或纯转发方案稍复杂
- 需要维护一层很薄的 Node 包装脚本

### 3.4 推荐结论

本阶段选择 **方案 C：`pnpm link` 暴露入口，Cargo 安装真实二进制，`watchexec` 负责自动重装**。

## 4. 本阶段范围

### 4.1 纳入本阶段

- 在 `package.json` 中定义全局命令入口
- 提供 `pnpm link` / `unlink` 风格的调试脚本
- 提供基于 `cargo install --path . --force` 的开发安装脚本
- 提供基于 `watchexec` 的 watch 安装脚本
- 提供仓库内默认开发配置文件路径约定
- 让全局 `melo` 默认使用仓库开发配置，但允许外部 `MELO_CONFIG` 覆盖

### 4.2 不纳入本阶段

- 修改 Rust 侧配置解析规则
- 引入新的配置发现策略
- 改变 `melo` CLI 的业务命令结构
- 发布到 npm registry 或 crates.io
- 处理多仓库或多 worktree 并发共享同一全局入口的高级冲突策略

## 5. 全局命令拓扑

本设计将全局调试工作流拆成三层：

- `pnpm` 负责暴露全局入口
- Cargo 负责构建并安装真实 Rust 二进制
- `watchexec` 负责在源码变化后自动触发重装

高层关系如下：

- 开发者在任意终端执行 `melo`
- 该命令来自 `pnpm link --global` 暴露的 `bin` 入口
- `bin` 入口将命令转发给 Cargo 已安装的 Rust 二进制
- Rust 二进制通过 `cargo install --path . --force` 保持最新

这种拆分的关键价值在于职责清晰：

- `pnpm` 只负责“让命令进入 PATH”
- Cargo 只负责“产出并更新真实二进制”
- `watchexec` 只负责“监听变化并触发更新”

## 6. 包装脚本设计

建议在仓库中新增：

- `bin/melo-dev.cjs`

并在 `package.json` 中新增：

```json
{
  "bin": {
    "melo": "./bin/melo-dev.cjs"
  }
}
```

该脚本只承担以下职责：

- 解析仓库根目录
- 检查当前环境是否已设置 `MELO_CONFIG`
- 如果未设置，则注入仓库内开发配置路径
- 调用 Cargo 已安装的 `melo` 二进制
- 原样透传 CLI 参数和退出码

包装脚本不负责：

- 直接编译 Rust 项目
- 解析业务参数
- 兜底创建数据库
- 改写 Melo 的业务命令语义

包装脚本应保持“薄而机械”，避免把调试逻辑和业务逻辑混在一起。

## 7. 配置行为

当前配置加载逻辑会优先读取 `MELO_CONFIG`，否则回退到当前工作目录下的 `config.toml`，数据库默认路径也依赖相对路径。

基于这一现状，包装脚本必须显式定义默认开发配置行为：

- 如果当前 shell 已显式设置 `MELO_CONFIG`，包装脚本完全尊重外部设置
- 如果当前 shell 未设置 `MELO_CONFIG`，包装脚本自动注入仓库内开发配置路径

推荐在仓库根目录新增一份专门的开发配置，例如：

- `config.dev.toml`

这样可以稳定保证：

- 从任意目录执行 `melo` 时都会绑定同一份开发配置
- 调试数据库不会意外落到调用时的当前目录
- 需要覆盖配置时，开发者仍可通过外部环境变量精确控制

## 8. `package.json` 脚本设计

建议将脚本分成四组。

### 8.1 Cargo 安装组

- `install:dev`
  - 执行 `cargo install --path . --force`

### 8.2 全局入口组

- `link:dev`
  - 执行 `pnpm link --global`
- `unlink:dev`
  - 执行 `pnpm unlink --global`

### 8.3 一键初始化组

- `setup:dev`
  - 顺序执行开发安装与全局链接

建议语义为：

- 先确保 Cargo 全局二进制已安装到最新
- 再确保 `pnpm` 全局入口已正确链接

### 8.4 watch 组

- `watch:install`
  - 使用 `watchexec` 监听改动并执行 `pnpm install:dev`

## 9. `watchexec` 触发策略

`watchexec` 在本设计中的职责是：

- 监听关键源码和配置文件变化
- 触发统一的开发安装脚本

本设计明确要求 `watch:install` 依赖系统 PATH 中存在可执行的 `watchexec`。

推荐安装方式为：

- `cargo install --locked watchexec-cli`

建议监听范围至少包括：

- `src/**/*.rs`
- `Cargo.toml`
- `Cargo.lock`
- `bin/melo-dev.cjs`
- `config.dev.toml`
- `package.json`

建议忽略范围包括：

- `target/`
- `node_modules/`
- `.git/`
- `local/`
- 其他日志或缓存输出目录

这样做的原因是避免 watch 被编译输出再次触发，形成循环重装。

watch 时触发的动作建议统一为：

- `pnpm install:dev`

而不是将完整的 Cargo 命令直接散落在多个脚本或 watch 命令中。

## 10. 推荐日常工作流

推荐的首次初始化步骤：

```powershell
pnpm setup:dev
```

推荐的持续开发步骤：

```powershell
pnpm watch:install
```

推荐的日常使用方式：

```powershell
melo status
melo play
melo db path
```

推荐的临时配置覆盖方式：

```powershell
$env:MELO_CONFIG = "D:/tmp/melo-test-config.toml"
melo status
```

在这一工作流中：

- `setup:dev` 只负责把入口和真实二进制挂好
- `watch:install` 负责把源码改动同步到全局 Rust 二进制
- `melo` 始终像一个已经安装到系统 PATH 的工具一样被使用

## 11. 风险与边界

### 11.1 风险

- 全局入口由 `pnpm link` 管理，因而依赖本机存在可用的 Node / pnpm 环境
- watch 触发的是安装流程而不是单纯构建，因此相较 `cargo run` 方案会更慢
- 包装脚本和 Cargo 安装产物是两个层次，二者路径解析必须保持稳定

### 11.2 边界

- 本阶段不追求“零配置自动发现任意工作树”
- 本阶段默认服务于当前仓库单开发实例
- 若未来需要支持多 worktree 并存，应在后续设计中明确全局入口如何选择当前活跃仓库

## 12. 验收标准

本阶段完成时，应满足以下条件：

- 执行 `pnpm link --global` 后，开发者可以在任意终端直接执行 `melo`
- `melo` 的全局入口来自 `package.json` 的 `bin` 字段，而不是手工 alias
- `cargo install --path . --force` 成为更新真实 Rust 全局二进制的统一方式
- `watchexec` 可以在源码变化后自动触发更新
- 默认情况下，全局 `melo` 使用仓库开发配置
- 当外部设置 `MELO_CONFIG` 时，全局 `melo` 会正确尊重覆盖值
- 调试工作流的运行路径与真实 Cargo 安装行为保持接近

## 13. 与实现计划的关系

本设计明确了三件事：

- 全局入口由谁负责
- 真实二进制由谁负责
- 源码变化后的自动更新由谁负责

后续实现计划只需要继续细化：

- 包装脚本的具体实现方式
- 开发配置文件的最终命名与内容
- `package.json` 脚本的具体命令串
- `watchexec` 的最终命令参数
