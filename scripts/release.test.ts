import { spawn } from 'node:child_process'
import { chmod, copyFile, link, mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { delimiter, join } from 'node:path'
import { platform } from 'node:process'

import { expect, it } from 'vite-plus/test'

const projectRoot = join(import.meta.dirname, '..')
const isWindows = platform === 'win32'

it('creates a release pull request with the caller git and gh commands', async () => {
  const root = await mkdtemp(join(tmpdir(), 'bunode-release-pr-'))
  const binDirectory = join(root, 'bin')
  const commandLog = join(root, 'commands.jsonl')
  const fakeCommandsModule = join(root, 'fake-commands.cjs')

  try {
    await mkdir(join(root, 'packages/cli'), { recursive: true })
    await mkdir(join(root, 'scripts'))
    await mkdir(binDirectory)
    await writeFile(join(root, 'Cargo.toml'), 'version = "0.0.0-alpha.0"\n')
    await writeFile(join(root, 'Cargo.lock'), 'version = "0.0.0-alpha.0"\n')
    await writeFile(
      join(root, 'packages/cli/package.json'),
      '{"name":"@bunode/cli","version":"0.0.0-alpha.0"}\n'
    )
    await writeFile(join(root, 'scripts/prepare-release.ts'), fakePrepareReleaseSource)
    await writeFile(fakeCommandsModule, fakeCommandsSource)
    await Promise.all(['gh', 'git', 'vpr'].map(command => copyExecutable(binDirectory, command)))

    const output = await runReleaseScript(root, binDirectory, commandLog, fakeCommandsModule)

    expect(output).toContain('https://github.com/liangmiQwQ/bunode/pull/7')
    const manifest = JSON.parse(await readFile(join(root, 'packages/cli/package.json'), 'utf8'))
    expect(manifest.version).toBe('0.0.0-alpha.1')

    const calls = (await readFile(commandLog, 'utf8'))
      .trim()
      .split('\n')
      .map(line => JSON.parse(line) as string[])
    expect(calls).toContainEqual([
      'git',
      'push',
      '--set-upstream',
      'origin',
      'release/v0.0.0-alpha.1'
    ])
    const createCall = calls.find(
      ([command, operation, subcommand]) =>
        command === 'gh' && operation === 'pr' && subcommand === 'create'
    )
    expect(createCall).toContain('chore(release): v0.0.0-alpha.1')
    expect(createCall).toContain("## What's Changed\n* feat: release entry")
  } finally {
    await rm(root, { force: true, recursive: true })
  }
})

async function copyExecutable(directory: string, command: string): Promise<void> {
  const path = join(directory, isWindows ? `${command}.exe` : command)

  try {
    await link(process.execPath, path)
  } catch (error) {
    const { code } = error as NodeJS.ErrnoException
    if (code !== 'EXDEV' && code !== 'EPERM' && code !== 'EACCES') {
      throw error
    }
    await copyFile(process.execPath, path)
  }
  if (!isWindows) {
    await chmod(path, 0o755)
  }
}

function runReleaseScript(
  cwd: string,
  binDirectory: string,
  commandLog: string,
  fakeCommandsModule: string
): Promise<string> {
  return new Promise((resolve, reject) => {
    const child = spawn(
      process.execPath,
      [join(projectRoot, 'scripts/release.ts'), '0.0.0-alpha.1'],
      {
        cwd,
        env: {
          ...process.env,
          BUNODE_RELEASE_TEST_LOG: commandLog,
          NODE_OPTIONS: `--require=${fakeCommandsModule}`,
          PATH: `${binDirectory}${delimiter}${process.env.PATH ?? ''}`
        }
      }
    )
    let output = ''

    child.stdout.setEncoding('utf8')
    child.stderr.setEncoding('utf8')
    child.stdout.on('data', chunk => {
      output += chunk
    })
    child.stderr.on('data', chunk => {
      output += chunk
    })
    child.on('error', reject)
    child.on('close', code => {
      if (code === 0) {
        resolve(output)
      } else {
        reject(new Error(`release exited with ${code ?? 1}: ${output}`))
      }
    })
  })
}

const fakePrepareReleaseSource = String.raw`
import { readFileSync, writeFileSync } from 'node:fs'

const version = process.argv[2]
for (const path of ['Cargo.toml', 'Cargo.lock', 'packages/cli/package.json']) {
  writeFileSync(path, readFileSync(path, 'utf8').replace(/0\.0\.0-alpha\.0/g, version))
}
`

const fakeCommandsSource = String.raw`
const { appendFileSync } = require('node:fs')
const { basename } = require('node:path')

const command = basename(process.execPath).replace(/\.exe$/, '')
if (['gh', 'git', 'vpr'].includes(command)) {
  const args = process.argv.slice(1)
  args[0] = basename(args[0])
  appendFileSync(process.env.BUNODE_RELEASE_TEST_LOG, JSON.stringify([command, ...args]) + '\n')

  if (command === 'git' && args[0] === 'branch') {
    process.stdout.write('main\n')
  } else if (command === 'git' && args[0] === 'rev-parse') {
    process.stdout.write('0123456789abcdef\n')
  } else if (command === 'gh' && args[0] === 'api') {
    process.stdout.write("## What's Changed\n* feat: release entry\n")
  } else if (command === 'gh' && args[0] === 'pr') {
    process.stdout.write('https://github.com/liangmiQwQ/bunode/pull/7\n')
  }

  process.exit()
}
`
