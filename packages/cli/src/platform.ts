import { createRequire } from 'node:module'
import { dirname } from 'node:path'
import { arch, platform, report } from 'node:process'

export interface PlatformPackage {
  arch: NodeJS.Architecture
  cpu: string
  libc?: 'glibc' | 'musl'
  name: string
  os: NodeJS.Platform
  target: string
}

export const platformPackages: PlatformPackage[] = [
  {
    arch: 'arm64',
    cpu: 'arm64',
    name: '@bunode/cli-darwin-arm64',
    os: 'darwin',
    target: 'aarch64-apple-darwin'
  },
  {
    arch: 'x64',
    cpu: 'x64',
    name: '@bunode/cli-darwin-x64',
    os: 'darwin',
    target: 'x86_64-apple-darwin'
  },
  {
    arch: 'arm64',
    cpu: 'arm64',
    libc: 'glibc',
    name: '@bunode/cli-linux-arm64-gnu',
    os: 'linux',
    target: 'aarch64-unknown-linux-gnu'
  },
  {
    arch: 'arm64',
    cpu: 'arm64',
    libc: 'musl',
    name: '@bunode/cli-linux-arm64-musl',
    os: 'linux',
    target: 'aarch64-unknown-linux-musl'
  },
  {
    arch: 'x64',
    cpu: 'x64',
    libc: 'glibc',
    name: '@bunode/cli-linux-x64-gnu',
    os: 'linux',
    target: 'x86_64-unknown-linux-gnu'
  },
  {
    arch: 'x64',
    cpu: 'x64',
    libc: 'musl',
    name: '@bunode/cli-linux-x64-musl',
    os: 'linux',
    target: 'x86_64-unknown-linux-musl'
  },
  {
    arch: 'arm64',
    cpu: 'arm64',
    name: '@bunode/cli-win32-arm64-msvc',
    os: 'win32',
    target: 'aarch64-pc-windows-msvc'
  },
  {
    arch: 'x64',
    cpu: 'x64',
    name: '@bunode/cli-win32-x64-msvc',
    os: 'win32',
    target: 'x86_64-pc-windows-msvc'
  }
]

export function findPlatformPackage(): PlatformPackage {
  const libc = platform === 'linux' ? detectLibc() : undefined
  const selected = platformPackages.find(
    item => item.os === platform && item.arch === arch && item.libc === libc
  )

  if (!selected) {
    throw new Error(`Bunode does not provide a binary for ${platform}-${arch}${libcSuffix(libc)}.`)
  }

  return selected
}

export function findPlatformPackageDirectory(): string {
  const selected = findPlatformPackage()
  const require = createRequire(import.meta.url)

  try {
    return dirname(require.resolve(`${selected.name}/package.json`))
  } catch (error) {
    throw new Error(
      `The optional package ${selected.name} is missing. Reinstall @bunode/cli for this platform.`,
      { cause: error }
    )
  }
}

function detectLibc(): 'glibc' | 'musl' {
  const { header } = report.getReport() as { header: { glibcVersionRuntime?: string } }
  return header.glibcVersionRuntime ? 'glibc' : 'musl'
}

function libcSuffix(libc: PlatformPackage['libc']): string {
  return libc ? `-${libc}` : ''
}
