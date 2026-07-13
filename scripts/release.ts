// @env node

import { execFile, spawn } from 'node:child_process'
import { promisify } from 'node:util'

// Node's promisify accepts the value-returning execFile callback despite the generic void constraint.
// oxlint-disable-next-line typescript/strict-void-return
const execFileAsync = promisify(execFile)
const [version] = process.argv.slice(2)

if (!version) {
  throw new Error('Usage: vp run release -- <version>')
}

const baseBranch = 'main'
const releaseBranch = `release/v${version}`
const releaseTitle = `chore(release): v${version}`

// 1. Start from the exact remote main revision so the release log and PR contain the same changes.
await run('gh', ['auth', 'status'])
await run('git', ['fetch', 'origin', baseBranch])
await assertOutput('git', ['branch', '--show-current'], baseBranch)
await assertOutput('git', ['status', '--porcelain'], '')
await assertOutput(
  'git',
  ['rev-parse', 'HEAD'],
  await capture('git', ['rev-parse', `origin/${baseBranch}`])
)

// 2. Prepare and validate the lockstep npm and Rust versions before creating Git state.
await run(process.execPath, ['scripts/prepare-release.ts', version])
await run('vpr', ['check'])
await run('vpr', ['test'])

// 3. Let GitHub generate the same release log format used by GitHub Releases.
const releaseNotes = await capture('gh', [
  'api',
  '--method',
  'POST',
  'repos/{owner}/{repo}/releases/generate-notes',
  '-f',
  `tag_name=v${version}`,
  '-f',
  `target_commitish=${baseBranch}`,
  '--jq',
  '.body'
])

// 4. Push the version-only release branch and open the PR as the authenticated gh user.
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
