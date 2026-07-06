import { spawnSync } from 'node:child_process'
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

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

const dataVersionResult = spawnSync(process.execPath, ['--version'], {
  encoding: 'utf8',
  env: {
    ...process.env,
    NODE_OPTIONS: '--import data:text/javascript,%GG'
  }
})

process.stdout.write(
  `data-version=${dataVersionResult.status}:${dataVersionResult.stdout.trim().startsWith('v')}\n`
)
process.stdout.write(dataVersionResult.stderr)

const tempDirectory = mkdtempSync(join(tmpdir(), 'bunode-node-options-'))
const envFile = join(tempDirectory, 'node-options.env')

try {
  writeFileSync(envFile, 'NODE_OPTIONS="--bad\n')

  const realEnvResult = spawnSync(
    process.execPath,
    ['--env-file', envFile, '-e', 'console.log("real-env-file=ok")'],
    {
      encoding: 'utf8',
      env: {
        ...process.env,
        NODE_OPTIONS: '--conditions custom'
      }
    }
  )

  process.stdout.write(realEnvResult.stdout)
  process.stdout.write(`real-env-file=${realEnvResult.status}\n`)
  process.stdout.write(realEnvResult.stderr)
} finally {
  rmSync(tempDirectory, { recursive: true, force: true })
}
