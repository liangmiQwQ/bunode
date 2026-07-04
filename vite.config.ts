import { base } from '@liangmi/vp-config'

const cargoTask = { input: [{ auto: true }, '!target/**'], output: [] }

export default base({
  run: {
    tasks: {
      test: {
        command: ['vp test', 'cargo test --workspace'],
        ...cargoTask
      },
      check: {
        command: [
          'vpr ccheck',
          'cargo fmt --check',
          'cargo clippy --workspace --all-targets -- -D warnings'
        ],
        ...cargoTask
      }
    }
  }
})
