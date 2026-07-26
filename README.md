# Bible

A fast, offline-first Bible reader built with a Rust core and a Svelte UI, packaged
with [Tauri 2](https://tauri.app). Runs on desktop today, with iOS and Android
planned from the same codebase.

## Architecture

| Layer | Location | What it is |
|-------|----------|------------|
| Core | `core/` | Rust workspace crates — `domain`, `versification`, `import`, `storage`, `app` |
| Tools | `tools/` | `build-corpus` (compiles the scripture database), `build-annotations` |
| Frontend | `apps/ui/` | Svelte 5 + Vite |
| Shell | `apps/desktop/` | Tauri 2 application wrapper |
| Data | `data/` | Vendored USFM sources, manifests, versification schemes, annotations |

The compiled scripture database (`build/corpus.db`, ~166 MB) is **not** committed —
it is regenerated locally from the sources in `data/vendor/`. This keeps the repo
small and reproducible.

## Prerequisites

- **Rust** 1.82+ (`rustup`) — [install](https://rustup.rs)
- **Node.js** 20+ and npm — [install](https://nodejs.org)
- **Tauri CLI**: `cargo install tauri-cli`
- Tauri's platform dependencies (webview, build tools) — see the
  [Tauri prerequisites guide](https://tauri.app/start/prerequisites/)

## Build from source

```bash
# 1. Clone
git clone git@github.com:h3bzzz/bible-app.git
cd bible-app

# 2. Build the scripture database from the vendored sources.
#    Produces build/corpus.db, which the desktop app bundles as a resource.
cargo run --release -p build-corpus

# 3. Install frontend dependencies
npm --prefix apps/ui install

# 4a. Run in development (hot-reloading UI + native window)
cargo tauri dev

# 4b. …or produce a distributable bundle
cargo tauri build
```

> **Important:** run step 2 before `cargo tauri build`. The app bundles
> `build/corpus.db` as a resource (see `apps/desktop/tauri.conf.json`), so the
> build fails if the database has not been generated.

Add `--trigram` to step 2 (`cargo run --release -p build-corpus -- --trigram`) to
also build the substring search index.

## Tests

```bash
cargo test                    # Rust core + tools
npm --prefix apps/ui test     # Frontend unit tests
```

## Regenerating annotation layers (optional)

The messianic-prophecy annotation layer is committed under `data/annotations/`.
To rebuild it from its source reference pairs:

```bash
cargo run --release -p build-annotations
```

## Mobile (planned)

The Tauri 2 shell in `apps/desktop/` is the foundation for iOS and Android
targets. When the mobile projects are initialized (`cargo tauri ios init` /
`cargo tauri android init`), their project files are committed while build
outputs and signing material stay out of the repo — see `.gitignore`.

## License

AGPL-3.0-or-later. Vendored scripture texts under `data/vendor/` retain their
respective upstream licenses.

> A `LICENSE` file has not been added yet — drop in the official AGPL-3.0 text
> from <https://www.gnu.org/licenses/agpl-3.0.txt>.
