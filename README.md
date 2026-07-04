# Bunode

Bunode is a Node.js compatibility layer for Bun.

> [!IMPORTANT]
> This project is under development actively. It's still working progress.

## Why

As a developer who loves cutting-edge tools in JavaScript ecosystem, [Bun](https://bun.sh/) interests me a lot. It is basically a JavaScript toolkit, including a bundler, a package manager, a test runner, and most importantly, a fast JavaScript runtime with lots of built-in modules.

That said, I don't want to adopt Bun's whole toolchain. I still prefer keep using my existing tools, `pnpm` for workspace management, and the [Vite](https://vite.dev/) ecosystem (including [Vitest](https://vitest.dev/), [Vite+](https://viteplus.dev/)). And all of them can run on top of Bun as the underlying runtime.

The problem is that Bun's CLI is designed to be all-in-one. Unlike the official Node.js CLI, it ships as a single `bun` binary rather than mirroring `node`, `npm` and `npx`. It gradually makes it apart from the existing Node.js ecosystem. For example, Node.js version-management tools (like Vite+'s `vp env`, Volta) can't recognize or manage Bun at all.

So this is why **Bunode** was born. It is a compatibility layer, wraps Bun and provides Node.js-compatible API.

## License

[MIT](./LICENSE) License - see [LICENSE](LICENSE) file for details.
