const { fork } = require('node:child_process')

function writeLine(value) {
  process.stdout.write(`${value}\n`)
}

if (process.argv[2] === 'child') {
  writeLine(`childExecArgv=${JSON.stringify(process.execArgv)}`)
  writeLine(`childArgv=${process.argv.slice(2).join('|')}`)
} else {
  writeLine(`parentExecArgv=${JSON.stringify(process.execArgv)}`)
  const originalExecArgv = process.execArgv
  process.execArgv = []
  writeLine(`execArgvWritable=${process.execArgv.length === 0}`)
  process.execArgv = originalExecArgv

  const child = fork(__filename, ['child'], {
    stdio: ['ignore', 'pipe', 'inherit', 'ipc']
  })

  child.stdout.setEncoding('utf8')
  child.stdout.on('data', chunk => {
    process.stdout.write(chunk)
  })

  child.on('exit', code => {
    process.exitCode = code ?? 1
  })
}
