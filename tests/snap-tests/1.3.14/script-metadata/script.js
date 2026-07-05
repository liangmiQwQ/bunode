import { basename } from 'node:path'

function executableName(value) {
  return basename(value).replace(/\.exe$/, '')
}

function isBunodeExecPath(value) {
  const normalized = value.replaceAll('\\', '/')

  return ['/.dev/bin/node', '/.dev/node.exe'].some(suffix => normalized.endsWith(suffix))
}

function canWriteExecPath() {
  const originalExecPath = process.execPath

  try {
    process.execPath = 'patched-exec-path'
    return process.execPath === 'patched-exec-path'
  } finally {
    process.execPath = originalExecPath
  }
}

process.stdout.write(
  [
    `argv0=${executableName(process.argv0)}`,
    `argv=${process.argv.slice(1).map(executableName).join('|')}`,
    `execPath=${isBunodeExecPath(process.execPath)}`,
    `execPathWritable=${canWriteExecPath()}`,
    `internalEnv=${process.env.BUNODE_EXEC_PATH === undefined}`
  ].join('\n')
)
process.stdout.write('\n')
