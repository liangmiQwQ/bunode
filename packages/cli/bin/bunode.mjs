#!/usr/bin/env node
try {
  const { runCli } = await import('../dist/cli.mjs')
  await runCli(import.meta.url)
} catch (error) {
  const { spawn } = await import('node:child_process')
  const { access } = await import('node:fs/promises')
  const { homedir } = await import('node:os')
  const { join } = await import('node:path')

  const binary = join(homedir(), '.bunode', process.platform === 'win32' ? 'bunode.exe' : 'bunode')

  process.stderr.write(`warning: Bunode's JavaScript entry failed: ${getErrorMessage(error)}\n`)
  try {
    await access(binary)
  } catch {
    process.stderr.write('Bunode could not find an installed native CLI.\n')
    process.exitCode = 1
    process.exit()
  }

  process.stderr.write('warning: Using the previously installed native Bunode CLI.\n')
  const child = spawn(binary, process.argv.slice(2), { stdio: 'inherit' })
  child.on('error', childError => {
    process.stderr.write(`${getErrorMessage(childError)}\n`)
    process.exitCode = 1
  })
  child.on('exit', (code, signal) => {
    if (signal) {
      process.kill(process.pid, signal)
      return
    }
    process.exitCode = code ?? 1
  })
}

function getErrorMessage(error) {
  return error instanceof Error ? error.message : String(error)
}
