# Bunode Project Guidance

Bunode is a Node.js compatible layer for Bun.

The project's final goal is to generate and release Node.js tarball, with `npm`, `npx`, and other ecosystem tools, while its runtime is actually Bun.

If you want to learn more about bunode's vision and purpose, please read README.md's `Why` part.

## Project Architecture

The project has mainly two different parts: Rust wrapper core and JavaScript tarball generator and mocked registry.

### Rust Core (Provide the ability to get Node.js wrapper tarball)

Rust core is a wrapper to underlying Bun, including options translation, environment variables injection, and CLI usage consistence.

Its source code won't be published on crates.io. Only for internal use.

### JavaScript Releasing (Release the Node.js compatible tarball)

- `@bunode/core`, the underlying library to generate Bun based Node.js-compatible tarball from a Bun tarball.
- `@bunode/registry`, the server with the same shape as `https://nodejs.org/dist`. Could be deployed on real servers.
- `@bunode/cli`, the local registry service installed on users' computers. Provide `bunode` cli to control the wrapper status.

`@bunode/registry` is based on `@bunode/core`, `@bunode/cli` is based on `@bunode/registry`.

## Rule

Vite+ is used as the project manager for JavaScript part. Use `vp install` to install dependencies, use `vp install -D` if the adden dependency can be bundled. Use `vp run` command to run commands in `package.json`. Vite+ is not the same as Vite, it includes Vitest (`vp test`), tsdown (`vp pack`), Oxlint (`vp lint`), Oxfmt (`vp fmt`) and task run, staged feature. Follow its document (node_modules/vite-plus/docs) to learn more. Do not add Vitest or tsdown separately unless Vite+ cannot provide the needed surface.

Use pnpm catalog for workspace package dependencies. Keep dependency versions in `pnpm-workspace.yaml`'s default catalog and use `catalog:` in package manifests.

Rust tasks are also defined in Vite+ (Vite Task), in project root's `vite.config.ts`.
Run `vpr check` (lint and format for both Rust and JavaScript) and `vpr test` after you make changes.

Core wrapper snap tests run through Vite+ and compare both plain and forced-color CLI output. Keep their steps declarative and schema-backed. They are defined in `tests/snap-tests/<bun-ver>/<project-name>`. If a test can be done in both Rust util test and snap tests, prefer snap tests.

Keep AGENTS.md updated with the project codebase. Consider if there is need to modify AGENTS.md after your changes. Don't store detail things like file structure or project implementation details in AGENTS.md.

Keep code functional. Never use classes. Write simple code and make function reusable if possible. Use Unix philosophy to design your code. For a function with multiple steps, use comment to divide, like `// 1. Do something first`, `// 2. Do something second`

Add `.gitkeep` file when creating new empty directory
