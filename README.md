# Open Circuit

Open Circuit is an open-source desktop application for drawing and simulating electrical circuits. The repository is a Cargo and pnpm monorepo. The Rust engine is kept independent of Tauri, while the desktop shell lives in `crates/app` and the React/TypeScript/PixiJS frontend lives in `src/`.

The current application is an initial desktop and canvas scaffold. The simulation engine is not connected to the UI yet.

## Prerequisites

- Git.
- Node.js **24.19.0** and pnpm **12.3.4**. The project pins both versions in `.node-version` and `package.json`.
- Rust **1.99.1** installed with rustup. Install rustup from [rustup.rs](https://rustup.rs); the root `rust-toolchain.toml` selects the pinned toolchain and its `rustfmt` and `clippy` components.
- Tauri's platform dependencies:
  - **Windows 10/11:** Microsoft C++ Build Tools with the **Desktop development with C++** workload. WebView2 is included on supported Windows 10 and Windows 11 installations.
  - **macOS:** Xcode Command Line Tools (`xcode-select --install`).
  - **Ubuntu/Debian:** install the WebKitGTK 4.1 and native build dependencies:

    ```sh
    sudo apt update
    sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file \
      libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev
    ```

See [Tauri's prerequisites guide](https://v2.tauri.app/start/prerequisites/) for current platform-specific setup details. The supported development and build targets are Windows, Linux, and macOS.

## Setup

Install the pinned pnpm version if it is not already available:

```sh
npm install --global pnpm@12.3.4
```

Install the repository dependencies from the lockfiles:

```sh
pnpm install --frozen-lockfile
```

## Run the desktop app

Start the Tauri desktop shell and Vite development server from the repository root:

```sh
pnpm dev
```

The root command starts the Tauri application defined in `crates/app`; no generated `src-tauri` project is used.

## Build and checks

Build the frontend and desktop application:

```sh
pnpm build
```

The current build produces a desktop executable without installer bundles; packaging is deferred.

Run the local verification commands:

```sh
pnpm format:check
pnpm lint
pnpm typecheck
pnpm test
pnpm types:check
pnpm boundaries:check
```

`pnpm types:generate` regenerates TypeScript bindings from the Rust engine types. `pnpm types:check` regenerates them and fails if the committed bindings are stale. `pnpm boundaries:check` fails if the engine crates depend on Tauri or other UI runtime crates, or if `src/editor` imports React.

## Continuous integration

The `CI` workflow (`.github/workflows/ci.yml`) runs on every pull request and on pushes to `main`, on `windows-latest`, `ubuntu-latest`, and `macos-latest`. Each job runs the commands above in this order, using the pinned toolchains and frozen lockfiles:

1. `pnpm install --frozen-lockfile`
2. `pnpm format:check` (`cargo fmt --check` and Prettier)
3. `pnpm lint` (`cargo clippy --workspace --all-targets -- -D warnings` and ESLint)
4. `pnpm typecheck`
5. `pnpm test` (`cargo test --workspace` and Vitest)
6. `pnpm types:generate`, then `git diff --exit-code -- src/generated` to detect stale generated types
7. `pnpm boundaries:check`
8. `pnpm build` (a Tauri build; nothing is published)

Run the same commands locally to reproduce a CI failure. The workflow needs no secrets. `.gitattributes` forces LF line endings so the formatting and generated-type checks give the same result on every OS, regardless of `core.autocrlf`.

## License

Copyright (C) 2026 SWOFART

This program is free software: you can redistribute it and/or modify it under
the terms of the GNU General Public License as published by the Free Software
Foundation, either version 3 of the License, or (at your option) any later
version. See [LICENSE](LICENSE) for the full text.

SPDX-License-Identifier: GPL-3.0-or-later
