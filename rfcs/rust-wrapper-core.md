# Bunode wrapper core (Rust)

We provide Node.js-compatible API, and the binary will call underlying Bun.

The binary parses Node.js options as a streaming translation pass. `lexopt` should own low-level argv tokenization, while Bunode owns Node.js semantics, version-specific option support, and Bun argv construction.
Node's `-p`, `--`, `NODE_OPTIONS`, and env-file behavior do not map cleanly to generic command parsers. The parser should therefore be a long, explicit match table backed by a small expandable state object and Bun args builder segments.

`node` is the only expected filename for that binary.

## Parser architecture

The parser is not a stable public command model. It should not expose a fixed `BunodeCommandOption`-style struct as the source of truth, because supported options and translation behavior can change by Bun version.

Instead, each version-selected parser should produce an execution plan local to the executor. The plan can contain builder segments and parser state, but the final command is built from the parser output just before replacing the current process with Bun.

The option table is metadata only: names, value shape, source-mode allowance, and help text. Runtime behavior belongs in the parser's direct `lexopt` match arms, where each branch validates source mode, updates state, and appends Bun arguments to the builder.

The parser has two source modes:

| Mode     | Source             | Rule                                                                                                                                                                  |
| -------- | ------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| CLI mode | Bunode binary argv | Supports command-line-only options, Bun-specific options, script operands, and user arguments.                                                                        |
| env mode | `NODE_OPTIONS`     | Supports only options allowed in Node's environment options. It rejects operands, `--`, command options such as `--eval`, env-file options, and Bun-specific options. |

The core parse loop should use `lexopt` for tokenization and a direct match table for behavior. Each option match can do three things:

1. Validate whether the option is allowed in the current source mode.
2. Update parser state for special Node.js semantics.
3. Append translated data to the correct Bun args builder segment.

The state should stay expandable and should only store facts that cannot be reconstructed from the final Bun argv:

| State               | Purpose                                                                                                                   |
| ------------------- | ------------------------------------------------------------------------------------------------------------------------- |
| command mode        | Tracks help, version, eval, print, script, stdin, or REPL selection.                                                      |
| operand boundary    | Tracks script position, `--`, and whether later tokens are user arguments.                                                |
| print/eval state    | Tracks `-p`, `-e`, `-pe`, stdin print mode, and expression-vs-script ambiguity.                                           |
| env-file state      | Tracks CLI-discovered env files and whether dotenv `NODE_OPTIONS` needs another env-mode parse.                           |
| preload ordering    | Preserves runtime preload, translated CommonJS preloads, translated ESM preloads, and Bun preloads in the required order. |
| `execArgv` boundary | Captures the original CLI-mode Node-specific options before the script operand for `process.execArgv`.                    |

`process.execArgv` must be derived from CLI-mode Node-specific options before the script operand, not from the final Bun argv and not from every raw Bunode argv. It must exclude the executable name, script name, script arguments, and options after the script operand.

## Calling underlying Bun

The underlying Bun binary's location is certain.

On Windows, it's always `./bun/bun.exe` resolved relative to the binary location.

On Linux and macOS, it's always `../bun/bun` resolved relative to the binary location.

It's mainly designed for future tarball releasing.

## Behavior

### Version control

Bun itself has a masqueraded Node.js version, we can get it from running `bun -p "process.version"`

To keep Bun's semver semantic, we use a hacky but effective way: add a build metadata mark to Bun's own compatible-layer version.

For example: `bun v1.4.0` -> `bun -p "process.version" v26.3.0` `node v26.3.0+bun.1.4.0` (Bunode)

So if developers define `package.json#engines.node` like `>=22.18.0` or `^26.2.0`, it will keep Bunode usable.

The version is injected in `node --version`, registry and other places needs version.

We won't modify `node -p "process.version"`'s version result, we will keep it as is for internal checking to avoid confusion.

The only problem is that flag will be ignored when comparing the version (`v26.3.0+bun.1.4.0` `v26.3.0+bun.1.3.8` has completely the same priority). So it requires developers to declare its version precisely.

### `node` direct calls

We wrap `bun repl`.

We can't make the behavior 100% compatible but it is basically similar. Considering this feature is mainly for human to call, so I think it's not a big deal.

For CI and non-TTY environment, Node.js executes stdin instead of starting the REPL. Bunode starts Bun first and passes a small stdin shim that reads fd 0 inside Bun, so preloads can run and exit before an unbounded stdin pipe is drained. Plain script stdin keeps Node-like script globals through indirect eval, while stdin that needs module parsing is loaded through an in-memory Blob module so static imports and top-level await still work.

For `node -p` reading the program from stdin, Bunode passes Bun a small print shim that reads fd 0 inside Bun, exposes stdin-like globals, evaluates the user program without helper bindings colliding with user declarations, and prints strings raw while inspecting other values without awaiting promises.
For eval, print, and stdin modes, Bunode wraps user code just enough to expose Node-like `[eval]` or `[stdin]` `module`, `exports`, and `require` globals for script-shaped input. Eval and stdin only fall back to module loading after the parse probe fails, so runtime `SyntaxError` values are not executed a second time as ESM. Print mode rejects module-shaped input instead of letting Bun accept ESM that Node reports as `ERR_EVAL_ESM_CANNOT_PRINT`, while malformed script input keeps its real syntax error.

### `node [options] [ script.js ] [arguments]`

We wrap `bun run --no-install --no-env-file` for the script running.

Considering `bun run` can also trigger tasks in `package.json`, we prepend a `./` (`.\` on Windows) for pure script name (without any `/`, `\` in windows). Script names starting with `-` keep Node.js's explicit path requirement, so users should pass `./--name.js` instead of `-- --name.js`.

For Node.js options, we will try to translate them to Bun options or wrap them as much as possible, as well as Node's environment variables.

The parser should build Bun args as segmented output instead of through a stable parsed-command struct. The final Bun command is assembled after command mode is known:

| Node.js mode | Bun command shape                                                     |
| ------------ | --------------------------------------------------------------------- |
| script       | `bun run --no-install --no-env-file ... <script> ...arguments`        |
| stdin        | `bun --no-install --no-env-file ... -e <fd0 stdin shim> ...arguments` |
| eval         | `bun --no-install --no-env-file ... -e <code> ...arguments`           |
| print        | `bun --no-install --no-env-file ... -e <print shim> ...arguments`     |
| repl         | `bun repl`                                                            |

For `NODE_OPTIONS`, we follow the same translating method and handle priority the same as Node does. Real `NODE_OPTIONS` is parsed first in env mode. When real `NODE_OPTIONS` is absent, `NODE_OPTIONS` loaded from `--env-file` is parsed in env mode before CLI flags, while the `--env-file` flag itself is still forwarded to Bun for normal environment loading.

The env-file rule requires a two-pass parse when dotenv `NODE_OPTIONS` is discovered from CLI mode:

1. Parse CLI argv once to validate and forward `--env-file`, and to read dotenv `NODE_OPTIONS`.
2. If real `NODE_OPTIONS` is absent and dotenv `NODE_OPTIONS` exists, parse dotenv options in env mode before parsing the same CLI argv again.
3. Reject env-mode options that would change command mode, consume script operands, or load another env file.

### Bun-specific options

We hope users are able to control Bun's behavior.

For the options / flags that Node.js doesn't directly have, we use a `--bun` prefix. For example, `--bun-config` to specific which `bunfig.toml` is to use.

### Preload

Different from Node.js, Bun's `process` object is mutable. It gives us room to modify misleading metadata like `process.execPath` and `process.argv0`.

For explicit `node -` stdin mode, Bunode preserves `-` in Node-facing user arguments while still avoiding Bun's empty `run -` missing-module behavior.

The preload can be injected with `bun --preload`, the preload JavaScript file will be generated by bunode(`node`) binary, next to `bun` executable binary.

For eval, print, stdin and script modes, Bunode injects the preload before CLI user preloads translated from `--require`, `--import`, and `--bun-preload`, so corrected process metadata is visible to normal user code. Bun project `bunfig.toml` preloads run before CLI runtime preloads in Bun itself and cannot be reordered by the wrapper without changing cwd/config lookup behavior, so they may observe raw Bun metadata. TTY REPL mode is delegated to `bun repl` without the metadata preload because Bun's REPL does not execute runtime preloads through the same entrypoint; this is acceptable because direct REPL usage is human-facing and already listed as best-effort behavior.
Node builtin module specifiers passed to `--require` or `--import` are kept in `process.execArgv` when they came from CLI mode before the script operand, but are not translated to Bun `--preload`, because Bun expects preload values to resolve as files.
CommonJS `--require` specifiers are translated to generated wrapper preloads that call `createRequire` from the caller's current working directory, so package conditional exports and relative preloads use Node's CommonJS resolution branch instead of Bun's ESM preload resolution.
JavaScript `data:` module specifiers passed to `--import` are not supported because Bun's preload surface does not implement Node's data URL preload semantics.

### Help document

In `node --help`, we only print supported options, including bun specific translated options. And avoid printing unsupported options, environment variables and subcommands.

We can warn / error to these flags if users call them. But they should not be put in the help document to confuse users.

## Parser migration plan

1. Replace the clap-backed option schema with a `lexopt` parse loop and explicit option match table.
2. Split parser input into source modes: env mode for `NODE_OPTIONS` and CLI mode for process argv.
3. Replace the fixed parsed-command struct with a version-local execution plan made from state plus Bun args builder segments.
4. Preserve `process.execArgv` by recording CLI-mode Node-specific options before the script operand.
5. Keep env-file handling as a two-pass parse so dotenv `NODE_OPTIONS` is applied before CLI flags without forwarding env-only state to Bun.
6. Move help output to the same option table used by the parser, while still hiding unsupported or intentionally rejected flags.
7. Update Rust util tests and snap tests for `-p`, `--`, `NODE_OPTIONS`, `--env-file`, preloads, unsupported options, and `process.execArgv` boundaries.
8. Remove the clap dependency after the parser, help output, and tests no longer use it.

## Non Goal

### Modifying output

Bun's output system (error rendering, repl interactive logic) is quite different from Node.js's. It's difficult and meaningless to make the output completely the same. Most cases, output is for humans and agents to read, and Bun's output is readable.

Modifying output needs PTY handling, which will bring complexity and decreased performance. We need to leave more effort to focus on input compatibility.

However, there does have some misleading output, like Bun version tags and brand output in syntax errors. It may confuse AI agents. This may be a trade-off that cannot be resolved in the short term.

### CLI 100% compatibility

Node.js CLI has some flags used to customize V8 engine behavior, and have some features which Bun is missing (like interactive inspect CLI). We only translate flags and options that we can find the corresponding ones.

For unsupported options, we determine whether to warn(ignore) or error(panic) based on the specific circumstances.

For example:

```
bunode: `node inspect` is not supported because Bun does not provide Node's built-in CLI debugger.
Use `node --inspect` / `node --inspect-brk` compatible flags instead.

tips: Bunode is a Node.js compatibility layer for Bun. Your using Node.js v26.3.0+bun.1.4.0 is actually based on Bun v1.4.0.
```

Different bun versions may also have different levels of Node.js compatibility, we change the detail behavior based on Bun's real version in codebase.
