#!/usr/bin/env node
const wrapperMarker = process.env.BUNODE_WRAPPER_MARKER

try {
  const { runCli } = await import('../dist/cli.mjs')

  if (wrapperMarker) {
    delete process.env.BUNODE_WRAPPER_MARKER
    try {
      const { writeFile } = await import('node:fs/promises')
      await writeFile(wrapperMarker, '')
    } catch {
      // The launcher treats a missing marker as a wrapper startup failure.
    }
  }

  await runCli(import.meta.url)
} catch (error) {
  process.stderr.write(`Bunode failed: ${getErrorMessage(error)}\n`)
  process.exitCode = 1
}

function getErrorMessage(error) {
  return error instanceof Error ? error.message : String(error)
}
