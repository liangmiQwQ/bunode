import { spawn } from 'node:child_process'
import { chmod, copyFile, link, mkdir, mkdtemp, rm, unlink, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { delimiter, dirname, join } from 'node:path'
import { platform } from 'node:process'

import { expect, it } from 'vite-plus/test'

import { syncBunodeInstallation } from './install.ts'

const isWindows = platform === 'win32'

it(
  'keeps the native CLI usable when the JavaScript wrapper disappears',
  { timeout: 30_000 },
  async () => {
    const root = await mkdtemp(join(tmpdir(), 'bunode-cli-'))
    const userHome = join(root, 'home')
    const packageDirectory = join(root, 'platform-package')
    const entryPath = join(root, 'bunode-entry.mjs')

    try {
      await mkdir(packageDirectory, { recursive: true })
      await copyExecutable(join(packageDirectory, isWindows ? 'bunode.exe' : 'bunode'))
      await copyExecutable(join(packageDirectory, isWindows ? 'node.exe' : 'node'))
      await writeFile(entryPath, `process.stdout.write('javascript wrapper\\n')\n`)

      const installation = await syncBunodeInstallation(entryPath, userHome, packageDirectory)
      const launcher = join(installation.binDirectory, isWindows ? 'bunode.ps1' : 'bunode')

      await expect(run(installation.nodeBinary, ['--version'])).resolves.toContain(process.version)
      await expect(run(launcher, ['--version'])).resolves.toContain('javascript wrapper')

      await unlink(entryPath)
      const fallback = await run(launcher, ['--version'])
      expect(fallback).toContain('JavaScript wrapper unavailable')
      expect(fallback).toContain(process.version)
    } finally {
      await rm(root, { force: true, recursive: true })
    }
  }
)

async function copyExecutable(path: string): Promise<void> {
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

function run(command: string, args: string[]): Promise<string> {
  return new Promise((resolve, reject) => {
    const executable = isWindows && command.endsWith('.ps1') ? 'pwsh' : command
    const executableArgs = executable === 'pwsh' ? ['-NoProfile', '-File', command, ...args] : args
    const child = spawn(executable, executableArgs, {
      env: {
        ...process.env,
        PATH: `${dirname(process.execPath)}${delimiter}${process.env.PATH ?? ''}`
      }
    })
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
        return
      }
      reject(new Error(`${command} exited with ${code ?? 1}: ${output}`))
    })
  })
}
