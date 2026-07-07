import { spawnSync } from 'node:child_process'

const source = `
var stdinGlobal = 1
const normalized = process.execPath.replaceAll('\\\\', '/')
console.log(\`argvLength=\${process.argv.length}\`)
console.log(\`hasDashArgv=\${process.argv.includes('-')}\`)
console.log(\`execPath=\${normalized.endsWith('/.dev/bin/node') || normalized.endsWith('/.dev/node.exe')}\`)
console.log(\`argvTail=\${process.argv.slice(1).map(value => value === '' ? '<empty>' : value).join('|')}\`)
console.log(\`globalVar=\${globalThis.stdinGlobal}\`)
`

const result = spawnSync(process.execPath, {
  input: source,
  encoding: 'utf8',
  env: process.env
})

process.stdout.write(result.stdout)
process.stderr.write(result.stderr)

if (result.status !== 0) {
  throw new Error(`stdin child exited with code ${result.status ?? 1}`)
}

const emptyOperandResult = spawnSync(process.execPath, ['', 'arg'], {
  input: source,
  encoding: 'utf8',
  env: process.env
})

process.stdout.write(emptyOperandResult.stdout)
process.stderr.write(emptyOperandResult.stderr)

if (emptyOperandResult.status !== 0) {
  throw new Error(`empty operand stdin child exited with code ${emptyOperandResult.status ?? 1}`)
}

const moduleSource = `
import { readFileSync } from 'node:fs'
const value = await Promise.resolve('ready')
console.log(\`esmStdin=\${typeof readFileSync}:\${value}\`)
`

const moduleResult = spawnSync(process.execPath, ['-'], {
  input: moduleSource,
  encoding: 'utf8',
  env: process.env
})

process.stdout.write(moduleResult.stdout)
process.stderr.write(moduleResult.stderr)

if (moduleResult.status !== 0) {
  throw new Error(`module stdin child exited with code ${moduleResult.status ?? 1}`)
}
