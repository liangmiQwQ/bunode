import { base } from '@liangmi/vp-config'

const cargoTask = {
  input: [{ auto: true }, '!target/**', '!.dev/**'],
  output: []
}

export default base({
  run: {
    tasks: {
      build: {
        command: 'echo "Build complete"',
        dependsOn: ['build:rs', 'build:js'],
        output: []
      },
      'build:js': {
        command: 'vp pack',
        cwd: 'packages/cli',
        input: [{ auto: true }, '!packages/cli/dist/**'],
        output: [{ pattern: 'packages/cli/dist/**', base: 'workspace' }]
      },
      'build:rs': {
        command: 'cargo build -p bunode',
        ...cargoTask,
        output: [{ auto: true }]
      },
      dev: {
        command: 'node scripts/setup-dev.ts',
        dependsOn: ['build:rs'],
        cache: false
      },
      test: {
        command: ['cargo test --workspace', 'vp test'],
        dependsOn: ['build:rs'],
        ...cargoTask
      },
      check: {
        command: [
          'node scripts/release.ts --check',
          'vpr ccheck',
          'cargo fmt --check',
          'cargo clippy --workspace --all-targets -- -D warnings'
        ],
        ...cargoTask
      }
    }
  }
})
