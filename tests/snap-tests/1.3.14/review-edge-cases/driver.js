import { spawnSync } from 'node:child_process'
import { mkdirSync, mkdtempSync, readdirSync, rmSync, writeFileSync } from 'node:fs'
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

const printPromise = run(['-p', 'Promise.resolve(1)'])
const printPromiseOutput = printPromise.stdout.trim()
process.stdout.write(`printPromiseStatus=${printPromise.status}\n`)
process.stdout.write(
  `printPromiseLooksNode=${printPromiseOutput.includes('Promise') && printPromiseOutput !== '1'}\n`
)

const printGlobals = run([
  '-p',
  `${isSyntheticFilename.toString()}; \`\${module.id}:\${isSyntheticFilename(module.filename, "[eval]")}:\${require.main === module}\``
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
  input: `#!/usr/bin/env node\n${isSyntheticFilename.toString()}\nvar stdinGlobal = 1\nconsole.log(\`hashbangMeta=\${__filename}:\${__dirname}:\${globalThis.stdinGlobal}:\${module.id}:\${isSyntheticFilename(module.filename, "[stdin]")}:\${require.main === module}\`)\n`
})
writeResult(hashbangStdin)

writeFileSync('mixed.env', 'BAD="unterminated\nNODE_OPTIONS="--conditions mixed"\n')
const mixedEnv = run(['--env-file', 'mixed.env', '-e', 'console.log("mixedEnvOk")'])
writeResult(mixedEnv)

writeFileSync('multi-preload.cjs', 'console.log("multiPreload")\n')
writeFileSync('multi.env', 'NODE_OPTIONS="--require ./multi-preload.cjs\n--conditions custom"\n')
const multiEnv = run(['--env-file', 'multi.env', '-e', 'console.log("multiMain")'])
writeResult(multiEnv)

writeFileSync('escaped-preload.cjs', 'console.log("escapedPreload")\n')
writeFileSync(
  'escaped.env',
  'NODE_OPTIONS="--require ./escaped-preload.cjs\\n--conditions custom"\n'
)
const escapedEnv = run(['--env-file', 'escaped.env', '-e', 'console.log("escapedMain")'])
writeResult(escapedEnv)

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

const requireTempDir = mkdtempSync(join(tmpdir(), 'bunode-require-temp-'))
try {
  const samePreloadSource = 'globalThis.samePreloadCount = (globalThis.samePreloadCount ?? 0) + 1\n'

  writeFileSync('same-preload-a.cjs', samePreloadSource)
  writeFileSync('same-preload-b.cjs', samePreloadSource)

  const noRequireWrapperFiles = run(
    [
      '--require',
      './same-preload-a.cjs',
      '--require',
      './same-preload-b.cjs',
      '-e',
      'console.log(`samePreloadCount=${globalThis.samePreloadCount}`)'
    ],
    {
      env: {
        ...process.env,
        TMPDIR: requireTempDir,
        TMP: requireTempDir,
        TEMP: requireTempDir
      }
    }
  )

  writeResult(noRequireWrapperFiles)

  const requireWrapperFiles = readdirSync(requireTempDir).filter(fileName =>
    fileName.startsWith('bunode-require-preload-')
  )

  process.stdout.write(`requireWrapperFiles=${requireWrapperFiles.length}\n`)
} finally {
  rmSync(requireTempDir, { recursive: true, force: true })
}

const streamWrap = run(['--require', '_stream_wrap', '-e', 'console.log("streamWrapOk")'])
writeResult(streamWrap)

const dataImport = run(['--import', 'data:text/javascript,globalThis.dataLoaded=1', '-e', '0'])
process.stdout.write(`dataImportStatus=${dataImport.status}\n`)
process.stdout.write(
  `dataImportUnsupported=${dataImport.stderr.includes('data URL imports passed to --import are not supported')}\n`
)
