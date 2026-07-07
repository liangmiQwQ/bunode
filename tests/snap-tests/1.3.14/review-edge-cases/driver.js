import { spawnSync } from 'node:child_process'
import { mkdirSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

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

const evalGlobals = run([
  '-e',
  'console.log(`evalMeta=${__filename}:${__dirname}:${module.id}:${module.filename.endsWith("/[eval]")}:${require.main === module}:${typeof exports}:${typeof require}`)'
])
writeResult(evalGlobals)

const evalRuntimeSyntax = run(['-e', 'console.log("evalRuntimeOnce"); JSON.parse("x")'])
process.stdout.write(`evalRuntimeStatus=${evalRuntimeSyntax.status}\n`)
process.stdout.write(
  `evalRuntimeOnce=${evalRuntimeSyntax.stdout.split('evalRuntimeOnce').length - 1}\n`
)

const printPromise = run(['-p', 'Promise.resolve(1)'])
const printPromiseOutput = printPromise.stdout.trim()
process.stdout.write(`printPromiseStatus=${printPromise.status}\n`)
process.stdout.write(
  `printPromiseLooksNode=${printPromiseOutput.includes('Promise') && printPromiseOutput !== '1'}\n`
)

const printGlobals = run([
  '-p',
  '`${module.id}:${module.filename.endsWith("/[eval]")}:${require.main === module}`'
])
writeResult(printGlobals)

const printModule = run(['-p'], {
  input: 'import { readFileSync } from "node:fs"; 1\n'
})
const printModuleRejected = [
  'ERR_EVAL_ESM_CANNOT_PRINT',
  '--print cannot be used with ESM input'
].some(value => printModule.stderr.includes(value))

process.stdout.write(`printModuleStatus=${printModule.status}\n`)
process.stdout.write(`printModuleRejected=${printModuleRejected}\n`)

const printSyntax = run(['-p', 'if ('])
process.stdout.write(`printSyntaxStatus=${printSyntax.status}\n`)
process.stdout.write(
  `printSyntaxIsSyntax=${printSyntax.stderr.includes('SyntaxError') && !printSyntax.stderr.includes('ERR_EVAL_ESM_CANNOT_PRINT')}\n`
)

const hashbangStdin = run(['-'], {
  input:
    '#!/usr/bin/env node\nvar stdinGlobal = 1\nconsole.log(`hashbangMeta=${__filename}:${__dirname}:${globalThis.stdinGlobal}:${module.id}:${module.filename.endsWith("/[stdin]")}:${require.main === module}`)\n'
})
writeResult(hashbangStdin)

writeFileSync('mixed.env', 'BAD="unterminated\nNODE_OPTIONS="--conditions mixed"\n')
const mixedEnv = run(['--env-file', 'mixed.env', '-e', 'console.log("mixedEnvOk")'])
writeResult(mixedEnv)

writeFileSync('multi-preload.cjs', 'console.log("multiPreload")\n')
writeFileSync('multi.env', 'NODE_OPTIONS="--require ./multi-preload.cjs\n--conditions custom"\n')
const multiEnv = run(['--env-file', 'multi.env', '-e', 'console.log("multiMain")'])
writeResult(multiEnv)

mkdirSync('node_modules/conditional-preload', { recursive: true })
writeFileSync(
  'node_modules/conditional-preload/package.json',
  JSON.stringify({
    name: 'conditional-preload',
    exports: {
      '.': {
        import: './import.mjs',
        require: './require.cjs'
      }
    }
  })
)
writeFileSync('node_modules/conditional-preload/import.mjs', 'console.log("conditionalImport")\n')
writeFileSync('node_modules/conditional-preload/require.cjs', 'console.log("conditionalRequire")\n')
const conditionalPreload = run([
  '--require',
  'conditional-preload',
  '-e',
  'console.log("conditionalMain")'
])
writeResult(conditionalPreload)

const streamWrap = run(['--require', '_stream_wrap', '-e', 'console.log("streamWrapOk")'])
writeResult(streamWrap)

const temporaryRelative = join(tmpdir(), 'x.js')
writeFileSync(temporaryRelative, 'globalThis.relativeLoaded = 1\n')
const dataSource = 'import "./x.js"; globalThis.relativeLoaded = 2'
const dataRelative = run([
  '--import',
  `data:text/javascript,${encodeURIComponent(dataSource)}`,
  '-e',
  'console.log("dataMain")'
])
process.stdout.write(`dataRelativeStatus=${dataRelative.status}\n`)
process.stdout.write(`dataRelativeRejected=${dataRelative.stderr.includes('blob:')}\n`)
rmSync(temporaryRelative, { force: true })
