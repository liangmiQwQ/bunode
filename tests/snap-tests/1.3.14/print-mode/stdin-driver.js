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

const dashResult = spawnSync(process.execPath, ['-p', '-', 'arg'], {
  input: 'JSON.stringify({ execArgv: process.execArgv, argv: process.argv.slice(1) })\n',
  encoding: 'utf8',
  env: process.env
})

process.stdout.write(dashResult.stdout)
process.stderr.write(dashResult.stderr)

if (dashResult.status !== 0) {
  throw new Error(`print dash stdin child exited with code ${dashResult.status ?? 1}`)
}

const requireResult = spawnSync(process.execPath, ['-p', '-'], {
  input: 'typeof require\n',
  encoding: 'utf8',
  env: process.env
})

process.stdout.write(requireResult.stdout)
process.stderr.write(requireResult.stderr)

if (requireResult.status !== 0) {
  throw new Error(`print require stdin child exited with code ${requireResult.status ?? 1}`)
}

const metadataResult = spawnSync(process.execPath, ['-p'], {
  input: 'JSON.stringify({ file: __filename, dir: __dirname })\n',
  encoding: 'utf8',
  env: process.env
})

process.stdout.write(metadataResult.stdout)
process.stderr.write(metadataResult.stderr)

if (metadataResult.status !== 0) {
  throw new Error(`print metadata stdin child exited with code ${metadataResult.status ?? 1}`)
}

const bindingResult = spawnSync(process.execPath, ['-p'], {
  input: 'var fs=1; var source=2; fs + source\n',
  encoding: 'utf8',
  env: process.env
})

process.stdout.write(bindingResult.stdout)
process.stderr.write(bindingResult.stderr)

if (bindingResult.status !== 0) {
  throw new Error(`print binding stdin child exited with code ${bindingResult.status ?? 1}`)
}

const largeProgram = `0;/*${'x'.repeat(500_000)}*/ 42\n`
const largeResult = spawnSync(process.execPath, ['-p', '-'], {
  input: largeProgram,
  encoding: 'utf8',
  env: process.env
})

process.stdout.write(largeResult.stdout)
process.stderr.write(largeResult.stderr)

if (largeResult.status !== 0) {
  throw new Error(`large print stdin child exited with code ${largeResult.status ?? 1}`)
}
