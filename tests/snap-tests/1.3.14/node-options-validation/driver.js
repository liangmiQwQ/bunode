import { spawnSync } from 'node:child_process'

const cases = [
  ['eval', '--eval 1'],
  ['env-file', '--env-file local.env'],
  ['double-dash', '--'],
  ['operand', 'script.js']
]

for (const [name, nodeOptions] of cases) {
  const result = spawnSync(process.execPath, ['--version'], {
    encoding: 'utf8',
    env: {
      ...process.env,
      NODE_OPTIONS: nodeOptions
    }
  })

  process.stdout.write(`${name}=${result.status ?? 1}\n`)
  process.stdout.write(result.stderr)
}
