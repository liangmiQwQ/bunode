import { base } from '@liangmi/vp-config'

import registerVpConfig from './packages/cli/vite.config.ts'

const lintOverride = registerVpConfig.lint
delete lintOverride?.categories
delete lintOverride?.options
delete lintOverride?.ignorePatterns
delete lintOverride?.overrides

export default base({
  staged: {
    '*': 'vp check --fix'
  },
  lint: {
    overrides: [{ files: ['packages/registry'], ...lintOverride }]
  }
})
