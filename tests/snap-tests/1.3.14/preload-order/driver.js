import { spawnSync } from 'node:child_process'

const cases = [
  ['lower', 'data:text/javascript,globalThis.dataLoaded=1'],
  ['upper', 'DATA:text/javascript,globalThis.dataLoaded=2']
]

for (const [name, specifier] of cases) {
  const result = spawnSync(process.execPath, ['--import', specifier, '-e', '0'], {
    encoding: 'utf8',
    env: process.env
  })

  process.stdout.write(`${name}DataImportStatus=${result.status}\n`)
  process.stdout.write(
    `${name}DataImportUnsupported=${result.stderr.includes('data URL imports passed to --import are not supported')}\n`
  )
}
