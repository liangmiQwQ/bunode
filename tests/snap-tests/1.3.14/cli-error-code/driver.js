import { spawnSync } from 'node:child_process'

for (const args of [['--eval'], ['--inspect=']]) {
  const result = spawnSync(process.execPath, args, {
    encoding: 'utf8',
    env: process.env
  })

  process.stdout.write(`${args.join(' ')}=${result.status}\n`)
}

const preloadExitResult = spawnSync(process.execPath, ['--require', './exit42.cjs'], {
  input: '0;'.repeat(500_000),
  encoding: 'utf8',
  env: process.env
})

process.stdout.write(`stdin-preload-exit=${preloadExitResult.status}\n`)
