import { spawnSync } from 'node:child_process'

const source = `
const normalized = process.execPath.replaceAll('\\\\', '/')
console.log(\`argvLength=\${process.argv.length}\`)
console.log(\`hasDashArgv=\${process.argv.includes('-')}\`)
console.log(\`execPath=\${normalized.endsWith('/.dev/bin/node') || normalized.endsWith('/.dev/node.exe')}\`)
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
