# Bunode Snap Tests

These tests protect the Rust wrapper's Node-compatible CLI behavior. Each case lives in:

```txt
tests/snap-tests/<bun-version>/<case-name>/
```

Every case directory contains:

- `snap.json`: machine-readable test metadata and shell commands.
- `snap.txt`: expected output with colors disabled.
- `snap-colored.txt`: expected output with forced colors enabled.
- Fixture files, when the behavior needs scripts, env files, stdin files, or a small driver.

The runner builds the development wrapper, installs the requested Bun version into `.dev`, then runs each command from the case directory twice. It concatenates stdout and stderr in command order and compares that output with the two snapshot files.

## Writing Cases

Keep cases focused on one behavior contract. Prefer a new directory with a clear name over adding unrelated checks to an existing driver.

Use plain command strings for simple shell commands:

```json
{
  "description": "The wrapper can execute eval, print, script file, and stdin entry points.",
  "commands": ["node --version"]
}
```

Use command objects when the case needs argv-safe arguments, stdin, environment overrides, or a non-zero expected exit code:

```json
{
  "description": "NODE_OPTIONS rejects disallowed flags.",
  "commands": [
    {
      "command": "node",
      "args": ["--version"],
      "env": {
        "NODE_OPTIONS": "--eval 1"
      },
      "exitCode": 9
    }
  ]
}
```

`description` should explain the behavior being protected, not repeat the directory name. Keep behavior directly in `commands` by default. `driver.js` is a last resort for cases that need process orchestration the runner cannot express cleanly, such as holding stdin open or generating very large input.

Use `after` for cleanup only when the cleanup cannot live in a `try`/`finally` inside the driver. Use `ignore` only for behavior that genuinely differs by `process.platform`.
