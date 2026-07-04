# Bunode wrapper core (Rust)

We provide node-compatible API, and the binary will call underlying Bun.

The binary is a CLI based on clap, mock Node.js's behavior as much as possible.

`node` is the only expected filename for that binary.

## Calling underlying Bun

The underlying Bun binary's location is certain.

On Windows, it's always `./bun/bun.exe` resolved relative to the binary location.

On Linux and macOS, it's always `../bun/bun` resolved relative to the binary location.

It's mainly designed for future tarball releasing.

## Behavior

### Version control

To keep Bun's semver semantic, we use a hacky but effective way: add 100 to the Bun's own compatible-layer version.

For example: `bun v1.4.0` -> ` bun -e "console.log(process.version)" v26.3.0` `node v26.3.100` (Bunode)

So if developers define `package.json#node#engine` like `>=22.18.0` or `^22.18.0`, it will keep Bunode usable.

The version is injected in `node --version`, registry and other places needs version.

### `node` direct calls

We wrap `bun repl`. Replacing the first line's `Bun` and its version to `Node.js` and its version follows the instruction above.

We can't make the behavior 100% compatible but it is basically similar. Considering this feature is mainly for human to call, so I think it's not a big deal.

### `node [options] [ script.js ] [arguments]`

We wrap `bun run` for the script running.

Considering `bun run` can also trigger tasks in `package.json`, we prepend a `./` for pure script name (without any `/`).

For node options, we will try to translate them to buns or wrap as much as possible.

### Node options and

## Non Goal

### The completely same output

Bun's output system (error rendering, repl interactive logic) is quite different from Node.js's. It's difficult and meaningless to make the output completely the same. Most cases, output is for humans and agents to read.

We only modify misleading output (like repl's header, `node -v`). And leave more energy to focus on input compatibility.

### CLI 100% compatibility

Node.js CLI has some flags used to customize V8 engine behavior, and have some features which Bun is missing (like interactive inspect CLI). We only translate flags and options that we can find the corresponding ones.

For unsupported options, we determine whether to warn(ignore) or error(panic) based on the specific circumstances.

For example:

```
bunode: `node inspect` is not supported because Bun does not provide Node's built-in CLI debugger.
Use `node --inspect` / `node --inspect-brk` compatible flags instead.

tips: Bunode is a Node.js compatibility layer for Bun. Your using Node.js v26.3.100 is actually based on Bun v1.4.0.
```

Different bun versions may also have different levels of Node.js compatibility, we change the detail behavior based on Bun's real version in codebase.
