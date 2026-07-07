import { spawnSync } from 'node:child_process'

const result = spawnSync(process.execPath, ['-p', '-'], {
  input: `0;/*${'x'.repeat(500_000)}*/ 42\n`,
  encoding: 'utf8',
  env: process.env
})

process.stdout.write(result.stdout)
process.stderr.write(result.stderr)

if (result.status !== 0) {
  throw new Error(`large print stdin child exited with code ${result.status ?? 1}`)
}
