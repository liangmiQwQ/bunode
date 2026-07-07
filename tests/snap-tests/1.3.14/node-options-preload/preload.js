globalThis.fromNodeOptionsValue = 'loaded'
const normalizedArgv0 = process.argv[0].replaceAll('\\', '/')

globalThis.fromNodeOptionsArgv0Patched = ['/.dev/bin/node', '/.dev/node.exe'].some(suffix =>
  normalizedArgv0.endsWith(suffix)
)
