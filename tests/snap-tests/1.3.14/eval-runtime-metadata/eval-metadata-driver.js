import { spawnSync } from 'node:child_process'

function run(args, options = {}) {
  return spawnSync(process.execPath, args, {
    encoding: 'utf8',
    env: process.env,
    ...options
  })
}

function writeResult(result) {
  process.stdout.write(result.stdout)
  process.stderr.write(result.stderr)
}

function isSyntheticFilename(value, name) {
  return [`/${name}`, `\\${name}`].some(suffix => value.endsWith(suffix))
}

const evalGlobals = run([
  '-e',
  `${isSyntheticFilename.toString()}; console.log(\`evalMeta=\${__filename}:\${__dirname}:\${module.id}:\${isSyntheticFilename(module.filename, "[eval]")}:\${require.main === module}:\${typeof exports}:\${typeof require}\`)`
])
writeResult(evalGlobals)

const evalRuntimeSyntax = run(['-e', 'console.log("evalRuntimeOnce"); JSON.parse("x")'])
process.stdout.write(`evalRuntimeStatus=${evalRuntimeSyntax.status}\n`)
process.stdout.write(
  `evalRuntimeOnce=${evalRuntimeSyntax.stdout.split('evalRuntimeOnce').length - 1}\n`
)

const evalHashbang = run([
  '-e',
  `#!/usr/bin/env node\n${isSyntheticFilename.toString()}\nvar evalHashbangGlobal = 1\nconsole.log(\`evalHashbangMeta=\${__filename}:\${__dirname}:\${globalThis.evalHashbangGlobal}:\${module.id}:\${isSyntheticFilename(module.filename, "[eval]")}:\${require.main === module}\`)`
])
writeResult(evalHashbang)
