import { spawnSync } from 'node:child_process'

const expression = '`${globalThis.fromNodeOptionsValue}:${globalThis.fromNodeOptionsArgv0Patched}`'

const result = spawnSync(process.execPath, ['-p', expression], {
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

const envFileEnv = { ...process.env }
delete envFileEnv.NODE_OPTIONS

const envFileResult = spawnSync(
  process.execPath,
  ['--env-file', 'node-options.env', '-p', expression],
  {
    encoding: 'utf8',
    env: envFileEnv
  }
)

process.stdout.write(envFileResult.stdout)
process.stderr.write(envFileResult.stderr)

if (envFileResult.status !== 0) {
  throw new Error(`env-file NODE_OPTIONS child exited with code ${envFileResult.status ?? 1}`)
}
