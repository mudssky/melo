# Melo 开发安装前受限停止 Daemon 设计

日期：2026-04-11

## 1. 目标

本设计聚焦于修复开发工作流中的一个高频问题：

- `pnpm install:dev`
- `pnpm setup:dev`

在 Windows 上执行 `cargo install --path . --force` 时，如果当前全局 `melo.exe` 仍被 daemon 占用，就会因为目标文件无法替换而失败。

本阶段目标是：

- 在开发安装前先检查当前注册的 Melo daemon 状态
- 仅在能够确认“这是受管理的 daemon”时，才尝试停止它
- 优先走优雅停止，必要时才限制性强杀
- 避免误杀普通前台 `melo` 命令或其它同名进程
- 保留原有 `install:dev` 作为原始低层命令
- 提供一个更安全的高级安装命令供一键工作流复用
- 把逻辑从 `package.json` 命令串中抽出来，放进可测试脚本

一句话概括：

让 `install:dev/setup:dev` 在遇到全局 `melo.exe` 被 daemon 占用时，能够自动、安全地为重新安装让路，而不是先失败再让用户手动处理。

## 2. 当前现状与问题

当前仓库里的开发安装链路是：

- `install:dev` -> `cargo install --path . --force`
- `setup:dev` -> `pnpm install:dev && pnpm link:dev`

这条链路的问题是：

- 它完全不知道 daemon 是否还在运行
- 如果 `~/.cargo/bin/melo.exe` 正被 daemon 进程占用，Windows 会在替换文件时返回 `os error 5`
- 安装脚本本身不会尝试恢复
- 用户只能自己去停进程，再重新执行

这在开发阶段尤其高频，因为：

- daemon 很可能就是最常见的长期运行 `melo.exe`
- 每次改完 CLI/daemon/TUI 之后，开发者都可能重新安装全局 `melo`

## 3. 备选方向与推荐

### 3.1 方案 A：纯强制停止

做法：

- 每次 `install:dev` 开始前，直接强杀 `~/.cargo/bin/melo.exe` 对应进程

优点：

- 逻辑最简单
- 成功率通常高

缺点：

- 太粗暴
- 无法利用已经存在的 daemon 优雅关闭链路
- 容易中断正在做清理的后台流程

### 3.2 方案 B：推荐方案，安装前预检查 daemon 并受限停止

做法：

- 安装前先读 daemon 注册文件
- 只有注册有效、pid 存在、路径精确匹配 `~/.cargo/bin/melo.exe` 时才处理
- 先优雅 `daemon stop`
- 超时后再限制性强杀该 pid
- 然后执行 `cargo install --force`

优点：

- 最贴合“daemon 占用是高频问题”的真实场景
- 避免“先失败一次再恢复”的无效噪音
- 误伤面小

缺点：

- 比简单命令多一个前置检查层

### 3.3 方案 C：install 失败后再恢复

做法：

- 先直接 `cargo install --force`
- 只有在报 `os error 5` 时，才回头 stop/kill daemon 再重试

优点：

- 理论上更少前置动作

缺点：

- 如果 daemon 占用是最常见问题，那每次都会先失败一遍
- 开发者体验更吵

### 3.4 推荐结论

本阶段选择 **方案 B：安装前预检查 daemon 并受限停止**。

## 4. 本阶段范围

### 4.1 纳入本阶段

- 保留 `install:dev` 原始命令
- 增加新的安全安装命令
- 安装前读取 daemon 注册文件
- 优雅停止当前注册 daemon
- 超时后限制性强杀
- 将 `setup:dev`、`watch:install` 复用新安全安装命令
- 为脚本补 Node 层单元测试

### 4.2 不纳入本阶段

- 多实例 daemon 管理
- 跨版本 daemon 迁移逻辑
- 真进程级别的 Node 集成测试
- 自动恢复 daemon 到安装后的运行状态

## 5. 基本策略

### 5.1 总体流程

新的安全安装流程为：

1. 解析当前全局安装的 `melo.exe` 路径
2. 读取全局 daemon 注册文件
3. 判断注册是否对应当前受管理 daemon
4. 若命中，则先执行优雅停止
5. 若优雅停止失败或超时，再限制性强杀
6. 然后执行 `cargo install --path . --force`

而原始：

- `install:dev`

继续保留为直接执行：

- `cargo install --path . --force`

### 5.2 “受管理 daemon”的定义

只有同时满足以下条件，脚本才会主动停止进程：

- 存在 daemon 注册文件
- 注册文件中的 `pid` 当前仍存在
- 该 pid 对应进程路径精确等于 `~/.cargo/bin/melo.exe`

不满足这些条件时：

- 不主动 kill
- 直接进入安装阶段

## 6. 优雅停止策略

### 6.1 优雅停止入口

脚本优先执行：

```text
<installed-melo-binary> daemon stop
```

即直接调用：

- `~/.cargo/bin/melo.exe daemon stop`

而不是依赖 PATH 解析结果。

### 6.2 优雅停止后的确认

脚本不能只看 `daemon stop` 命令的退出码，还必须检查：

- 注册中的 pid 是否真的退出

原因是：

- `daemon stop` 可能返回失败，但进程其实已经退出
- 也可能返回成功，但进程仍残留

最终是否进入强杀，应以“目标 pid 是否仍在运行”为准。

### 6.3 等待窗口

建议在优雅停止后等待一个很短的时间窗口，例如：

- 1000ms 到 2000ms

并在窗口内轮询 pid 是否退出。

## 7. 限制性强杀策略

### 7.1 强杀触发条件

只有在以下情况下才允许强杀：

- 注册有效
- pid 存在
- 进程路径仍然精确等于 `~/.cargo/bin/melo.exe`
- 优雅停止后仍未退出

### 7.2 强杀边界

强杀范围必须限制为：

- 注册文件中的单个 pid
- 且路径精确匹配 `~/.cargo/bin/melo.exe`

明确不允许：

- 按进程名批量杀所有 `melo`
- 杀掉路径不匹配的 Melo 进程

## 8. 降级与异常边界

### 8.1 没有注册文件

行为：

- 认为当前没有可确认的 daemon
- 不做停止动作
- 直接安装

### 8.2 注册文件陈旧

表现：

- 注册文件存在
- 但 pid 不存在

行为：

- 清理注册文件
- 直接安装

### 8.3 路径不匹配

表现：

- pid 存在
- 但进程路径不是 `~/.cargo/bin/melo.exe`

行为：

- 不做停止动作
- 直接安装

### 8.4 优雅停止失败但进程已退出

行为：

- 视为恢复成功
- 继续安装

### 8.5 强杀后仍未退出

行为：

- 直接中止安装
- 明确输出 pid 和路径
- 提示用户手动处理

### 8.6 install 失败但不是文件占用

行为：

- 不尝试 stop/kill 恢复
- 直接抛出原始安装错误

## 9. 脚本结构

### 9.1 新脚本

建议新增：

- `scripts/dev-cli/install-dev.cjs`

### 9.2 `package.json` 调整

建议改为：

- `install:dev` 保持不变
- 新增 `install:dev:safe` 调新脚本
- `setup:dev` 改为先 `install:dev:safe` 再 `link:dev`
- `watch:install` 改为走 `install:dev:safe`

### 9.3 关键函数分解

建议脚本至少拆出：

- `resolveInstalledBinaryPath()`
- `loadRegisteredDaemon()`
- `matchesManagedDaemon()`
- `stopRegisteredDaemon()`
- `runCargoInstall()`
- `run()`

这样测试时可以稳定注入：

- `spawnSync`
- 文件读取
- 时间等待
- 进程状态探测

## 10. 输出提示策略

建议脚本输出保持英文开发向信息，代码注释继续用中文。

推荐提示包括：

- `Detected running Melo daemon, stopping before reinstall...`
- `Daemon stopped cleanly. Continuing install...`
- `Daemon did not exit in time, force-stopping registered process...`
- `Failed to stop registered daemon process <pid>. Please stop it manually and retry.`

## 11. 测试策略

### 11.1 Node 单元测试

本阶段仅做 Node 层单元测试，不做真进程集成测试。

### 11.2 必须覆盖的场景

- 没有注册文件时直接安装
- 注册文件陈旧时清理并安装
- 注册有效且路径匹配时，先 stop 再安装
- stop 超时后会强杀同一个 pid
- 路径不匹配时不会误杀
- 非文件占用类 install 失败会直接抛出

## 12. 验收标准

本阶段完成时，应满足以下条件：

- 原始 `pnpm install:dev` 继续保留，不改变其低层语义
- 新的安全安装命令在 daemon 占用 `~/.cargo/bin/melo.exe` 时不会直接卡死
- 脚本会先检查受管理 daemon 状态
- 优雅停止优先于强制停止
- 强杀范围严格限制为注册文件中的同一 pid 且路径精确匹配
- 非 daemon 场景不会被误杀
- `setup:dev` 自动复用这一恢复策略

## 13. 后续扩展点

本设计完成后，可自然继续推进：

- 安装后自动重新拉起 daemon 的可选行为
- 脚本输出分级与 `--verbose`
- 对旧版本 daemon 的更细粒度兼容判断
- 开发脚本统一诊断命令
