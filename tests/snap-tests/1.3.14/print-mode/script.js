import { basename } from 'node:path'

process.stdout.write(
  [
    `scriptExecArgv=${JSON.stringify(process.execArgv)}`,
    `scriptArgv=${process.argv
      .slice(1)
      .map(value => basename(value))
      .join('|')}`
  ].join('\n')
)
process.stdout.write('\n')
