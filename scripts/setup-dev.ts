import { spawn } from 'node:child_process'
import { chmod, copyFile, mkdir, mkdtemp, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { dirname, join, resolve } from 'node:path'
import { arch, platform } from 'node:process'
import { styleText } from 'node:util'

const projectRoot = resolve(import.meta.dirname, '..')
const [rawBunVersion] = process.argv.slice(2)

if (!rawBunVersion) {
  process.stderr.write(styleText(['red', 'bold'], '✗ Usage: vpr dev <bun-version>\n'))
  process.exit(1)
}

const isWindows = platform === 'win32'
const executableMode = 0o755
const nodePath = isWindows
  ? resolve(projectRoot, '.dev/node.exe')
  : resolve(projectRoot, '.dev/bin/node')
const bunPath = resolve(projectRoot, '.dev/bun', isWindows ? 'bun.exe' : 'bun')

await copyExecutable(
  resolve(projectRoot, 'target/debug', isWindows ? 'node.exe' : 'node'),
  nodePath
)
await installBun(normalizeBunVersion(rawBunVersion), getBunVersionOutput(rawBunVersion), bunPath)
printUsage(nodePath)

async function installBun(bunVersion: string, expectedVersion: string, destination: string) {
  const installedVersion = await readOutput(destination, ['--version'])

  if (installedVersion === expectedVersion) {
    return
  }

  const assetName = getBunAssetName()
  const tempRoot = await mkdtemp(join(tmpdir(), 'bunode-'))
  const zipPath = resolve(tempRoot, `${assetName}.zip`)

  try {
    await downloadFile(
      `https://github.com/oven-sh/bun/releases/download/${bunVersion}/${assetName}.zip`,
      zipPath
    )
    await run('unzip', ['-q', zipPath, '-d', dirname(zipPath)])
    await copyExecutable(
      resolve(dirname(zipPath), assetName, isWindows ? 'bun.exe' : 'bun'),
      destination
    )
  } finally {
    await rm(tempRoot, { force: true, recursive: true })
  }
}

function readOutput(command: string, args: string[]) {
  return new Promise<string | null>(resolve => {
    const child = spawn(command, args, { stdio: ['ignore', 'pipe', 'ignore'] })
    let output = ''
    let settled = false

    child.stdout.setEncoding('utf8')
    child.stdout.on('data', chunk => {
      output += chunk
    })

    child.on('error', () => {
      if (!settled) {
        settled = true
        resolve(null)
      }
    })

    child.on('close', code => {
      if (!settled) {
        settled = true
        resolve(code === 0 ? output.trim() : null)
      }
    })
  })
}

async function downloadFile(url: string, destination: string) {
  const response = await fetch(url)

  if (!response.ok) {
    throw new Error(`Failed to download ${url}: ${response.status} ${response.statusText}`)
  }

  await writeFile(destination, Buffer.from(await response.arrayBuffer()))
}

async function copyExecutable(source: string, destination: string) {
  await mkdir(dirname(destination), { recursive: true })
  await rm(destination, { force: true })
  await copyFile(source, destination)

  if (!isWindows) {
    await chmod(destination, executableMode)
  }
}

function run(command: string, args: string[]) {
  return new Promise<void>((resolve, reject) => {
    const child = spawn(command, args, { stdio: 'inherit' })

    child.on('error', reject)
    child.on('close', code => {
      if (code === 0) {
        resolve()
        return
      }

      reject(new Error(`${command} exited with code ${code ?? 1}`))
    })
  })
}

function normalizeBunVersion(value: string) {
  const version = value.trim()

  if (version.startsWith('bun-v')) {
    return version
  }

  if (version.startsWith('v')) {
    return `bun-${version}`
  }

  return `bun-v${version}`
}

function getBunVersionOutput(value: string) {
  const version = value.trim()

  if (version.startsWith('bun-v')) {
    return version.slice(5)
  }

  if (version.startsWith('v')) {
    return version.slice(1)
  }

  return version
}

function getBunAssetName() {
  if (platform === 'darwin' && arch === 'arm64') {
    return 'bun-darwin-aarch64'
  }

  if (platform === 'darwin' && arch === 'x64') {
    return 'bun-darwin-x64'
  }

  if (platform === 'linux' && arch === 'arm64') {
    return 'bun-linux-aarch64'
  }

  if (platform === 'linux' && arch === 'x64') {
    return 'bun-linux-x64'
  }

  if (platform === 'win32' && arch === 'x64') {
    return 'bun-windows-x64'
  }

  throw new Error(`Unsupported platform for Bun download: ${platform} ${arch}`)
}

function printUsage(nodeExecutablePath: string) {
  const binDirectory = dirname(nodeExecutablePath)
  const pathCommand = getPathCommand(binDirectory)

  process.stdout.write('\n======== DEV BUILD SUCCESS ========')

  process.stdout.write(
    `\n\nRun: \n${styleText(['greenBright', 'bold'], nodeExecutablePath)} \n\nOr add to PATH: \n${styleText(['greenBright', 'bold'], pathCommand)}\n`
  )

  process.stdout.write('\n===================================')
}

function getPathCommand(binDirectory: string) {
  const shell = process.env.SHELL ?? process.env.ComSpec ?? ''
  const shellName = shell.toLowerCase()

  if (isWindows && shellName.includes('powershell')) {
    return `$env:Path = ${quotePowerShell(`${binDirectory};`)} + $env:Path`
  }

  if (isWindows && shellName.includes('cmd')) {
    return `set "PATH=${binDirectory};%PATH%"`
  }

  if (shellName.endsWith('/fish') || shellName.endsWith(String.raw`\fish.exe`)) {
    return `set -gx PATH "${quotePosixDoubleQuoted(binDirectory)}" $PATH`
  }

  if (shellName.endsWith('/csh') || shellName.endsWith('/tcsh')) {
    return `setenv PATH "${quotePosixDoubleQuoted(binDirectory)}:$PATH"`
  }

  const separator = isWindows ? ';' : ':'
  return `export PATH="${quotePosixDoubleQuoted(binDirectory)}${separator}$PATH"`
}

function quotePowerShell(value: string) {
  return `'${value.replaceAll("'", "''")}'`
}

function quotePosixDoubleQuoted(value: string) {
  return value
    .replaceAll('\\', String.raw`\\`)
    .replaceAll('"', String.raw`\"`)
    .replaceAll('$', String.raw`\$`)
    .replaceAll('`', '\\`')
}
