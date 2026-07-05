import { spawnSync } from 'node:child_process'

const result = spawnSync(process.execPath, ['-p'], {
  input: '1 + 2\n',
  encoding: 'utf8',
  env: process.env
})

process.stdout.write(result.stdout)
process.stderr.write(result.stderr)

if (result.status !== 0) {
  throw new Error(`print stdin child exited with code ${result.status ?? 1}`)
}
