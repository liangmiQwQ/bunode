const { createRequire } = require('node:module')
const { sep } = require('node:path')
const { pathToFileURL } = require('node:url')

const execPath = process.env.BUNODE_EXEC_PATH
const argv0 = process.env.BUNODE_ARGV0 ?? execPath
const execArgv = process.env.BUNODE_EXEC_ARGV
const argv = process.env.BUNODE_ARGV
const requires = process.env.BUNODE_REQUIRE

delete process.env.BUNODE_EXEC_PATH
delete process.env.BUNODE_ARGV0
delete process.env.BUNODE_EXEC_ARGV
delete process.env.BUNODE_ARGV
delete process.env.BUNODE_REQUIRE

function readJsonArray(value) {
  return value ? JSON.parse(value) : []
}

if (execPath) {
  Object.defineProperty(process, 'execPath', {
    value: execPath,
    writable: true,
    enumerable: true,
    configurable: true
  })
}

if (argv) {
  Object.defineProperty(process, 'argv', {
    value: readJsonArray(argv),
    writable: true,
    enumerable: true,
    configurable: true
  })
} else if (execPath) {
  process.argv[0] = execPath
}

if (argv0) {
  Object.defineProperty(process, 'argv0', {
    value: argv0,
    writable: false,
    enumerable: true,
    configurable: false
  })
}

if (execArgv) {
  Object.defineProperty(process, 'execArgv', {
    value: readJsonArray(execArgv),
    writable: true,
    enumerable: true,
    configurable: true
  })
}

const requirePreloads = readJsonArray(requires)

if (requirePreloads.length > 0) {
  const cwd = process.cwd()
  const base = cwd.endsWith(sep) ? cwd : cwd + sep
  const preloadRequire = createRequire(pathToFileURL(base))

  for (const specifier of requirePreloads) {
    preloadRequire(specifier)
  }
}
