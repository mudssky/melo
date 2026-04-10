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
    expect(packageJson.scripts["link:dev"]).toBe("node ./scripts/dev-cli/link-dev.cjs");
    expect(packageJson.scripts["unlink:dev"]).toBe("pnpm uninstall --global melo");
    expect(packageJson.scripts["setup:dev"]).toBe("pnpm install:dev && pnpm link:dev");
    expect(packageJson.scripts["watch:install"]).toBe(
      "watchexec --postpone --watch src --watch bin --watch Cargo.toml --watch Cargo.lock --watch config.dev.toml --watch package.json --ignore target --ignore node_modules --ignore .git --ignore local --shell=none -- pnpm install:dev",
    );
    expect(packageJson.scripts.qa).toBe("pnpm test:dev-cli && pnpm qa:rs");
  });
});
