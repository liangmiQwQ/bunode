import { createHash } from 'node:crypto'
import { chmod, mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { pathToFileURL } from 'node:url'

import { expect, it, vi } from 'vite-plus/test'

import { runCli } from './cli.ts'
import type * as InstallModule from './install.ts'

const fixture = vi.hoisted(() => ({
  binDirectory: '',
  bunodeBinary: '',
  nodeBinary: '',
  outputPath: '',
  syncCalls: 0
}))

vi.mock(import('./install.ts'), async importOriginal => {
  const original = await importOriginal<typeof InstallModule>()

  return {
    ...original,
    getBunodeInstallation: () => fixture,
    syncBunodeInstallation: async () => {
      fixture.syncCalls += 1
      await writeFile(
        fixture.bunodeBinary,
        `#!/bin/sh\nprintf native > '${fixture.outputPath}'\n`,
        { mode: 0o755 }
      )
      await chmod(fixture.bunodeBinary, 0o755)
      return fixture
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

  fixture.binDirectory = join(root, 'home/bin')
  fixture.bunodeBinary = join(root, 'home/bunode')
  fixture.nodeBinary = join(root, 'home/node')
  fixture.outputPath = join(root, 'native-output')
  fixture.syncCalls = 0

  try {
    await mkdir(join(root, 'bin'), { recursive: true })
    await mkdir(fixture.binDirectory, { recursive: true })
    await writeFile(join(root, 'package.json'), JSON.stringify({ name: packageName }))
    await writeFile(entryPath, '')
    await writeFile(restartPath, '')
    process.env.SHELL = '/bin/zsh'

    await runCli(pathToFileURL(entryPath))

    expect(fixture.syncCalls).toBe(1)
    await expect(readFile(fixture.outputPath, 'utf8')).resolves.toBe('native')
  } finally {
    process.env.SHELL = originalShell
    await rm(restartPath, { force: true })
    await rm(root, { force: true, recursive: true })
  }
})
