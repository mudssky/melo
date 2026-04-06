const quoteFiles = (files) => files.map((file) => `"${file}"`).join(" ");

const hasRustChanges = (files) =>
  files.some((file) => {
    const normalized = file.replace(/\\/g, "/");
    return (
      normalized.endsWith(".rs") ||
      normalized === "Cargo.toml" ||
      normalized === "Cargo.lock"
    );
  });

module.exports = {
  "**/*.md": ["rumdl check --fix"],
  "**/*": (files) => {
    if (!hasRustChanges(files)) {
      return [];
    }

    const rustFiles = files.filter((file) => file.endsWith(".rs"));
    const commands = [];

    if (rustFiles.length > 0) {
      commands.push(`rustfmt --edition 2024 ${quoteFiles(rustFiles)}`);
    }

    commands.push("cargo fmt --all --check");
    commands.push("cargo clippy --all-targets --all-features -- -D warnings");

    return commands;
  },
};
