// @env node

import { execFile, spawn } from 'node:child_process'
import { readFile } from 'node:fs/promises'
import { promisify } from 'node:util'

import { versionBump } from 'bumpp'

interface CargoMetadata {
  packages: {
    name: string
    version: string
  }[]
}

const cliManifestPath = 'packages/cli/package.json'
// Node's promisify accepts the value-returning execFile callback despite the generic void constraint.
// oxlint-disable-next-line typescript/strict-void-return
const execFileAsync = promisify(execFile)
const [version] = process.argv.slice(2)

await main()

async function main(): Promise<void> {
  if (version === '--check') {
    await assertVersionsMatch()
    return
  }

  await prepareRelease(version)
}

async function prepareRelease(nextVersion: string | undefined): Promise<void> {
  if (!nextVersion) {
    throw new Error('Usage: vp run prepare-release -- <version>')
  }

  // 1. Refuse to build a release PR from versions that are already out of sync.
  await run('cargo', ['set-version', '--version'])
  await assertVersionsMatch()

  // 2. Update both package ecosystems without creating release commits or tags.
  const result = await versionBump({
    release: nextVersion,
    files: [cliManifestPath],
    commit: false,
    tag: false,
    push: false,
    confirm: false
  })

  await run('cargo', ['set-version', result.newVersion, '--workspace', '--offline'])
  await run('cargo', ['check', '--workspace'])

  // 3. Keep the PR creation workflow from opening a partially updated release.
  await assertVersionsMatch()
}

async function assertVersionsMatch(): Promise<void> {
  const manifest = JSON.parse(await readFile(cliManifestPath, 'utf8')) as { version: string }
  const { stdout } = await execFileAsync(
    'cargo',
    ['metadata', '--format-version', '1', '--no-deps', '--offline'],
    { encoding: 'utf8' }
  )
  const metadata = JSON.parse(stdout) as CargoMetadata
  const bunodePackage = metadata.packages.find(packageMetadata => packageMetadata.name === 'bunode')

  if (!bunodePackage) {
    throw new Error('Cargo metadata does not contain the bunode package')
  }

  if (manifest.version !== bunodePackage.version) {
    throw new Error(
      `Release versions do not match: @bunode/cli is ${manifest.version}, bunode is ${bunodePackage.version}`
    )
  }
}

async function run(command: string, args: string[]): Promise<void> {
  const child = spawn(command, args, { stdio: 'inherit' })

  await new Promise<void>((resolve, reject) => {
    child.once('error', reject)
    child.once('exit', code => {
      if (code === 0) {
        resolve()
      } else {
        reject(new Error(`${command} exited with code ${code ?? 'unknown'}`))
      }
    })
  })
}
