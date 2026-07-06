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

function descriptorText(value) {
  const descriptor = Object.getOwnPropertyDescriptor(process, value)

  return [
    descriptor.writable === true ? 'w' : 'r',
    descriptor.enumerable === true ? 'e' : 'h',
    descriptor.configurable === true ? 'c' : 'f'
  ].join('')
}

process.stdout.write(
  [
    `argv0=${executableName(process.argv0)}`,
    `argv=${process.argv.slice(1).map(executableName).join('|')}`,
    `execPath=${isBunodeExecPath(process.execPath)}`,
    `execPathWritable=${canWriteExecPath()}`,
    `argv0Descriptor=${descriptorText('argv0')}`,
    `internalEnv=${process.env.BUNODE_EXEC_PATH === undefined}`
  ].join('\n')
)
process.stdout.write('\n')
