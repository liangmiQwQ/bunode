import { basename } from 'node:path'

function executableName(value) {
  return basename(value).replace(/\.exe$/, '')
}

function isBunodeExecPath(value) {
  const normalized = value.replaceAll('\\', '/')

  return ['/.dev/bin/node', '/.dev/node.exe'].some(suffix => normalized.endsWith(suffix))
}

process.stdout.write(
  [
    `argv0=${executableName(process.argv0)}`,
    `argv=${process.argv.slice(1).map(executableName).join('|')}`,
    `execPath=${isBunodeExecPath(process.execPath)}`,
    `internalEnv=${process.env.BUNODE_EXEC_PATH === undefined}`
  ].join('\n')
)
process.stdout.write('\n')
