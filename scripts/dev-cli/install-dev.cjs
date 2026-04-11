#!/usr/bin/env node

const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { spawnSync } = require("node:child_process");

/**
 * 解析 Cargo home 目录。
 *
 * @param {NodeJS.ProcessEnv} [env=process.env] 当前环境变量
 * @param {string} [homeDir=os.homedir()] 当前用户家目录
 * @returns {string} Cargo home 绝对路径
 */
function resolveCargoHome(env = process.env, homeDir = os.homedir()) {
  return env.CARGO_HOME ? path.resolve(env.CARGO_HOME) : path.join(homeDir, ".cargo");
}

/**
 * 解析当前已安装的 `melo` 二进制路径。
 *
 * @param {NodeJS.ProcessEnv} [env=process.env] 当前环境变量
 * @param {NodeJS.Platform} [platform=process.platform] 当前平台
 * @param {string} [homeDir=os.homedir()] 当前用户家目录
 * @returns {string} `melo` 二进制绝对路径
 */
function resolveInstalledBinaryPath(
  env = process.env,
  platform = process.platform,
  homeDir = os.homedir(),
) {
  const cargoHome = resolveCargoHome(env, homeDir);
  const binaryName = platform === "win32" ? "melo.exe" : "melo";
  return path.join(cargoHome, "bin", binaryName);
}

/**
 * 解析 daemon 注册文件路径。
 *
 * @param {NodeJS.ProcessEnv} [env=process.env] 当前环境变量
 * @param {NodeJS.Platform} [platform=process.platform] 当前平台
 * @param {string} [homeDir=os.homedir()] 当前用户家目录
 * @returns {string} daemon 注册文件路径
 */
function resolveDaemonStatePath(
  env = process.env,
  platform = process.platform,
  homeDir = os.homedir(),
) {
  if (env.MELO_DAEMON_STATE_FILE) {
    return env.MELO_DAEMON_STATE_FILE;
  }

  if (platform === "win32") {
    const localAppData =
      env.LOCALAPPDATA ?? path.join(homeDir, "AppData", "Local");
    return path.join(localAppData, "melo", "daemon.json");
  }

  return path.join(homeDir, ".local", "share", "melo", "daemon.json");
}

/**
 * 同步执行一个子进程命令。
 *
 * @param {string} command 要执行的命令
 * @param {string[]} args 命令参数
 * @param {{
 *   cwd?: string,
 *   env?: NodeJS.ProcessEnv,
 *   encoding?: BufferEncoding,
 *   spawnSyncImpl?: typeof spawnSync
 * }} [options={}] 运行选项
 * @returns {{ status: number | null, stdout?: string, stderr?: string }} 子进程结果
 */
function runCommand(command, args, options = {}) {
  const result = (options.spawnSyncImpl ?? spawnSync)(command, args, {
    cwd: options.cwd ?? options.repoRoot,
    env: options.env,
    encoding: options.encoding ?? "utf8",
  });

  if (result.error) {
    throw result.error;
  }

  return result;
}

/**
 * 读取当前 daemon 注册信息。
 *
 * @param {{
 *   daemonStatePath?: string,
 *   env?: NodeJS.ProcessEnv,
 *   existsSyncImpl?: typeof fs.existsSync,
 *   homeDir?: string,
 *   platform?: NodeJS.Platform,
 *   readFileSyncImpl?: typeof fs.readFileSync
 * }} [options={}] 运行选项
 * @returns {{ pid?: number } | null} 注册信息，不存在时返回 `null`
 */
function loadRegisteredDaemon(options = {}) {
  const daemonStatePath =
    options.daemonStatePath ??
    resolveDaemonStatePath(
      options.env ?? process.env,
      options.platform ?? process.platform,
      options.homeDir ?? os.homedir(),
    );
  const existsSyncImpl = options.existsSyncImpl ?? fs.existsSync;
  if (!existsSyncImpl(daemonStatePath)) {
    return null;
  }

  const readFileSyncImpl = options.readFileSyncImpl ?? fs.readFileSync;
  return JSON.parse(readFileSyncImpl(daemonStatePath, "utf8"));
}

/**
 * 查询指定 pid 的进程信息。
 *
 * @param {number} pid 目标进程 ID
 * @param {{
 *   cwd?: string,
 *   env?: NodeJS.ProcessEnv,
 *   platform?: NodeJS.Platform,
 *   spawnSyncImpl?: typeof spawnSync
 * }} [options={}] 运行选项
 * @returns {{ pid: number, path: string } | null} 存活时返回进程信息
 */
function queryProcessInfo(pid, options = {}) {
  if (typeof pid !== "number") {
    return null;
  }

  if ((options.platform ?? process.platform) === "win32") {
    const command = [
      "$process = Get-Process -Id ",
      String(pid),
      " -ErrorAction SilentlyContinue; ",
      "if ($null -eq $process) { exit 1 }; ",
      "[pscustomobject]@{ pid = $process.Id; path = $process.Path } | ConvertTo-Json -Compress",
    ].join("");
    const result = runCommand(
      "pwsh",
      ["-NoLogo", "-Command", command],
      options,
    );
    if (result.status !== 0 || !result.stdout?.trim()) {
      return null;
    }
    return JSON.parse(result.stdout);
  }

  const result = runCommand(
    "ps",
    ["-p", String(pid), "-o", "pid=", "-o", "comm="],
    options,
  );
  if (result.status !== 0 || !result.stdout?.trim()) {
    return null;
  }
  const [resolvedPid, executablePath] = result.stdout.trim().split(/\s+/, 2);
  return {
    pid: Number(resolvedPid),
    path: executablePath,
  };
}

/**
 * 判断注册的进程是否是当前脚本允许处理的受管 daemon。
 *
 * @param {{ pid: number, path: string } | null} processInfo 当前进程信息
 * @param {string} installedBinaryPath 当前全局 `melo` 二进制路径
 * @param {NodeJS.Platform} [platform=process.platform] 当前平台
 * @returns {boolean} 是否是本次允许自动停止的 daemon
 */
function matchesManagedDaemon(
  processInfo,
  installedBinaryPath,
  platform = process.platform,
) {
  if (!processInfo?.path) {
    return false;
  }

  const normalize = (value) =>
    platform === "win32" ? value.toLowerCase() : value;
  return normalize(processInfo.path) === normalize(installedBinaryPath);
}

/**
 * 同步等待一小段时间，便于轮询进程退出。
 *
 * @param {number} milliseconds 等待毫秒数
 * @returns {void}
 */
function sleepSync(milliseconds) {
  Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, milliseconds);
}

/**
 * 运行 cargo 安装命令。
 *
 * @param {{
 *   cwd?: string,
 *   env?: NodeJS.ProcessEnv,
 *   spawnSyncImpl?: typeof spawnSync
 * }} [options={}] 运行选项
 * @returns {number} 安装命令退出码
 */
function runCargoInstall(options = {}) {
  const result = runCommand(
    "cargo",
    ["install", "--path", ".", "--force"],
    options,
  );

  if (result.status !== 0) {
    throw new Error(result.stderr || "cargo install --path . --force failed");
  }

  return result.status ?? 1;
}

/**
 * 停止当前注册的受管 daemon，必要时限制性强杀。
 *
 * @param {{
 *   cwd?: string,
 *   daemonStatePath?: string,
 *   env?: NodeJS.ProcessEnv,
 *   existsSyncImpl?: typeof fs.existsSync,
 *   homeDir?: string,
 *   platform?: NodeJS.Platform,
 *   readFileSyncImpl?: typeof fs.readFileSync,
 *   spawnSyncImpl?: typeof spawnSync
 * }} [options={}] 运行选项
 * @returns {void}
 */
function stopRegisteredDaemon(options = {}) {
  const installedBinaryPath = resolveInstalledBinaryPath(
    options.env ?? process.env,
    options.platform ?? process.platform,
    options.homeDir ?? os.homedir(),
  );
  const registration = loadRegisteredDaemon(options);
  if (!registration?.pid) {
    return;
  }

  const processInfo = queryProcessInfo(registration.pid, options);
  if (!matchesManagedDaemon(processInfo, installedBinaryPath, options.platform)) {
    return;
  }

  console.log("Detected running Melo daemon, stopping before reinstall...");
  runCommand(installedBinaryPath, ["daemon", "stop"], options);

  for (let attempt = 0; attempt < 5; attempt += 1) {
    sleepSync(200);
    const current = queryProcessInfo(registration.pid, options);
    if (!current) {
      console.log("Daemon stopped cleanly. Continuing install...");
      return;
    }
  }

  console.log("Daemon did not exit in time, force-stopping registered process...");
  runCommand(
    "pwsh",
    [
      "-NoLogo",
      "-Command",
      `Stop-Process -Id ${registration.pid} -Force`,
    ],
    options,
  );
}

/**
 * 执行安全的开发安装流程。
 *
 * @param {{
 *   cwd?: string,
 *   daemonStatePath?: string,
 *   env?: NodeJS.ProcessEnv,
 *   existsSyncImpl?: typeof fs.existsSync,
 *   homeDir?: string,
 *   platform?: NodeJS.Platform,
 *   readFileSyncImpl?: typeof fs.readFileSync,
 *   repoRoot?: string,
 *   spawnSyncImpl?: typeof spawnSync
 * }} [options={}] 运行选项
 * @returns {number} 安装流程退出码
 */
function run(options = {}) {
  stopRegisteredDaemon(options);
  return runCargoInstall({
    cwd: options.repoRoot ?? options.cwd,
    env: options.env,
    spawnSyncImpl: options.spawnSyncImpl,
  });
}

if (require.main === module) {
  process.exit(run({ repoRoot: path.resolve(__dirname, "../..") }));
}

module.exports = {
  loadRegisteredDaemon,
  matchesManagedDaemon,
  queryProcessInfo,
  resolveCargoHome,
  resolveDaemonStatePath,
  resolveInstalledBinaryPath,
  run,
  runCargoInstall,
  runCommand,
  stopRegisteredDaemon,
};
