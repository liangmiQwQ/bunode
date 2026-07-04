import { cli } from '@liangmi/vp-config'

export default cli.only(['pack'], {
  pack: {
    entry: './src/cli.ts'
  }
})
