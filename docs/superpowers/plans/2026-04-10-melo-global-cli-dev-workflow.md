# Melo Global CLI Dev Workflow Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a `pnpm link`-driven global `melo` development workflow that defaults to repo-local dev config, refreshes the real Cargo-installed binary, and can be smoke-tested from any terminal.

**Architecture:** Keep the runtime glue inside a single CommonJS bin shim so `pnpm link --global` only exposes a thin entrypoint while the real executable remains the Cargo-installed `melo` binary. Track the default dev config in-repo, verify the shim with Vitest, and define the package scripts so install/link/watch/qa all flow through one `package.json` contract. Although the spec used the label `unlink:dev`, the implementation should wire that script to `pnpm uninstall --global melo` because pnpm removes globally linked packages through uninstall rather than `unlink`.

**Tech Stack:** Node.js CommonJS bin shim, pnpm 10 scripts, Vitest, Cargo `install --path . --force`, `watchexec-cli`, PowerShell smoke commands

---

## File structure impact

### Existing files to modify

- Modify: `package.json`

### New files to create

- Create: `bin/melo-dev.cjs`
- Create: `config.dev.toml`
- Create: `tests/dev-cli/melo-dev-wrapper.test.mjs`
- Create: `tests/dev-cli/package-json.test.mjs`

### Responsibilities

- `bin/melo-dev.cjs`
  Global `melo` shim that resolves the repo root, applies default config behavior, targets the Cargo-installed binary, and runs it from the repo root so `config.dev.toml` can keep a relative development database path.
- `config.dev.toml`
  Repo-tracked development config whose database path always stays inside the ignored `local/` directory.
- `tests/dev-cli/melo-dev-wrapper.test.mjs`
  Unit tests for the wrapper helpers: config precedence, Cargo binary resolution, and child-process spawning contract.
- `tests/dev-cli/package-json.test.mjs`
  Contract tests for `package.json` `bin` metadata and the install/link/watch/qa script strings.
- `package.json`
  The single source of truth for the global bin entry and the developer workflow commands.

---

### Task 1: Add the global wrapper shim, repo dev config, and failing wrapper tests

**Files:**
- Create: `bin/melo-dev.cjs`
- Create: `config.dev.toml`
- Create: `tests/dev-cli/melo-dev-wrapper.test.mjs`

- [ ] **Step 1: Write the failing wrapper tests**

```js
// tests/dev-cli/melo-dev-wrapper.test.mjs
import path from "node:path";
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";

import { afterEach, describe, expect, it, vi } from "vitest";

const require = createRequire(import.meta.url);
const currentDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(currentDir, "../..");
const wrapper = require("../../bin/melo-dev.cjs");

describe("melo dev wrapper", () => {
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("injects the repo config when MELO_CONFIG is missing", () => {
    const env = wrapper.buildChildEnv({}, repoRoot);

    expect(env.MELO_CONFIG).toBe(path.join(repoRoot, "config.dev.toml"));
  });

  it("preserves a caller-provided MELO_CONFIG override", () => {
    const env = wrapper.buildChildEnv(
      { MELO_CONFIG: "D:/tmp/custom-melo-config.toml" },
      repoRoot,
    );

    expect(env.MELO_CONFIG).toBe("D:/tmp/custom-melo-config.toml");
  });

  it("resolves the cargo-installed binary from CARGO_HOME on Windows", () => {
    const binary = wrapper.resolveBinaryPath(
      { CARGO_HOME: "D:/tools/cargo-home" },
      "win32",
      "C:/Users/dev",
    );

    expect(binary).toBe(path.join("D:/tools/cargo-home", "bin", "melo.exe"));
  });

  it("spawns the installed binary from the repo root with forwarded args", () => {
    const spawnSyncImpl = vi.fn(() => ({ status: 0 }));

    const exitCode = wrapper.run(["status"], {
      env: {},
      homeDir: "C:/Users/dev",
      platform: "win32",
      repoRoot,
      spawnSyncImpl,
    });

    expect(exitCode).toBe(0);
    expect(spawnSyncImpl).toHaveBeenCalledWith(
      path.join("C:/Users/dev", ".cargo", "bin", "melo.exe"),
      ["status"],
      expect.objectContaining({
        cwd: repoRoot,
        stdio: "inherit",
        env: expect.objectContaining({
          MELO_CONFIG: path.join(repoRoot, "config.dev.toml"),
        }),
      }),
    );
  });
});
```

- [ ] **Step 2: Run the wrapper tests to verify they fail**

Run: `pnpm exec vitest run tests/dev-cli/melo-dev-wrapper.test.mjs`  
Expected: FAIL because `bin/melo-dev.cjs` does not exist yet.

- [ ] **Step 3: Implement the wrapper shim and the repo-tracked dev config**

```js
// bin/melo-dev.cjs
#!/usr/bin/env node

const os = require("node:os");
const path = require("node:path");
const { spawnSync } = require("node:child_process");

/**
 * 解析仓库根目录。
 *
 * @param {string} [scriptPath=__filename] 当前脚本绝对路径
 * @returns {string} 仓库根目录绝对路径
 */
function resolveRepoRoot(scriptPath = __filename) {
  return path.resolve(path.dirname(scriptPath), "..");
}

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
  const cargoHome = resolveCargoHome(env, homeDir);
  const binaryName = platform === "win32" ? "melo.exe" : "melo";

  return path.join(cargoHome, "bin", binaryName);
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
    return { ...env };
  }

  return {
    ...env,
    MELO_CONFIG: path.join(repoRoot, "config.dev.toml"),
  };
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
  const repoRoot = options.repoRoot ?? resolveRepoRoot(options.scriptPath ?? __filename);
  const env = buildChildEnv(options.env ?? process.env, repoRoot);
  const binaryPath = resolveBinaryPath(
    env,
    options.platform ?? process.platform,
    options.homeDir ?? os.homedir(),
  );
  const result = (options.spawnSyncImpl ?? spawnSync)(binaryPath, argv, {
    cwd: repoRoot,
    env,
    stdio: "inherit",
  });

  if (result.error) {
    throw result.error;
  }

  return typeof result.status === "number" ? result.status : 1;
}

if (require.main === module) {
  process.exit(run());
}

module.exports = {
  buildChildEnv,
  resolveBinaryPath,
  resolveCargoHome,
  resolveRepoRoot,
  run,
};
```

```toml
# config.dev.toml
[database]
path = "local/melo-dev.db"
```

- [ ] **Step 4: Run the wrapper tests and verify they pass**

Run: `pnpm exec vitest run tests/dev-cli/melo-dev-wrapper.test.mjs`  
Expected: PASS and the wrapper contract is green.

- [ ] **Step 5: Run the existing project QA before committing the wrapper slice**

Run: `pnpm qa`  
Expected: PASS. At this stage `pnpm qa` still covers the Rust pipeline only, so the wrapper test from Step 4 remains the explicit Node-side gate.

- [ ] **Step 6: Commit the wrapper/config slice**

```bash
git add bin/melo-dev.cjs config.dev.toml tests/dev-cli/melo-dev-wrapper.test.mjs
git commit -m "feat: add global melo dev wrapper"
```

---

### Task 2: Add package metadata tests, wire the dev workflow scripts, and fold the wrapper tests into QA

**Files:**
- Modify: `package.json`
- Create: `tests/dev-cli/package-json.test.mjs`

- [ ] **Step 1: Write the failing package metadata test**

```js
// tests/dev-cli/package-json.test.mjs
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { describe, expect, it } from "vitest";

const currentDir = path.dirname(fileURLToPath(import.meta.url));
const packageJson = JSON.parse(
  fs.readFileSync(path.resolve(currentDir, "../../package.json"), "utf8"),
);

describe("package metadata for the global dev CLI workflow", () => {
  it("publishes the global melo bin entry", () => {
    expect(packageJson.bin).toEqual({
      melo: "./bin/melo-dev.cjs",
    });
  });

  it("defines the install/link/watch/qa scripts", () => {
    expect(packageJson.scripts["test:dev-cli"]).toBe("vitest run tests/dev-cli");
    expect(packageJson.scripts["install:dev"]).toBe("cargo install --path . --force");
    expect(packageJson.scripts["link:dev"]).toBe("pnpm link --global");
    expect(packageJson.scripts["unlink:dev"]).toBe("pnpm uninstall --global melo");
    expect(packageJson.scripts["setup:dev"]).toBe("pnpm install:dev && pnpm link:dev");
    expect(packageJson.scripts["watch:install"]).toBe(
      "watchexec --watch src --watch bin --watch-file Cargo.toml --watch-file Cargo.lock --watch-file config.dev.toml --watch-file package.json --ignore target --ignore node_modules --ignore .git --ignore local --shell=none -- pnpm install:dev",
    );
    expect(packageJson.scripts.qa).toBe("pnpm test:dev-cli && pnpm qa:rs");
  });
});
```

- [ ] **Step 2: Run the package metadata test to verify it fails**

Run: `pnpm exec vitest run tests/dev-cli/package-json.test.mjs`  
Expected: FAIL because `package.json` does not yet have the required `bin` entry or workflow scripts.

- [ ] **Step 3: Implement the package metadata and workflow scripts**

```json
{
  "name": "melo",
  "version": "1.0.0",
  "private": true,
  "bin": {
    "melo": "./bin/melo-dev.cjs"
  },
  "scripts": {
    "lint-staged": "lint-staged",
    "format:rs": "cargo fmt --all",
    "format:rs:check": "cargo fmt --all --check",
    "lint:rs": "cargo clippy --all-targets --all-features -- -D warnings",
    "lint:rs:rtk": "rtk cargo clippy --all-targets --all-features -- -D warnings",
    "precommit:rs": "pnpm format:rs:check && pnpm lint:rs",
    "prepare": "husky",
    "test:rs": "cargo test -q",
    "test:rs:rtk": "rtk cargo test -q",
    "test:dev-cli": "vitest run tests/dev-cli",
    "install:dev": "cargo install --path . --force",
    "link:dev": "pnpm link --global",
    "unlink:dev": "pnpm uninstall --global melo",
    "setup:dev": "pnpm install:dev && pnpm link:dev",
    "watch:install": "watchexec --watch src --watch bin --watch-file Cargo.toml --watch-file Cargo.lock --watch-file config.dev.toml --watch-file package.json --ignore target --ignore node_modules --ignore .git --ignore local --shell=none -- pnpm install:dev",
    "qa:rs": "pnpm format:rs && pnpm lint:rs:rtk && pnpm test:rs:rtk",
    "qa": "pnpm test:dev-cli && pnpm qa:rs"
  },
  "packageManager": "pnpm@10.26.1",
  "devDependencies": {
    "husky": "^9.1.7",
    "lint-staged": "^16.4.0",
    "rumdl": "^0.1.67",
    "typescript": "^6.0.2",
    "vitest": "^4.1.4"
  }
}
```

- [ ] **Step 4: Run the package-side tests and verify they pass**

Run: `pnpm run test:dev-cli`  
Expected: PASS and both `tests/dev-cli/melo-dev-wrapper.test.mjs` and `tests/dev-cli/package-json.test.mjs` are green.

- [ ] **Step 5: Run the full project QA and verify the wrapper tests are now part of it**

Run: `pnpm qa`  
Expected: PASS with Vitest first and the Rust format/clippy/test pipeline afterwards.

- [ ] **Step 6: Commit the package metadata and QA wiring**

```bash
git add package.json tests/dev-cli/package-json.test.mjs
git commit -m "feat: add global cli dev workflow scripts"
```

---

### Task 3: Smoke-test the global install/link/watch workflow from a real terminal

**Files:**
- No code changes required. This task is purely verification.

- [ ] **Step 1: Verify the `watchexec` prerequisite or install it once**

Run: `watchexec --version`  
Expected: prints a version line such as `watchexec 2.x`.

If the command is missing, run: `cargo install --locked watchexec-cli`  
Expected: installation completes successfully and `watchexec --version` works afterwards.

- [ ] **Step 2: Run the one-shot setup workflow**

Run: `pnpm setup:dev`  
Expected: `cargo install --path . --force` reinstalls the local crate and `pnpm link --global` makes the local package available system-wide.

- [ ] **Step 3: Smoke-test the globally linked command**

Run: `melo --help`  
Expected: PASS and the output includes `Daemon-backed local music library manager`, `player`, `queue`, and `db`.

- [ ] **Step 4: Verify that an external `MELO_CONFIG` override still wins**

Run this PowerShell block from the repo root:

```powershell
$overridePath = Join-Path $env:TEMP "melo-override.toml"
Set-Content -Path $overridePath -Value "[database]`npath = 'local/melo-override.db'"
$env:MELO_CONFIG = $overridePath
melo db path
```

Expected: PASS and the output is `local/melo-override.db`, proving the wrapper preserved the caller override instead of forcing `config.dev.toml`.

- [ ] **Step 5: Verify that the watch workflow re-runs the install command**

In terminal A, run:

```powershell
pnpm watch:install
```

Expected: `watchexec` starts and waits for file changes.

In terminal B, run:

```powershell
$content = Get-Content config.dev.toml -Raw
[System.IO.File]::WriteAllText((Resolve-Path "config.dev.toml"), $content)
```

Expected in terminal A: one fresh `pnpm install:dev` / `cargo install --path . --force` run is triggered.

Then verify the working tree stayed clean:

Run: `git diff -- config.dev.toml`  
Expected: no output.

- [ ] **Step 6: Confirm the repo is still healthy after the smoke test**

Run: `pnpm qa`  
Expected: PASS.

Run: `git status --short`  
Expected: no output.

---

## Self-review notes

### Spec coverage

- Global `melo` entry via `pnpm link` and `package.json bin`: Task 2 + Task 3
- Default repo dev config with `MELO_CONFIG` override preserved: Task 1 + Task 3
- Cargo official install path retained through `cargo install --path . --force`: Task 2 + Task 3
- `watchexec`-based watch workflow: Task 2 + Task 3
- QA updated so the wrapper is not an untested sidecar: Task 2

### Placeholder scan

- No `TBD` / `TODO` placeholders remain
- Every code-changing step includes explicit file contents
- Every verification step includes exact commands and expected output

### Type consistency

- Wrapper helpers consistently use `resolveRepoRoot`, `resolveCargoHome`, `resolveBinaryPath`, `buildChildEnv`, and `run`
- The repo default config file is consistently named `config.dev.toml`
- The package workflow scripts are consistently named `test:dev-cli`, `install:dev`, `link:dev`, `unlink:dev`, `setup:dev`, and `watch:install`
