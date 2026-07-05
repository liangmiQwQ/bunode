import { spawnSync } from 'node:child_process'

for (const args of [['--eval'], ['--inspect=']]) {
  const result = spawnSync(process.execPath, args, {
    encoding: 'utf8',
    env: process.env
  })

  process.stdout.write(`${args.join(' ')}=${result.status}\n`)
}
