import { createRequire } from "node:module";

import { afterEach, describe, expect, it, vi } from "vitest";

const require = createRequire(import.meta.url);
const linkDev = require("../../scripts/dev-cli/link-dev.cjs");

describe("link dev helper", () => {
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("sets global-bin-dir before linking when pnpm config is missing", () => {
    const spawnSyncImpl = vi
      .fn()
      .mockReturnValueOnce({ status: 0, stdout: "undefined\r\n", stderr: "" })
      .mockReturnValueOnce({ status: 0, stdout: "", stderr: "" })
      .mockReturnValueOnce({ status: 0, stdout: "", stderr: "" });

    const exitCode = linkDev.run({
      cwd: "D:/coding/Projects/rust/melo",
      env: {
        PNPM_HOME: "C:/Users/dev/AppData/Local/pnpm",
        Path: "C:/Windows/System32",
      },
      homeDir: "C:/Users/dev",
      platform: "win32",
      spawnSyncImpl,
    });

    expect(exitCode).toBe(0);
    expect(spawnSyncImpl).toHaveBeenNthCalledWith(
      1,
      "pnpm",
      ["config", "get", "global-bin-dir"],
      expect.objectContaining({
        cwd: "D:/coding/Projects/rust/melo",
        encoding: "utf8",
      }),
    );
    expect(spawnSyncImpl).toHaveBeenNthCalledWith(
      2,
      "pnpm",
      ["config", "set", "global-bin-dir", "C:/Users/dev/AppData/Local/pnpm"],
      expect.objectContaining({
        cwd: "D:/coding/Projects/rust/melo",
        encoding: "utf8",
      }),
    );
    expect(spawnSyncImpl).toHaveBeenNthCalledWith(
      3,
      "pnpm",
      ["link", "--global"],
      expect.objectContaining({
        cwd: "D:/coding/Projects/rust/melo",
        encoding: "utf8",
        env: expect.objectContaining({
          PNPM_HOME: "C:/Users/dev/AppData/Local/pnpm",
        }),
      }),
    );
  });

  it("links directly when global-bin-dir is already configured", () => {
    const spawnSyncImpl = vi
      .fn()
      .mockReturnValueOnce({
        status: 0,
        stdout: "C:/Users/dev/AppData/Local/pnpm\r\n",
        stderr: "",
      })
      .mockReturnValueOnce({ status: 0, stdout: "", stderr: "" });

    const exitCode = linkDev.run({
      cwd: "D:/coding/Projects/rust/melo",
      env: {
        PNPM_HOME: "C:/Users/dev/AppData/Local/pnpm",
        Path: "C:/Windows/System32",
      },
      homeDir: "C:/Users/dev",
      platform: "win32",
      spawnSyncImpl,
    });

    expect(exitCode).toBe(0);
    expect(spawnSyncImpl).toHaveBeenCalledTimes(2);
    expect(spawnSyncImpl).toHaveBeenNthCalledWith(
      2,
      "pnpm",
      ["link", "--global"],
      expect.objectContaining({
        cwd: "D:/coding/Projects/rust/melo",
        encoding: "utf8",
      }),
    );
  });

  it("falls back to npm_execpath when pnpm is not directly on PATH", () => {
    const spawnSyncImpl = vi
      .fn()
      .mockReturnValueOnce({
        status: 0,
        stdout: "C:/Users/dev/AppData/Local/pnpm\r\n",
        stderr: "",
      })
      .mockReturnValueOnce({ status: 0, stdout: "", stderr: "" });

    const exitCode = linkDev.run({
      cwd: "D:/coding/Projects/rust/melo",
      env: {
        PNPM_HOME: "C:/Users/dev/AppData/Local/pnpm",
        Path: "C:/Windows/System32",
        npm_execpath: "C:/Users/dev/AppData/Local/node/corepack/pnpm.js",
      },
      homeDir: "C:/Users/dev",
      nodeExecPath: "C:/Program Files/nodejs/node.exe",
      platform: "win32",
      spawnSyncImpl,
    });

    expect(exitCode).toBe(0);
    expect(spawnSyncImpl).toHaveBeenNthCalledWith(
      1,
      "C:/Program Files/nodejs/node.exe",
      [
        "C:/Users/dev/AppData/Local/node/corepack/pnpm.js",
        "config",
        "get",
        "global-bin-dir",
      ],
      expect.objectContaining({
        cwd: "D:/coding/Projects/rust/melo",
        encoding: "utf8",
      }),
    );
    expect(spawnSyncImpl).toHaveBeenNthCalledWith(
      2,
      "C:/Program Files/nodejs/node.exe",
      [
        "C:/Users/dev/AppData/Local/node/corepack/pnpm.js",
        "link",
        "--global",
      ],
      expect.objectContaining({
        cwd: "D:/coding/Projects/rust/melo",
        encoding: "utf8",
      }),
    );
  });
});
