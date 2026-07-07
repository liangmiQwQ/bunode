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
