import { constants } from 'node:fs'
import {
  access,
  chmod,
  copyFile,
  link,
  mkdir,
  readFile,
  rename,
  rm,
  writeFile
} from 'node:fs/promises'
import { homedir } from 'node:os'
import { basename, dirname, join } from 'node:path'
import { platform } from 'node:process'

import type { Shell } from 'free-shellrc'

import { findPlatformPackageDirectory } from './platform.ts'

export interface BunodeInstallation {
  binDirectory: string
  bunodeBinary: string
  homeDirectory: string
  nodeBinary: string
}

const executableMode = 0o755
const isWindows = platform === 'win32'

export async function syncBunodeInstallation(
  entryPath: string,
  userHome = homedir(),
  platformPackageDirectory = findPlatformPackageDirectory()
): Promise<BunodeInstallation> {
  const installation = getBunodeInstallation(userHome)
  const extension = isWindows ? '.exe' : ''

  // 1. Keep native binaries outside the package manager's installation tree.
  await mkdir(installation.binDirectory, { recursive: true })
  await installExecutable(
    join(platformPackageDirectory, `bunode${extension}`),
    installation.bunodeBinary
  )
  await installExecutable(
    join(platformPackageDirectory, `node${extension}`),
    installation.nodeBinary
  )

  // 2. Point the stable user command at this package, with the native CLI as fallback.
  if (isWindows) {
    await installLauncher(
      join(installation.binDirectory, 'bunode.cmd'),
      createCommandPromptLauncher(entryPath, installation.bunodeBinary)
    )
    await installLauncher(
      join(installation.binDirectory, 'bunode.ps1'),
      createPowerShellLauncher(entryPath, installation.bunodeBinary)
    )
  } else {
    await installLauncher(
      join(installation.binDirectory, 'bunode'),
      createPosixLauncher(entryPath, installation.bunodeBinary)
    )
  }

  return installation
}

export function getBunodeInstallation(userHome = homedir()): BunodeInstallation {
  const homeDirectory = join(userHome, '.bunode')
  const extension = isWindows ? '.exe' : ''

  return {
    binDirectory: join(homeDirectory, 'bin'),
    bunodeBinary: join(homeDirectory, `bunode${extension}`),
    homeDirectory,
    nodeBinary: join(homeDirectory, `node${extension}`)
  }
}

export function createPathCommand(shell: Shell, binDirectory: string): string {
  if (shell === 'fish') {
    return `fish_add_path --prepend --move ${quoteFish(binDirectory)}`
  }
  if (shell === 'powershell' || shell === 'pwsh') {
    return `$env:Path = ${quotePowerShell(`${binDirectory};`)} + $env:Path`
  }
  return `export PATH=${quotePosix(binDirectory)}:"$PATH"`
}

export async function isExecutable(path: string): Promise<boolean> {
  try {
    await access(path, isWindows ? constants.F_OK : constants.X_OK)
    return true
  } catch {
    return false
  }
}

async function installExecutable(source: string, destination: string): Promise<void> {
  const temporary = join(
    dirname(destination),
    `.${basename(destination)}.${process.pid}.${Math.random().toString(16).slice(2)}`
  )

  try {
    try {
      await link(source, temporary)
    } catch (error) {
      if (!isLinkFallbackError(error)) {
        throw error
      }
      await copyFile(source, temporary)
    }

    if (!isWindows) {
      await chmod(temporary, executableMode)
    }
    await replaceFile(temporary, destination)
  } finally {
    await rm(temporary, { force: true })
  }
}

async function installLauncher(path: string, content: string): Promise<void> {
  try {
    if ((await readFile(path, 'utf8')) === content) {
      return
    }
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code !== 'ENOENT') {
      throw error
    }
  }

  const temporary = `${path}.${process.pid}.${Math.random().toString(16).slice(2)}`
  try {
    await writeFile(temporary, content, { mode: executableMode })
    await replaceFile(temporary, path)
  } finally {
    await rm(temporary, { force: true })
  }
}

async function replaceFile(source: string, destination: string): Promise<void> {
  try {
    await rename(source, destination)
  } catch (error) {
    if (!isWindows || !isReplaceError(error)) {
      throw error
    }
    await rm(destination, { force: true })
    await rename(source, destination)
  }
}

function createPosixLauncher(entryPath: string, nativePath: string): string {
  return `#!/bin/sh
if [ -f ${quotePosix(entryPath)} ] && command -v node >/dev/null 2>&1; then
  exec node ${quotePosix(entryPath)} "$@"
fi
printf '%s\n' 'bunode: JavaScript wrapper unavailable; using installed native CLI.' >&2
exec ${quotePosix(nativePath)} "$@"
`
}

function createCommandPromptLauncher(entryPath: string, nativePath: string): string {
  return `@echo off\r
setlocal\r
if exist ${quoteCommandPrompt(entryPath)} (\r
  where node >nul 2>nul\r
  if not errorlevel 1 (\r
    node ${quoteCommandPrompt(entryPath)} %*\r
    exit /b %errorlevel%\r
  )\r
)\r
>&2 echo bunode: JavaScript wrapper unavailable; using installed native CLI.\r
${quoteCommandPrompt(nativePath)} %*\r
exit /b %errorlevel%\r
`
}

function createPowerShellLauncher(entryPath: string, nativePath: string): string {
  return `if ((Test-Path -LiteralPath ${quotePowerShell(entryPath)}) -and (Get-Command node -ErrorAction SilentlyContinue)) {
  & node ${quotePowerShell(entryPath)} @args
  exit $LASTEXITCODE
}
[Console]::Error.WriteLine('bunode: JavaScript wrapper unavailable; using installed native CLI.')
& ${quotePowerShell(nativePath)} @args
exit $LASTEXITCODE
`
}

function quotePosix(value: string): string {
  return `'${value.replaceAll("'", `'"'"'`)}'`
}

function quoteFish(value: string): string {
  return `'${value.replaceAll('\\', String.raw`\\`).replaceAll("'", String.raw`\'`)}'`
}

function quotePowerShell(value: string): string {
  return `'${value.replaceAll("'", "''")}'`
}

function quoteCommandPrompt(value: string): string {
  return `"${value.replaceAll('"', '""')}"`
}

function isLinkFallbackError(error: unknown): boolean {
  const { code } = error as NodeJS.ErrnoException
  return code === 'EXDEV' || code === 'EPERM' || code === 'EACCES' || code === 'ENOTSUP'
}

function isReplaceError(error: unknown): boolean {
  const { code } = error as NodeJS.ErrnoException
  return code === 'EEXIST' || code === 'EPERM' || code === 'EACCES'
}
