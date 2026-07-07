# `@bunode/core` The Node.js tarball generator

The package is released on npm as a library. It's used to generate Bunode tarball.

The tarball should just look like a Node.js tarball, but with a little more bun related files, and replace the `node` (`node.exe`) binary to bunode's binary.

## Interface

We export a `generate` function, it receives a Bun's version, and it is to generate the tarball.

Fake code:

```typescript
async function generate(version: string): Promise<BunodeArtifact>
```

## The flow

1. Download the Bun tarball from the GitHub release (https://github.com/oven-sh/bun/releases/download/bun-v(bun-version)/(filename).zip)
2. Unzip the bun tarball, get the bun binary
3. Use the Bun binary to get the masqueraded Node.js version
4. Fetch the corresponding Node.js tarball from https://nodejs.org/dist
5. Resolve the Node.js tarball
6. Replace the Node with the Bundoe binary, put the Bun binary into bun directory
7. Generate tarball and return

## Bunode Binary Releasing

We borrow napi-rs's idea, we use optional dependency which includes the bunode binary to have the bunode binary. The version of bunode binary should be the same as the cli's.

It requires we have a simple releasing logic and a releasing script on CI. (`@bunode/binary-....`)

## Future Plan

1. Avoid downloading the whole Node.js tarball, we can use the masqueraded Node.js version to know npm's version and corepack's version (`<=25.0.0`)
2. Avoid to get masqueraded Node.js version from `bun`'s output, fetch and query from the website manifest instead.
3. Add way to let caller customize the registry
