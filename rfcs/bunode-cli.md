# @bunode/cli JS package wrapper & Rust binary

The `@bunode/cli` provides ability to let users use the bunode core binary `node`, by modifying or cloning the current using Node.js, and managing all of them.

Considering the runtime version of global-install packages are fixed for some Node.js version managers, we aren't able to make the CLI tool in Node.js. We divide it into two parts, the first part is a small Node.js wrapper, based on `free-shellrc`, and the another part is the core `bunode` native binary built with Rust, it can be launch without any Node.js.

The JavaScript part only handles the native `bunode`'s storage, uses NPM's optional dependency, prepend the `bunode`'s directory to system PATH, managing with `free-shellrc`. It should only be called for one time (the first time for each shell), and for other times, the command should always passthrough to the bunode native binary.

The reason we design in this way is to make sure users can use bunode through their familiar Node.js (npm ecosystem), and keep bunode usable even when their Node.js / Bunode is broken.

## JavaScript part details

We use `picocolors` to get our output colored. Even if the JavaScript part is just a wrapper and small, its output should also get formatted.

For calling `free-shellrc`, we only inject shellrc for the current using shell. It means that for every time users switch to a new shell, the JavaScript part should will still be run, and generate shellrc integration for this shell.

## Integration

We use a `napi-rs`-like style, and npm's optional dependencies to release the native Bunode cli and bunode(`node`) binary. They should be put in one package, and all packages' versions should be the same.

The bunode cli's directory is under `~/.bunode`, and `~/.bunode/bin` will be in `PATH` (shellrc integration). The only thing in it is a executable script (`.sh` on Unix, `.cmd` and `.ps1` on Windows), it calls bunode JavaScript cli wrapper, which hardlinks (or failback to copy) the real native bunode binary to `~/.bunode/bunode[.extension]`, and finally call the real native bunode cli.

If the CLI itself goes inaccessible, like exit with non-zero code, we print a warning but don't block users' actions.

## Core features

The CLI's features will be all implemented in Rust. It's related data should be put inside `~/.bunode`.

### `bunode patch <version> [node-prefix] [--options]`

The core function of bunode cli. It is used to generate Bunode-managed prefixes.

If the `node-prefix` is missing, it will call `node -p "process.execPath"` to get the current using Node.js prefix.

The flow:

1. Check the given Node.js prefix, make sure it is a real Node.js prefix (like refusing symlink, directory, ensurethe Node is really here). Print an error message and exit if it is not a Node.js prefix, or if it is already a Bunode prefix. Record the Node.js version for future use (by running `node -p "process.version").

2. Download the bun binary from npm according to user's given Bun version, print an error message and exit if downloading is failed or the given Bun version does not exist.

3. Check the Bun's masqueraded Node.js by running `bun -p "process.version"`, if the masqueraded is not the same as Node.js prefix's version, prompt users whether to continue.

4. Check the Node.js prefix's path, if its path contains the original Node.js version, or it is like a shape managed by `fnm`, `Vite+` or other Node.js version manager, prompt users whether to turn the given Node.js prefix into a Bunode prefix or copy into another prefix (use new prefix's `node -v` output, printed by bunode core binary); if not, just modify the given prefix.

5. Apply the change, copy the Bunode binary to there, rename the original Node.js bin to `<prefix-root>/bun/node.old[.extension]` Record this change in a file of `~/.bunode`.

For every step's prompt, we provide an option to skip the prompt. Warn if an given option is unused.

### `bunode revert [bunode-prefix]`

Revert a Bunode prefix back to node prefix, or just simply delete it if it is generated.

If the `bunode-prefix` is missing, it will call `node -p "process.execPath"` to get the current using Bunode prefix. If it is not a Bunode prefix, exit and print an error message.

Prompt users to confirm whether to revert the Bunode's change. There are two styles of prompt based on the prefix type (modified or copied) and its influence (delete the Node.js version or other).

### `bunode list`

List the recorded Bunode prefixes. In the output, it should contain the Node's version (`node -v`, printed by Bunode), Bunode's version(bunode itself), original Node.js version and Bunode prefix's path.

### `bunode implode`

Revert all recorded Bunode prefixes back to normal Node prefix.

List all the Bunode prefixes first and then prompt users to confirm.

### `bunode update`

Regenerate recorded Bunode prefixes which are using a different Bunode version from the current using one. Test `node -e "if (1 == 1) { process.exit(0); }"` to confirm everything goes well. If found a prefix get broken after the re-generation, revert the refresh and print a warn.
