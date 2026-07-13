import { chmod, copyFile, mkdir, readFile, rm, writeFile } from 'node:fs/promises'
import { basename, join, resolve } from 'node:path'

import { platformPackages } from '../src/platform.ts'

interface PackageManifest {
  optionalDependencies?: Record<string, string>
  repository?: unknown
  version: string
}

const packageRoot = resolve(import.meta.dirname, '..')
const artifactRoot = resolve(process.argv[2] ?? 'release-artifacts')
const outputRoot = join(packageRoot, 'npm')
const manifestPath = join(packageRoot, 'package.json')
const manifest = JSON.parse(await readFile(manifestPath, 'utf8')) as PackageManifest

await rm(outputRoot, { force: true, recursive: true })

for (const platformPackage of platformPackages) {
  const extension = platformPackage.os === 'win32' ? '.exe' : ''
  const artifactDirectory = join(artifactRoot, `bunode-${platformPackage.target}`)
  const packageDirectory = join(outputRoot, basename(platformPackage.name))
  const files = [`bunode${extension}`, `node${extension}`]

  await mkdir(packageDirectory, { recursive: true })
  for (const file of files) {
    const destination = join(packageDirectory, file)
    await copyFile(join(artifactDirectory, file), destination)
    if (platformPackage.os !== 'win32') {
      await chmod(destination, 0o755)
    }
  }

  const platformManifest = {
    name: platformPackage.name,
    version: manifest.version,
    description: `Bunode native binaries for ${basename(platformPackage.name)}`,
    license: 'MIT',
    repository: manifest.repository,
    os: [platformPackage.os],
    cpu: [platformPackage.cpu],
    ...(platformPackage.libc ? { libc: [platformPackage.libc] } : {}),
    files,
    exports: {
      './package.json': './package.json'
    },
    publishConfig: {
      access: 'public'
    }
  }

  await writeFile(
    join(packageDirectory, 'package.json'),
    `${JSON.stringify(platformManifest, undefined, 2)}\n`
  )
}

manifest.optionalDependencies = Object.fromEntries(
  platformPackages.map(platformPackage => [platformPackage.name, manifest.version])
)
await writeFile(manifestPath, `${JSON.stringify(manifest, undefined, 2)}\n`)
