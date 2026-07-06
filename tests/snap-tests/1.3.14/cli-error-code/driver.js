import { spawn, spawnSync } from 'node:child_process'

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

const openStdinPreloadExit = await new Promise((resolve, reject) => {
  const child = spawn(process.execPath, ['--require', './exit42.cjs'], {
    env: process.env,
    stdio: ['pipe', 'ignore', 'pipe']
  })
  let settled = false
  const timer = setTimeout(() => {
    settled = true
    child.kill()
    resolve('timeout')
  }, 2000)

  child.stdin.on('error', error => {
    if (error.code !== 'EPIPE' && !settled) {
      settled = true
      clearTimeout(timer)
      reject(error instanceof Error ? error : new Error(String(error)))
    }
  })
  child.stderr.setEncoding('utf8')
  child.stderr.on('data', chunk => process.stderr.write(chunk))
  child.on('error', reject)
  child.on('exit', code => {
    clearTimeout(timer)

    if (!settled) {
      resolve(`${code ?? 1}`)
    }
  })

  // Keep stdin open so the wrapper must launch preloads before EOF.
  child.stdin.write('0;'.repeat(1000))
})

process.stdout.write(`stdin-preload-open=${openStdinPreloadExit}\n`)
