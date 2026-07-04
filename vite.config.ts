import { base } from '@liangmi/vp-config'

export default base({
  run: {
    tasks: {
      test: {
        command: ['vp test', 'cargo test --workspace'],
        input: [{ auto: true }, '!target/**']
      },
      check: {
        command: [
          'vpr ccheck',
          'cargo fmt --check',
          'cargo clippy --workspace --all-targets -- -D warnings'
        ],
        input: [{ auto: true }, '!target/**']
      }
    }
  }
})
