#!/usr/bin/env node

const os = require('node:os')
const path = require('node:path')
const { spawnSync } = require('node:child_process')

/**
 * 解析仓库根目录。
 *
 * @param {string} [scriptPath=__filename] 当前脚本绝对路径
 * @returns {string} 仓库根目录绝对路径
 */
function resolveRepoRoot(scriptPath = __filename) {
  return path.resolve(path.dirname(scriptPath), '..')
}

/**
 * 解析 Cargo home 目录。
 *
 * @param {NodeJS.ProcessEnv} [env=process.env] 当前环境变量
 * @param {string} [homeDir=os.homedir()] 当前用户家目录
 * @returns {string} Cargo home 绝对路径
 */
function resolveCargoHome(env = process.env, homeDir = os.homedir()) {
  return env.CARGO_HOME
    ? path.resolve(env.CARGO_HOME)
    : path.join(homeDir, '.cargo')
}

/**
 * 解析 Cargo 安装出来的 `melo` 二进制路径。
 *
 * @param {NodeJS.ProcessEnv} [env=process.env] 当前环境变量
 * @param {NodeJS.Platform} [platform=process.platform] 当前平台
 * @param {string} [homeDir=os.homedir()] 当前用户家目录
 * @returns {string} `melo` 二进制绝对路径
 */
function resolveBinaryPath(
  env = process.env,
  platform = process.platform,
  homeDir = os.homedir(),
) {
  const cargoHome = resolveCargoHome(env, homeDir)
  const binaryName = platform === 'win32' ? 'melo.exe' : 'melo'

  return path.join(cargoHome, 'bin', binaryName)
}

/**
 * 为子进程构造环境变量。
 *
 * @param {NodeJS.ProcessEnv} [env=process.env] 当前环境变量
 * @param {string} [repoRoot=resolveRepoRoot()] 仓库根目录
 * @returns {NodeJS.ProcessEnv} 传给 Rust 二进制的环境变量
 */
function buildChildEnv(env = process.env, repoRoot = resolveRepoRoot()) {
  if (env.MELO_CONFIG) {
    return { ...env }
  }

  return {
    ...env,
    MELO_CONFIG: path.join(repoRoot, 'config.dev.toml'),
  }
}

/**
 * 执行 Cargo 已安装的 `melo` 二进制。
 *
 * @param {string[]} [argv=process.argv.slice(2)] 透传给 `melo` 的参数
 * @param {{
 *   env?: NodeJS.ProcessEnv,
 *   homeDir?: string,
 *   platform?: NodeJS.Platform,
 *   repoRoot?: string,
 *   scriptPath?: string,
 *   spawnSyncImpl?: typeof spawnSync
 * }} [options={}] 运行时注入项，便于测试
 * @returns {number} 子进程退出码
 */
function run(argv = process.argv.slice(2), options = {}) {
  const repoRoot =
    options.repoRoot ?? resolveRepoRoot(options.scriptPath ?? __filename)
  const env = buildChildEnv(options.env ?? process.env, repoRoot)
  const binaryPath = resolveBinaryPath(
    env,
    options.platform ?? process.platform,
    options.homeDir ?? os.homedir(),
  )
  const result = (options.spawnSyncImpl ?? spawnSync)(binaryPath, argv, {
    cwd: repoRoot,
    env,
    stdio: 'inherit',
  })

  if (result.error) {
    throw result.error
  }

  return typeof result.status === 'number' ? result.status : 1
}

if (require.main === module) {
  process.exit(run())
}

module.exports = {
  buildChildEnv,
  resolveBinaryPath,
  resolveCargoHome,
  resolveRepoRoot,
  run,
}
