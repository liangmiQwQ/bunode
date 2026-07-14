import { createHash } from 'node:crypto'
import { mkdir, mkdtemp, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import { platform } from 'node:process'
import { pathToFileURL } from 'node:url'

import { expect, it, vi } from 'vite-plus/test'

import { runCli } from './cli.ts'
import type * as InstallModule from './install.ts'

const fixture = vi.hoisted(() => ({
  binDirectory: '',
  bunodeBinary: '',
  nodeBinary: '',
  syncCalls: 0
}))

vi.mock(import('./install.ts'), async importOriginal => {
  const original = await importOriginal<typeof InstallModule>()

  return {
    ...original,
    getBunodeInstallation: () => fixture,
    syncBunodeInstallation: () => {
      fixture.syncCalls += 1
      return Promise.resolve(fixture)
    }
  }
})

it('bootstraps a missing native CLI while a shell restart is pending', async () => {
  const root = await mkdtemp(join(tmpdir(), 'bunode-cli-restart-'))
  const packageName = `@bunode/restart-test-${process.pid}-${Date.now()}`
  const entryPath = join(root, 'bin/bunode.mjs')
  const restartPath = join(
    tmpdir(),
    `.free-shellrc-${createHash('sha256').update(packageName).digest('hex').slice(0, 24)}.restart`
  )
  const originalShell = process.env.SHELL
  const originalArgv = process.argv
  const originalExitCode = process.exitCode
  const extension = platform === 'win32' ? '.exe' : ''

  fixture.binDirectory = join(root, 'home/bin')
  fixture.bunodeBinary = resolve(import.meta.dirname, `../../../target/debug/bunode${extension}`)
  fixture.nodeBinary = join(root, 'home/node')
  fixture.syncCalls = 0

  try {
    await mkdir(join(root, 'bin'), { recursive: true })
    await mkdir(fixture.binDirectory, { recursive: true })
    await writeFile(join(root, 'package.json'), JSON.stringify({ name: packageName }))
    await writeFile(entryPath, '')
    await writeFile(restartPath, '')
    process.env.SHELL = '/bin/zsh'
    process.argv = [process.execPath, entryPath, '--version']
    process.exitCode = undefined

    await runCli(pathToFileURL(entryPath))

    expect(fixture.syncCalls).toBe(1)
    expect(process.exitCode).toBe(0)
  } finally {
    process.env.SHELL = originalShell
    process.argv = originalArgv
    process.exitCode = originalExitCode
    await rm(restartPath, { force: true })
    await rm(root, { force: true, recursive: true })
  }
})
