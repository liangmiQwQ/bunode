// @env node

import { execFile, spawn } from 'node:child_process'
import { readFile, writeFile } from 'node:fs/promises'
import { promisify } from 'node:util'

import { versionBump } from 'bumpp'

import { platformPackages } from '../packages/cli/src/platform.ts'

const baseBranch = 'main'
// Node's promisify accepts the value-returning execFile callback despite the generic void constraint.
// oxlint-disable-next-line typescript/strict-void-return
const execFileAsync = promisify(execFile)
const [version] = process.argv.slice(2)

await (version === '--check' ? assertVersionsMatch() : createRelease(version))

async function createRelease(nextVersion: string | undefined): Promise<void> {
  if (!nextVersion) {
    throw new Error('Usage: vp run release -- <version>')
  }

  // 1. Start from the exact remote main revision.
  await run('gh', ['auth', 'status'])
  await run('git', ['fetch', 'origin', baseBranch])
  await assertOutput('git', ['branch', '--show-current'], baseBranch)
  await assertOutput('git', ['status', '--porcelain'], '')
  await assertOutput(
    'git',
    ['rev-parse', 'HEAD'],
    await capture('git', ['rev-parse', `origin/${baseBranch}`])
  )

  // 2. Update and validate the lockstep npm and Rust versions.
  await assertVersionsMatch()
  const manifestPath = 'packages/cli/package.json'
  const originalManifest = await readFile(manifestPath, 'utf8')
  const result = await versionBump({
    release: nextVersion,
    files: [manifestPath],
    commit: false,
    tag: false,
    push: false,
    confirm: false
  })
  try {
    await assertVersionAvailable(result.newVersion)
  } catch (error) {
    await writeFile(manifestPath, originalManifest)
    throw error
  }
  const releaseBranch = `release/v${result.newVersion}`
  const releaseTitle = `chore(release): v${result.newVersion}`
  await run('cargo', ['set-version', result.newVersion, '--workspace', '--offline'])
  await assertVersionsMatch()
  await run('vpr', ['check'])
  await run('vpr', ['test'])

  // 3. Generate release notes before creating Git state.
  const releaseNotes = await capture('gh', [
    'api',
    '--method',
    'POST',
    'repos/{owner}/{repo}/releases/generate-notes',
    '-f',
    `tag_name=v${result.newVersion}`,
    '-f',
    `target_commitish=${baseBranch}`,
    '--jq',
    '.body'
  ])

  // 4. Push the version-only branch and open the PR.
  await run('git', ['switch', '-c', releaseBranch])
  await run('git', ['add', 'Cargo.toml', 'Cargo.lock', 'packages/cli/package.json'])
  await run('git', ['commit', '-m', releaseTitle])
  await run('git', ['push', '--set-upstream', 'origin', releaseBranch])

  const pullRequestUrl = await capture('gh', [
    'pr',
    'create',
    '--base',
    baseBranch,
    '--head',
    releaseBranch,
    '--title',
    releaseTitle,
    '--body',
    releaseNotes,
    '--assignee',
    '@me'
  ])
  process.stdout.write(`${pullRequestUrl}\n`)
}

async function assertVersionAvailable(version: string): Promise<void> {
  const packageNames = ['@bunode/cli', ...platformPackages.map(({ name }) => name)]
  const occupiedPackages: string[] = []

  for (const packageName of packageNames) {
    const packageUrl = `https://registry.npmjs.org/${encodeURIComponent(packageName)}/${encodeURIComponent(version)}`
    const response = await fetch(packageUrl)

    if (response.ok) {
      occupiedPackages.push(packageName)
    } else if (response.status !== 404) {
      throw new Error(
        `Cannot check ${packageName}@${version} on npm: ${response.status} ${response.statusText}`
      )
    }
  }

  if (occupiedPackages.length > 0) {
    throw new Error(
      `Release version ${version} is already published for: ${occupiedPackages.join(', ')}`
    )
  }
}

async function assertVersionsMatch(): Promise<void> {
  const manifest = JSON.parse(await readFile('packages/cli/package.json', 'utf8')) as {
    version: string
  }
  const metadata = JSON.parse(
    await capture('cargo', ['metadata', '--format-version', '1', '--no-deps', '--offline'])
  ) as { packages: { name: string; version: string }[] }
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

async function assertOutput(command: string, args: string[], expected: string): Promise<void> {
  const output = await capture(command, args)

  if (output !== expected) {
    throw new Error(
      `${command} ${args.join(' ')} returned ${JSON.stringify(output)}, expected ${JSON.stringify(expected)}`
    )
  }
}

async function capture(command: string, args: string[]): Promise<string> {
  const { stdout } = await execFileAsync(command, args, { encoding: 'utf8' })
  return stdout.trim()
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
