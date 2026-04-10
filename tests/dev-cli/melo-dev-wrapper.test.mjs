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
