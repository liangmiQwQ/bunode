import { spawn } from 'node:child_process'
import { readdir, readFile } from 'node:fs/promises'
import { delimiter, resolve } from 'node:path'
import { platform } from 'node:process'

import { beforeAll, describe, expect, it } from 'vite-plus/test'

const projectRoot = resolve(import.meta.dirname, '..')
const snapTestsRoot = resolve(import.meta.dirname, 'snap-tests')
const snapConfigFileName = 'snap.json'
const snapFileName = 'snap.txt'
const snapColoredFileName = 'snap-colored.txt'
const devSetupTimeout = 300_000
const snapTestTimeout = 120_000
// Windows CI prebuilds outside Vite Task to avoid MSVC vctip handle leaks.
const skipCargoDevBuild = process.env.BUNODE_SKIP_DEV_BUILD === '1'
const ansiPattern = new RegExp(
  `${String.fromCodePoint(27)}(?:[@-Z\\\\-_]|\\[[0-?]*[ -/]*[@-~])`,
  'g'
)
const knownPlatforms = new Set(['darwin', 'linux', 'win32'])

type SnapCommand = string | SnapCommandConfig

interface SnapCommandConfig {
  command: string
  args: string[]
  cwd: string | undefined
  stdin: string | undefined
  stdinFile: string | undefined
  env: Record<string, string | null>
  exitCode: number
}

interface SnapConfig {
  description: string
  commands: SnapCommand[]
  ignore: string[]
  after: string[]
}

interface SnapTest {
  bunVersion: string
  name: string
  description: string
  directory: string
  config: SnapConfig
}

interface SnapTestGroup {
  bunVersion: string
  tests: SnapTest[]
}

const snapTests = await loadSnapTests()
const snapTestGroups = groupSnapTests(snapTests)

describe('snap tests', () => {
  describe.each(snapTestGroups)('$bunVersion', ({ bunVersion, tests }) => {
    const runnableTests = tests.filter(snapTest => !snapTest.config.ignore.includes(platform))

    beforeAll(async () => {
      await setupDevBuild(bunVersion)
    }, devSetupTimeout)

    it.each(runnableTests)(
      '$bunVersion/$name - $description',
      { timeout: snapTestTimeout, concurrent: false },
      async snapTest => {
        const plainOutput = stripAnsi(
          await runSnapshotPhase(snapTest.config, snapTest.directory, createSnapEnv('plain'))
        )
        await expect(plainOutput).toMatchFileSnapshot(resolve(snapTest.directory, snapFileName))

        const coloredOutput = await runSnapshotPhase(
          snapTest.config,
          snapTest.directory,
          createSnapEnv('colored')
        )
        await expect(coloredOutput).toMatchFileSnapshot(
          resolve(snapTest.directory, snapColoredFileName)
        )
      }
    )
  })
})

async function loadSnapTests() {
  const snapTests: SnapTest[] = []

  for (const bunVersion of await readDirectoryNames(snapTestsRoot)) {
    const bunVersionDirectory = resolve(snapTestsRoot, bunVersion)

    for (const name of await readDirectoryNames(bunVersionDirectory)) {
      const directory = resolve(bunVersionDirectory, name)
      await readRequiredFile(resolve(directory, snapFileName))
      await readRequiredFile(resolve(directory, snapColoredFileName))
      const config = await readSnapConfig(resolve(directory, snapConfigFileName))

      snapTests.push({
        bunVersion,
        name,
        description: config.description,
        directory,
        config
      })
    }
  }

  return snapTests
}

function groupSnapTests(snapTests: SnapTest[]): SnapTestGroup[] {
  const grouped = new Map<string, SnapTest[]>()

  for (const snapTest of snapTests) {
    grouped.set(snapTest.bunVersion, [...(grouped.get(snapTest.bunVersion) ?? []), snapTest])
  }

  return Array.from(grouped, ([bunVersion, tests]) => ({ bunVersion, tests }))
}

async function readDirectoryNames(directory: string) {
  const entries = await readdir(directory, { withFileTypes: true })

  return entries
    .filter(entry => entry.isDirectory())
    .map(entry => entry.name)
    .toSorted()
}

async function readRequiredFile(filePath: string) {
  await readFile(filePath, 'utf8')
}

async function readSnapConfig(filePath: string): Promise<SnapConfig> {
  const value = parseJson(await readFile(filePath, 'utf8'), filePath)
  assertObject(value, filePath)

  const config = {
    description: readString(value, 'description', filePath),
    commands: readRequiredSnapCommands(value, filePath),
    ignore: readStringArray(value, 'ignore', filePath),
    after: readStringArray(value, 'after', filePath)
  }

  for (const key of Object.keys(value)) {
    if (key !== 'description' && key !== 'commands' && key !== 'ignore' && key !== 'after') {
      throw new Error(`${filePath}: unknown field "${key}"`)
    }
  }

  for (const ignoredPlatform of config.ignore) {
    if (!knownPlatforms.has(ignoredPlatform)) {
      throw new Error(`${filePath}: unknown ignored OS "${ignoredPlatform}"`)
    }
  }

  return config
}

function parseJson(source: string, filePath: string) {
  try {
    return JSON.parse(source) as unknown
  } catch (error) {
    throw new Error(`${filePath}: invalid JSON`, { cause: error })
  }
}

function assertObject(value: unknown, filePath: string): asserts value is Record<string, unknown> {
  if (!value || typeof value !== 'object' || Array.isArray(value)) {
    throw new Error(`${filePath}: expected an object`)
  }
}

function readString(
  value: Record<string, unknown>,
  field: 'description' | 'command',
  filePath: string
) {
  const rawValue = value[field]

  if (typeof rawValue !== 'string' || !rawValue) {
    throw new Error(`${filePath}: "${field}" must be a non-empty string`)
  }

  return rawValue
}

function readStringArray(
  value: Record<string, unknown>,
  field: 'ignore' | 'after',
  filePath: string
) {
  const rawValue = value[field]

  if (rawValue === undefined) {
    return []
  }

  if (!Array.isArray(rawValue) || rawValue.some(item => typeof item !== 'string' || !item)) {
    throw new Error(`${filePath}: "${field}" must be an array of non-empty strings`)
  }

  return rawValue
}

function readRequiredSnapCommands(value: Record<string, unknown>, filePath: string) {
  const rawValue = value.commands

  if (!Array.isArray(rawValue) || rawValue.length === 0) {
    throw new Error(`${filePath}: "commands" must include at least one command`)
  }

  return rawValue.map((item, index) => readSnapCommand(item, `${filePath}: commands[${index}]`))
}

function readSnapCommand(value: unknown, location: string): SnapCommand {
  if (typeof value === 'string' && value) {
    return value
  }

  assertObject(value, location)

  const command = readString(value, 'command', location)
  const args = readOptionalStringArray(value, 'args', location)
  const cwd = readOptionalString(value, 'cwd', location)
  const stdin = readOptionalString(value, 'stdin', location)
  const stdinFile = readOptionalString(value, 'stdinFile', location)
  const env = readEnv(value, location)
  const exitCode = readExitCode(value, location)

  if (stdin !== undefined && stdinFile !== undefined) {
    throw new Error(`${location}: "stdin" and "stdinFile" cannot be used together`)
  }

  for (const key of Object.keys(value)) {
    if (
      key !== 'command' &&
      key !== 'args' &&
      key !== 'cwd' &&
      key !== 'stdin' &&
      key !== 'stdinFile' &&
      key !== 'env' &&
      key !== 'exitCode'
    ) {
      throw new Error(`${location}: unknown field "${key}"`)
    }
  }

  return { command, args, cwd, stdin, stdinFile, env, exitCode }
}

function readOptionalString(
  value: Record<string, unknown>,
  field: 'cwd' | 'stdin' | 'stdinFile',
  location: string
): string | undefined {
  const rawValue = value[field]

  if (rawValue === undefined) {
    return undefined
  }

  if (typeof rawValue !== 'string') {
    throw new TypeError(`${location}: "${field}" must be a string`)
  }

  return rawValue
}

function readOptionalStringArray(value: Record<string, unknown>, field: 'args', location: string) {
  const rawValue = value[field]

  if (rawValue === undefined) {
    return []
  }

  if (!Array.isArray(rawValue) || rawValue.some(item => typeof item !== 'string')) {
    throw new Error(`${location}: "${field}" must be an array of strings`)
  }

  return rawValue
}

function readEnv(value: Record<string, unknown>, location: string) {
  const rawValue = value.env

  if (rawValue === undefined) {
    return {}
  }

  assertObject(rawValue, `${location}: env`)

  const env: Record<string, string | null> = {}

  for (const [key, item] of Object.entries(rawValue)) {
    if (typeof item !== 'string' && item !== null) {
      throw new Error(`${location}: env.${key} must be a string or null`)
    }

    env[key] = item
  }

  return env
}

function readExitCode(value: Record<string, unknown>, location: string) {
  const rawValue = value.exitCode

  if (rawValue === undefined) {
    return 0
  }

  if (
    typeof rawValue !== 'number' ||
    !Number.isInteger(rawValue) ||
    rawValue < 0 ||
    rawValue > 255
  ) {
    throw new Error(`${location}: "exitCode" must be an integer from 0 to 255`)
  }

  return rawValue
}

async function setupDevBuild(bunVersion: string) {
  if (!skipCargoDevBuild) {
    await runProcess('cargo', ['build', '-p', 'bunode'], projectRoot, process.env)
  }

  await runProcess(process.execPath, ['scripts/setup-dev.ts', bunVersion], projectRoot, process.env)
}

async function runSnapshotPhase(
  config: SnapConfig,
  cwd: string,
  env: NodeJS.ProcessEnv,
  signal?: AbortSignal
) {
  let output = ''
  let commandError: unknown
  let cleanupError: unknown

  try {
    output = await runCommands(config.commands, cwd, env, signal)
  } catch (error) {
    commandError = error
  }

  try {
    await runCommands(config.after, cwd, env, signal)
  } catch (error) {
    cleanupError = error
  }

  if (commandError || cleanupError) {
    throw new Error(formatPhaseError(commandError, cleanupError))
  }

  return output
}

async function runCommands(
  commands: SnapCommand[],
  cwd: string,
  env: NodeJS.ProcessEnv,
  signal?: AbortSignal
) {
  let output = ''

  for (const command of commands) {
    output += await runSnapCommand(command, cwd, env, signal)
  }

  return output
}

async function runSnapCommand(
  command: SnapCommand,
  cwd: string,
  env: NodeJS.ProcessEnv,
  signal?: AbortSignal
) {
  if (typeof command === 'string') {
    return runCommandProcess(command, [], 0, cwd, env, undefined, true, signal)
  }

  const commandEnv = createCommandEnv(env, command.env)
  const commandCwd = command.cwd === undefined ? cwd : resolve(cwd, command.cwd)
  const stdin = await readCommandStdin(command, commandCwd)

  return runCommandProcess(
    command.command,
    command.args,
    command.exitCode,
    commandCwd,
    commandEnv,
    stdin,
    false,
    signal
  )
}

function createCommandEnv(env: NodeJS.ProcessEnv, overrides: Record<string, string | null>) {
  const commandEnv: NodeJS.ProcessEnv = { ...env }

  for (const [key, value] of Object.entries(overrides)) {
    if (value === null) {
      delete commandEnv[key]
    } else {
      commandEnv[key] = value
    }
  }

  return commandEnv
}

async function readCommandStdin(
  command: SnapCommandConfig,
  cwd: string
): Promise<string | undefined> {
  if (command.stdin !== undefined) {
    return command.stdin
  }

  if (command.stdinFile !== undefined) {
    return readFile(resolve(cwd, command.stdinFile), 'utf8')
  }

  return undefined
}

function runCommandProcess(
  command: string,
  args: string[],
  exitCode: number,
  cwd: string,
  env: NodeJS.ProcessEnv,
  stdin: string | undefined,
  shell: boolean,
  signal?: AbortSignal
) {
  return new Promise<string>((resolve, reject) => {
    const child = spawn(command, args, {
      cwd,
      env,
      shell,
      signal,
      stdio: [stdin === undefined ? 'ignore' : 'pipe', 'pipe', 'pipe']
    })
    let output = ''
    let settled = false
    const { stdin: childStdin, stdout, stderr } = child

    if (!stdout || !stderr) {
      reject(new Error(`${formatCommand(command, args)} did not expose stdout and stderr pipes`))
      return
    }

    stdout.setEncoding('utf8')
    stderr.setEncoding('utf8')
    stdout.on('data', chunk => {
      output += chunk
    })
    stderr.on('data', chunk => {
      output += chunk
    })

    if (stdin !== undefined) {
      if (!childStdin) {
        reject(new Error(`${formatCommand(command, args)} did not expose a stdin pipe`))
        return
      }

      childStdin.end(stdin)
    }

    child.on('error', error => {
      if (!settled) {
        settled = true
        reject(error)
      }
    })

    child.on('close', code => {
      if (settled) {
        return
      }

      settled = true

      if ((code ?? 1) === exitCode) {
        resolve(output)
        return
      }

      reject(
        new Error(
          `${formatCommand(command, args)} exited with code ${code ?? 1}, expected ${exitCode} in ${cwd}\n${output}`
        )
      )
    })
  })
}

function formatCommand(command: string, args: string[]) {
  return args.length === 0 ? command : `${command} ${args.join(' ')}`
}

function runProcess(
  command: string,
  args: string[],
  cwd: string,
  env: NodeJS.ProcessEnv,
  signal?: AbortSignal
) {
  return new Promise<string>((resolve, reject) => {
    const child = spawn(command, args, {
      cwd,
      env,
      signal,
      stdio: ['ignore', 'pipe', 'pipe']
    })
    let output = ''
    let settled = false

    child.stdout.setEncoding('utf8')
    child.stderr.setEncoding('utf8')
    child.stdout.on('data', chunk => {
      output += chunk
    })
    child.stderr.on('data', chunk => {
      output += chunk
    })

    child.on('error', error => {
      if (!settled) {
        settled = true
        reject(error)
      }
    })

    child.on('close', code => {
      if (settled) {
        return
      }

      settled = true

      if (code === 0) {
        resolve(output)
        return
      }

      reject(
        new Error(`${command} ${args.join(' ')} exited with code ${code ?? 1} in ${cwd}\n${output}`)
      )
    })
  })
}

function createSnapEnv(mode: 'plain' | 'colored') {
  const env: NodeJS.ProcessEnv = {
    ...process.env,
    PATH: `${getDevPathDirectory()}${delimiter}${process.env.PATH ?? ''}`
  }

  if (mode === 'plain') {
    env.NO_COLOR = '1'
    env.FORCE_COLOR = '0'
    env.CLICOLOR = '0'
    delete env.CLICOLOR_FORCE
    return env
  }

  delete env.NO_COLOR
  env.FORCE_COLOR = '1'
  env.CLICOLOR_FORCE = '1'
  env.TERM ??= 'xterm-256color'

  return env
}

function getDevPathDirectory() {
  return platform === 'win32' ? resolve(projectRoot, '.dev') : resolve(projectRoot, '.dev/bin')
}

function stripAnsi(value: string) {
  return value.replaceAll(ansiPattern, '')
}

function formatPhaseError(commandError: unknown, cleanupError: unknown) {
  if (commandError && cleanupError) {
    return `${formatError(commandError)}\nCleanup failed:\n${formatError(cleanupError)}`
  }

  return formatError(commandError ?? cleanupError)
}

function formatError(error: unknown) {
  return error instanceof Error ? error.message : String(error)
}
