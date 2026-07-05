import { spawn } from 'node:child_process'
import { readdir, readFile } from 'node:fs/promises'
import { delimiter, resolve } from 'node:path'
import { platform } from 'node:process'

import { beforeAll, describe, expect, it } from 'vite-plus/test'

const projectRoot = resolve(import.meta.dirname, '..')
const snapTestsRoot = resolve(import.meta.dirname, 'snap-tests')
const stepsFileName = 'steps.json'
const snapFileName = 'snap.txt'
const snapColoredFileName = 'snap-colored.txt'
const devSetupTimeout = 300_000
const snapTestTimeout = 120_000
const ansiPattern = new RegExp(
  `${String.fromCodePoint(27)}(?:[@-Z\\\\-_]|\\[[0-?]*[ -/]*[@-~])`,
  'g'
)
const knownPlatforms = new Set(['darwin', 'linux', 'win32'])

interface Steps {
  commands: string[]
  ignore: string[]
  after: string[]
}

interface SnapTest {
  bunVersion: string
  name: string
  directory: string
  steps: Steps
}

interface SnapTestGroup {
  bunVersion: string
  tests: SnapTest[]
}

const snapTests = await loadSnapTests()
const snapTestGroups = groupSnapTests(snapTests)

describe('snap tests', () => {
  describe.each(snapTestGroups)('$bunVersion', ({ bunVersion, tests }) => {
    const runnableTests = tests.filter(snapTest => !snapTest.steps.ignore.includes(platform))

    beforeAll(async () => {
      await setupDevBuild(bunVersion)
    }, devSetupTimeout)

    it.each(runnableTests)(
      '$bunVersion/$name',
      { timeout: snapTestTimeout, concurrent: false },
      async snapTest => {
        const plainOutput = stripAnsi(
          await runSnapshotPhase(snapTest.steps, snapTest.directory, createSnapEnv('plain'))
        )
        await expect(plainOutput).toMatchFileSnapshot(resolve(snapTest.directory, snapFileName))

        const coloredOutput = await runSnapshotPhase(
          snapTest.steps,
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

      snapTests.push({
        bunVersion,
        name,
        directory,
        steps: await readSteps(resolve(directory, stepsFileName))
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

async function readSteps(filePath: string): Promise<Steps> {
  const value = parseJson(await readFile(filePath, 'utf8'), filePath)
  assertObject(value, filePath)

  const steps = {
    commands: readStringArray(value, 'commands', filePath),
    ignore: readStringArray(value, 'ignore', filePath),
    after: readStringArray(value, 'after', filePath)
  }

  for (const key of Object.keys(value)) {
    if (key !== 'commands' && key !== 'ignore' && key !== 'after') {
      throw new Error(`${filePath}: unknown field "${key}"`)
    }
  }

  for (const ignoredPlatform of steps.ignore) {
    if (!knownPlatforms.has(ignoredPlatform)) {
      throw new Error(`${filePath}: unknown ignored OS "${ignoredPlatform}"`)
    }
  }

  return steps
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

function readStringArray(
  value: Record<string, unknown>,
  field: 'commands' | 'ignore' | 'after',
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

async function setupDevBuild(bunVersion: string) {
  await runShellCommand(`vpr dev ${quoteShellArgument(bunVersion)}`, projectRoot, process.env)
}

async function runSnapshotPhase(
  steps: Steps,
  cwd: string,
  env: NodeJS.ProcessEnv,
  signal?: AbortSignal
) {
  let output = ''
  let commandError: unknown
  let cleanupError: unknown

  try {
    output = await runCommands(steps.commands, cwd, env, signal)
  } catch (error) {
    commandError = error
  }

  try {
    await runCommands(steps.after, cwd, env, signal)
  } catch (error) {
    cleanupError = error
  }

  if (commandError || cleanupError) {
    throw new Error(formatPhaseError(commandError, cleanupError))
  }

  return output
}

async function runCommands(
  commands: string[],
  cwd: string,
  env: NodeJS.ProcessEnv,
  signal?: AbortSignal
) {
  let output = ''

  for (const command of commands) {
    output += await runShellCommand(command, cwd, env, signal)
  }

  return output
}

function runShellCommand(
  command: string,
  cwd: string,
  env: NodeJS.ProcessEnv,
  signal?: AbortSignal
) {
  return new Promise<string>((resolve, reject) => {
    const child = spawn(command, {
      cwd,
      env,
      shell: true,
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

      reject(new Error(`${command} exited with code ${code ?? 1} in ${cwd}\n${output}`))
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

function quoteShellArgument(value: string) {
  if (platform === 'win32') {
    return `"${value.replaceAll('"', String.raw`\"`)}"`
  }

  return `'${value.replaceAll("'", String.raw`'\''`)}'`
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
