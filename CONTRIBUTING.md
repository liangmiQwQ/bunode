# Contributing

This project is built with JavaScript and Rust. You need to know some basic concept about Rust, Cargo and computer science.

This project uses Vite+ and `pnpm`, the united toolchain for the Web. We highly suggest you to install Vite+ global CLI: https://viteplus.dev/

## Setup

Install dependencies from the repository root:

```bash
vp install
# Or `pnpm install` if you didn't install Vite+ global CLI
```

## Validation

Run the full local validation from the repository root:

```bash
vpr check
# Or `npx vpr check` if you didn't install Vite+ global CLI
```

Run tests from the repository root:

```bash
vpr test
# Or `npx vpr test` if you didn't install Vite+ global CLI
```

For focused JavaScript-only checks, use `vpr ccheck` or `vpr ctest` directly. For focused Rust work, use the matching Cargo command directly while iterating, then finish with `vpr check` and `vpr test`.
