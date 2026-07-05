const execPath = process.env.BUNODE_EXEC_PATH
const argv0 = process.env.BUNODE_ARGV0 ?? execPath
const dropStdinArgv = process.env.BUNODE_DROP_STDIN_ARGV === '1'

delete process.env.BUNODE_EXEC_PATH
delete process.env.BUNODE_ARGV0
delete process.env.BUNODE_DROP_STDIN_ARGV

if (execPath) {
  Object.defineProperty(process, 'execPath', { value: execPath, configurable: true })
  process.argv[0] = execPath
}

if (argv0) {
  Object.defineProperty(process, 'argv0', { value: argv0, configurable: true })
}

if (dropStdinArgv && process.argv[1] === '-') {
  process.argv.splice(1, 1)
}
