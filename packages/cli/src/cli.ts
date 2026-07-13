import { spawn } from 'node:child_process'
import { fileURLToPath } from 'node:url'

import { installShellrc, shellrcGuard } from 'free-shellrc'
import pc from 'picocolors'

import {
  createPathCommand,
  getBunodeInstallation,
  isExecutable,
  syncBunodeInstallation
} from './install.ts'

export async function runCli(entryUrl: string | URL): Promise<void> {
  const entryPath = fileURLToPath(entryUrl)
  const diagnostic = shellrcGuard(entryUrl)

  let binDirectory: string
  let bunodeBinary: string
  try {
    const installation = await syncBunodeInstallation(entryPath)
    const { binDirectory: installedBinDirectory, bunodeBinary: installedBinary } = installation
    binDirectory = installedBinDirectory
    bunodeBinary = installedBinary
  } catch (error) {
    printWarning(`JavaScript wrapper failed: ${getErrorMessage(error)}`)
    await runFallback()
    return
  }

  if (diagnostic) {
    printWarning(diagnostic.message)
    await runBinary(bunodeBinary)
    return
  }

  try {
    const changed = await installShellrc(shell => createPathCommand(shell, binDirectory))

    if (changed) {
      process.stderr.write(
        `${pc.green('Bunode is ready.')} Restart this shell to use ${binDirectory}.\n`
      )
    }
  } catch (error) {
    printWarning(`JavaScript wrapper failed: ${getErrorMessage(error)}`)
    await runFallback()
    return
  }

  await runBinary(bunodeBinary)
}

async function runFallback(): Promise<void> {
  const { bunodeBinary } = getBunodeInstallation()
  if (!(await isExecutable(bunodeBinary))) {
    process.stderr.write(`${pc.red('Bunode could not find an installed native CLI.')}\n`)
    process.exitCode = 1
    return
  }

  printWarning('Using the previously installed native Bunode CLI.')
  await runBinary(bunodeBinary)
}

function runBinary(binary: string): Promise<void> {
  return new Promise((resolve, reject) => {
    const child = spawn(binary, process.argv.slice(2), { stdio: 'inherit' })

    child.on('error', reject)
    child.on('exit', (code, signal) => {
      if (signal) {
        process.kill(process.pid, signal)
        return
      }
      process.exitCode = code ?? 1
      resolve()
    })
  })
}

function printWarning(message: string): void {
  process.stderr.write(`${pc.yellow('warning:')} ${message}\n`)
}

function getErrorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error)
}
