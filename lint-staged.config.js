const quoteFiles = (files) => files.map((file) => `"${file}"`).join(' ')

const hasRustChanges = (files) =>
  files.some((file) => {
    const normalized = file.replace(/\\/g, '/')
    return (
      normalized.endsWith('.rs') ||
      normalized === 'Cargo.toml' ||
      normalized === 'Cargo.lock'
    )
  })

export default {
  '**/*.md': ['rumdl check --fix'],
  '*.{js,jsx,ts,tsx,css,html,json,jsonc}': 'biome check --write',
  '**/*': (files) => {
    if (!hasRustChanges(files)) {
      return []
    }

    const rustFiles = files.filter((file) => file.endsWith('.rs'))
    const commands = []

    if (rustFiles.length > 0) {
      commands.push(`rustfmt --edition 2024 ${quoteFiles(rustFiles)}`)
    }

    commands.push('cargo fmt --all --check')
    commands.push('cargo clippy --no-deps -- -D warnings')

    return commands
  },
}
