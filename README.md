# FolderFlow

A macOS app for automations that use agents. Choose a folder, describe what
should happen to each file added to it, and FolderFlow runs an agent on every
new file.

## Layout

```
src/          React + TypeScript UI, rendered by the system WebKit
src-tauri/    Rust core: window, menu bar, file access, talks to the engine
engine/       Python engine: watches folders, queues jobs, runs the agents
```

The UI only talks to the Rust core (through `invoke()`), and the Rust core
runs the engine as a sidecar process. The engine can later move into Rust
without the UI changing.

## Development

Requirements: Node 20+, uv, Xcode command line tools, and rustup. The Rust
version is pinned in `rust-toolchain.toml`; rustup installs it on first build.

```sh
npm install
npm run tauri dev        # run the app with hot reload
```

The engine on its own:

```sh
cd engine
uv run folderflow-engine
```
