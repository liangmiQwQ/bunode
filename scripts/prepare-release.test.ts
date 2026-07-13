import { spawn } from 'node:child_process'
import { chmod, copyFile, link, mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { delimiter, join } from 'node:path'
import { platform } from 'node:process'

import { expect, it } from 'vite-plus/test'

const projectRoot = join(import.meta.dirname, '..')
const isWindows = platform === 'win32'

it('prepares matching npm and Cargo release versions in an isolated workspace', async () => {
  const root = await mkdtemp(join(tmpdir(), 'bunode-release-'))
  const binDirectory = join(root, 'bin')
  const fakeCargoPath = join(binDirectory, isWindows ? 'cargo.exe' : 'cargo')
  const fakeCargoModule = join(root, 'fake-cargo.cjs')

  try {
    await mkdir(join(root, 'packages/cli'), { recursive: true })
    await mkdir(binDirectory)
    await writeFile(join(root, 'package.json'), '{"private":true,"type":"module"}\n')
    await writeFile(
      join(root, 'packages/cli/package.json'),
      '{"name":"@bunode/cli","version":"0.0.0-alpha.0"}\n'
    )
    await writeFile(
      join(root, 'Cargo.toml'),
      '[workspace]\nmembers = ["crates/*"]\n\n[workspace.package]\nversion = "0.0.0-alpha.0"\n'
    )
    await writeFile(join(root, 'Cargo.lock'), 'name = "bunode"\nversion = "0.0.0-alpha.0"\n')
    await writeFile(fakeCargoModule, fakeCargoSource)
    await copyExecutable(fakeCargoPath)

    await runReleaseScript(root, binDirectory, fakeCargoModule, '0.0.0-alpha.1')

    const cliManifest = JSON.parse(await readFile(join(root, 'packages/cli/package.json'), 'utf8'))
    expect(cliManifest.version).toBe('0.0.0-alpha.1')
    await expect(readFile(join(root, 'Cargo.toml'), 'utf8')).resolves.toContain(
      'version = "0.0.0-alpha.1"'
    )
    await expect(readFile(join(root, 'Cargo.lock'), 'utf8')).resolves.toContain(
      'version = "0.0.0-alpha.1"'
    )
  } finally {
    await rm(root, { force: true, recursive: true })
  }
})

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

function runReleaseScript(
  cwd: string,
  binDirectory: string,
  fakeCargoModule: string,
  version: string
): Promise<string> {
  return new Promise((resolve, reject) => {
    const child = spawn(
      process.execPath,
      [join(projectRoot, 'scripts/prepare-release.ts'), version],
      {
        cwd,
        env: {
          ...process.env,
          NODE_OPTIONS: `--require=${fakeCargoModule}`,
          PATH: `${binDirectory}${delimiter}${process.env.PATH ?? ''}`
        }
      }
    )
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
      } else {
        reject(new Error(`prepare-release exited with ${code ?? 1}: ${output}`))
      }
    })
  })
}

const fakeCargoSource = String.raw`
const { basename, join } = require('node:path')

if (basename(process.execPath).startsWith('cargo')) {
  const fs = require('node:fs')
  const args = process.argv.slice(1)
  args[0] = basename(args[0])
  const manifestPath = join(process.cwd(), 'Cargo.toml')
  const lockPath = join(process.cwd(), 'Cargo.lock')
  const readVersion = () => fs.readFileSync(manifestPath, 'utf8').match(/version = "([^"]+)"/)[1]

  if (args[0] === 'metadata') {
    process.stdout.write(JSON.stringify({ packages: [{ name: 'bunode', version: readVersion() }] }))
  } else if (args[0] === 'set-version' && args[1] === '--version') {
    process.stdout.write('cargo-edit-set-version test\n')
  } else if (args[0] === 'set-version') {
    const nextVersion = args[1]
    fs.writeFileSync(
      manifestPath,
      fs.readFileSync(manifestPath, 'utf8').replace(/version = "[^"]+"/, 'version = "' + nextVersion + '"')
    )
    fs.writeFileSync(
      lockPath,
      fs.readFileSync(lockPath, 'utf8').replace(/version = "[^"]+"/, 'version = "' + nextVersion + '"')
    )
  } else if (args[0] !== 'check') {
    process.stderr.write('Unexpected cargo arguments: ' + args.join(' ') + '\n')
    process.exitCode = 1
  }

  process.exit()
}
`
