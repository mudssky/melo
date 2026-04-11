const fs = require('node:fs')
const path = require('node:path')
const { execFileSync } = require('node:child_process')

const output = path.resolve(process.cwd(), 'docs/openapi/melo.openapi.json')
const tempOutput = `${output}.tmp`

if (process.argv.includes('--print-url')) {
  console.log('http://127.0.0.1:8080/api/docs/')
  process.exit(0)
}

execFileSync(
  'cargo',
  ['run', '--quiet', '--bin', 'export_openapi', '--', tempOutput],
  { stdio: 'inherit' },
)

const current = fs.existsSync(output) ? fs.readFileSync(output, 'utf8') : ''
const next = fs.readFileSync(tempOutput, 'utf8')
fs.rmSync(tempOutput, { force: true })

if (current !== next) {
  console.error('openapi spec is outdated')
  process.exit(1)
}
