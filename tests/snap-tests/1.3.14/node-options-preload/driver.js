import { spawnSync } from 'node:child_process'

const result = spawnSync(process.execPath, ['-p', 'globalThis.fromNodeOptionsValue'], {
  encoding: 'utf8',
  env: {
    ...process.env,
    NODE_OPTIONS: '--require ./preload.js'
  }
})

process.stdout.write(result.stdout)
process.stderr.write(result.stderr)

if (result.status !== 0) {
  throw new Error(`NODE_OPTIONS child exited with code ${result.status ?? 1}`)
}
