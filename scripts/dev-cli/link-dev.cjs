#!/usr/bin/env node

const os = require('node:os')
const path = require('node:path')
const { spawnSync } = require('node:child_process')

/**
 * 推导 pnpm 全局 bin 目录。
 *
 * @param {NodeJS.ProcessEnv} [env=process.env] 当前环境变量
 * @param {NodeJS.Platform} [platform=process.platform] 当前平台
 * @param {string} [homeDir=os.homedir()] 当前用户家目录
 * @returns {string} pnpm 全局 bin 目录绝对路径
 */
function resolvePnpmHome(
  env = process.env,
  platform = process.platform,
  homeDir = os.homedir(),
) {
  if (env.PNPM_HOME) {
    return env.PNPM_HOME
  }

  if (platform === 'win32') {
    return path.join(homeDir, 'AppData', 'Local', 'pnpm')
  }

  return path.join(homeDir, '.local', 'share', 'pnpm')
}

/**
 * 解析 pnpm 调用入口。
 *
 * @param {NodeJS.ProcessEnv} [env=process.env] 当前环境变量
 * @param {string} [nodeExecPath=process.execPath] 当前 Node 可执行路径
 * @returns {{command: string, prefixArgs: string[]}} pnpm 启动命令与前置参数
 */
function resolvePnpmCommand(
  env = process.env,
  nodeExecPath = process.execPath,
) {
  if (env.npm_execpath) {
    return {
      command: nodeExecPath,
      prefixArgs: [env.npm_execpath],
    }
  }

  return {
    command: 'pnpm',
    prefixArgs: [],
  }
}

/**
 * 同步执行 pnpm 子命令。
 *
 * @param {string[]} args 要执行的 pnpm 参数
 * @param {{
 *   cwd?: string,
 *   env?: NodeJS.ProcessEnv,
 *   nodeExecPath?: string,
 *   spawnSyncImpl?: typeof spawnSync
 * }} [options={}] 运行选项
 * @returns {{status: number | null, stdout?: string, stderr?: string}} 子进程结果
 */
function runPnpm(args, options = {}) {
  const invocation = resolvePnpmCommand(
    options.env ?? process.env,
    options.nodeExecPath ?? process.execPath,
  )
  const result = (options.spawnSyncImpl ?? spawnSync)(
    invocation.command,
    [...invocation.prefixArgs, ...args],
    {
      cwd: options.cwd,
      env: options.env,
      encoding: 'utf8',
    },
  )

  if (result.error) {
    throw result.error
  }

  if (result.status !== 0) {
    throw new Error(result.stderr || `pnpm ${args.join(' ')} failed`)
  }

  return result
}

/**
 * 确保 pnpm 已为全局二进制设置目标目录。
 *
 * @param {{
 *   cwd?: string,
 *   env?: NodeJS.ProcessEnv,
 *   homeDir?: string,
 *   nodeExecPath?: string,
 *   platform?: NodeJS.Platform,
 *   spawnSyncImpl?: typeof spawnSync
 * }} [options={}] 运行选项
 * @returns {string} 生效中的 pnpm 全局 bin 目录
 */
function ensureGlobalBinDir(options = {}) {
  const env = options.env ?? process.env
  const pnpmHome = resolvePnpmHome(
    env,
    options.platform ?? process.platform,
    options.homeDir ?? os.homedir(),
  )
  const current = runPnpm(
    ['config', 'get', 'global-bin-dir'],
    options,
  ).stdout?.trim()

  if (!current || current === 'undefined') {
    runPnpm(['config', 'set', 'global-bin-dir', pnpmHome], options)
    return pnpmHome
  }

  return current
}

/**
 * 确保当前环境可执行 `pnpm link --global`。
 *
 * @param {{
 *   cwd?: string,
 *   env?: NodeJS.ProcessEnv,
 *   homeDir?: string,
 *   nodeExecPath?: string,
 *   platform?: NodeJS.Platform,
 *   spawnSyncImpl?: typeof spawnSync
 * }} [options={}] 运行选项
 * @returns {number} 子进程退出码
 */
function run(options = {}) {
  const env = { ...(options.env ?? process.env) }
  const pnpmHome = ensureGlobalBinDir({
    ...options,
    env,
  })

  if (!env.PNPM_HOME) {
    env.PNPM_HOME = pnpmHome
  }

  const pathValue = env.Path ?? env.PATH ?? ''
  if (!pathValue.split(path.delimiter).includes(pnpmHome)) {
    const nextPathValue = pathValue
      ? `${pnpmHome}${path.delimiter}${pathValue}`
      : pnpmHome
    env.Path = nextPathValue
    env.PATH = nextPathValue
  }

  return (
    runPnpm(['link', '--global'], {
      ...options,
      env,
    }).status ?? 1
  )
}

if (require.main === module) {
  process.exit(run())
}

module.exports = {
  ensureGlobalBinDir,
  resolvePnpmCommand,
  resolvePnpmHome,
  run,
  runPnpm,
}
