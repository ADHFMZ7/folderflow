# FolderFlow

A macOS app for automations that use agents. Choose a folder, describe what
should happen to each file added to it, and FolderFlow runs an agent on every
new file.

## Layout

```
src/          React + TypeScript UI, rendered by the system WebKit
src-tauri/    Rust core: window, menu bar, settings and workflows, and the
              engine that runs workflows (src-tauri/src/engine)
docs/         The api contract, the workflow format, step settings, and the engine
```

The UI only talks to the Rust core, through the `Api` interface in
`src/api/api.ts`; `docs/api-contract.md` lists the commands. How workflows run
is in `docs/engine.md`.

## Development

Requirements: Node 20+, Xcode command line tools, and rustup. The Rust
version is pinned in `rust-toolchain.toml`; rustup installs it on first build.

```sh
npm install
npm run tauri dev        # run the app with hot reload
npm test                 # the UI tests
cd src-tauri && cargo test   # the core's tests
```

Read `TESTING.md` before writing tests.
