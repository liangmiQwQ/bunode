import { spawn } from 'node:child_process'
import { appendFile, copyFile, mkdir, readFile } from 'node:fs/promises'
import { homedir, tmpdir } from 'node:os'
import { dirname, join, resolve } from 'node:path'
import { arch, platform } from 'node:process'
import { fileURLToPath } from 'node:url'

// 1. Keep the project runtime declaration as the only CI version source.
// oxlint-disable-next-line unicorn/prefer-import-meta-properties -- This runs before CI installs the project's Node version.
const projectRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const packageJson = JSON.parse(await readFile(resolve(projectRoot, 'package.json'), 'utf8'))
const runtimeVersion = packageJson.devEngines?.runtime?.version
const [nodeVersion, bunVersion, extra] = runtimeVersion?.split('+bun.') ?? []

if (!runtimeVersion || !nodeVersion || !bunVersion || extra) {
  throw new Error('package.json#devEngines.runtime.version must use <node>+bun.<bun>')
}

// 2. Give the composite action the plain Node bootstrap and qualified runtime versions.
if (process.argv.includes('--github-output')) {
  const output = process.env.GITHUB_OUTPUT
  if (!output) {
    throw new Error('GITHUB_OUTPUT is required')
  }
  await appendFile(output, `node-version=${nodeVersion}\nruntime-version=${runtimeVersion}\n`)
  process.exit(0)
}

// 3. Prove normal project commands resolve to Bunode after bootstrap.
if (process.argv.includes('--verify')) {
  if (process.version !== `v${nodeVersion}` || process.versions.bun !== bunVersion) {
    throw new Error(
      `Bunode runtime verification failed: got ${process.version} with Bun ${process.versions.bun}`
    )
  }
  process.stdout.write(
    `Using Bunode ${runtimeVersion} on ${platform} ${arch} for dependency install and project tasks.\n`
  )
  process.exit(0)
}

const isWindows = platform === 'win32'
const extension = isWindows ? '.exe' : ''
const vpHome = process.env.VP_HOME ?? join(homedir(), '.vite-plus')
const bunodeHome = resolve(process.env.RUNNER_TEMP ?? tmpdir(), 'bunode-ci')
const targetPrefix = join(vpHome, 'js_runtime', 'node', runtimeVersion)
const environment = { ...process.env, BUNODE_HOME: bunodeHome }

// 4. Bootstrap the native CLI and wrapper without relying on a published Bunode package.
await mkdir(bunodeHome, { recursive: true })
await copyFile(
  resolve(projectRoot, 'target/debug', `node${extension}`),
  resolve(bunodeHome, `node${extension}`)
)
await run(
  resolve(projectRoot, 'target/debug', `bunode${extension}`),
  ['patch', bunVersion, process.execPath, '--copy', targetPrefix, '--yes'],
  environment
)

function run(command, args, env) {
  return new Promise((resolve, reject) => {
    const child = spawn(command, args, { env, stdio: 'inherit' })

    child.on('error', reject)
    child.on('close', code => {
      if (code === 0) {
        resolve()
      } else {
        reject(new Error(`${command} exited with code ${code ?? 1}`))
      }
    })
  })
}
