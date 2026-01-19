# Repository Guidelines

## Project Structure & Module Organization
This repo has two deliverables: a Tauri desktop app and a Cloudflare Worker. The desktop app lives in `desktop/` with the frontend in `desktop/src/` (TypeScript, CSS, HTML) and the Rust backend in `desktop/src-tauri/` (commands, audio, settings, `tauri.conf.json`, and `icons/`). The worker lives in `worker/` with code in `worker/src/` and deployment config in `worker/wrangler.toml`. Root scripts and docs include `deploy-worker.ps1`, `README.md`, and `LICENSE`.

## Build, Test, and Development Commands
- Desktop (run from `desktop/`):
  - `npm install`: install frontend dependencies.
  - `npm run dev`: start Vite frontend dev server.
  - `npm run build`: typecheck and bundle the frontend.
  - `npm run tauri dev`: run the desktop app in dev mode.
  - `npm run tauri build`: produce release bundles.
- Worker (run from `worker/`):
  - `npm install`: install worker dependencies.
  - `npm run dev`: local worker dev server (wrangler).
  - `npm run deploy`: deploy the worker.
  - `../deploy-worker.ps1`: Windows helper script for worker setup.

## Coding Style & Naming Conventions
- TypeScript is strict (`desktop/tsconfig.json`) with `noUnusedLocals` and `noUnusedParameters`; avoid unused variables and dead code.
- Match existing formatting per file: TS uses 4-space indentation; CSS/JSON use 2-space indentation.
- Rust uses standard naming: `snake_case` functions/modules, `CamelCase` types, and `SCREAMING_SNAKE_CASE` constants.
- Keep filenames descriptive and consistent (e.g., `settings.html`, `audio.rs`).

## Testing Guidelines
There is no test framework or `npm test` script configured yet, and no `tests/` directories. If you add Rust tests, place them under `desktop/src-tauri/tests` or inline with `#[cfg(test)]` and run `cargo test` from `desktop/src-tauri`. For frontend/worker tests, add scripts and document them here.

## Commit & Pull Request Guidelines
Commit history mostly follows Conventional Commit style (e.g., `feat: ...`), with a few legacy `init`/`Initial commit` entries. Use `type: summary` where possible (`feat`, `fix`, `chore`) and keep messages imperative. PRs should include a short summary, the commands run, linked issues, and screenshots for UI changes. Call out platform-specific behavior changes (Windows/macOS/Linux) and worker config updates.

## Configuration & Secrets
Worker API keys are managed via `wrangler` secrets and should not be committed. Desktop settings store the worker URL and API key; avoid hardcoding them in code or config files.
