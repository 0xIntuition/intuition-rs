# Repository Guidelines

## Project Structure & Module Organization
- The CLI entry point lives at `src/main.rs`; it wires Tokio, Crossterm, and Ratatui.
- Core state management and data fetch routines sit in `src/app.rs`.
- Terminal views and widgets are in `src/ui/`.
- GraphQL queries and schema artifacts belong in `src/queries/`; keep `.graphql` documents and `schema.graphql` aligned.
- Example data flows are staged under `src/examples/`.
- Root metadata such as `Cargo.toml`, `Dockerfile`, and workspace docs stay in the repo root.

## Build, Test, and Development Commands
- `cargo check` catches compiler issues fast; run it before commits.
- `cargo fmt && cargo clippy --all-targets -- -D warnings` enforces style and lints.
- `cargo run` launches the TUI; add flags after `--` (e.g., `cargo run -- --help`).
- `cargo test` executes all unit and integration tests.
- Update Hasura metadata with `npx get-graphql-schema http://localhost:8080/v1/graphql > src/queries/schema.graphql`.

## Coding Style & Naming Conventions
- Follow Rust 2024 defaults: four-space indentation, `snake_case` functions, `PascalCase` types.
- Prefer module-level visibility (`pub(crate)`) unless the API is shared externally.
- Keep async handlers lightweight; push heavy logic into `App` helpers.
- Run `cargo fmt` before pushing; CI treats formatting drift as failures.

## Testing Guidelines
- Co-locate fast unit tests in the same file under `#[cfg(test)] mod tests`.
- Use `#[tokio::test]` for async scenarios that touch the network client.
- Mock GraphQL calls via lightweight fixtures; store reusable payloads in `src/examples/` when practical.
- Target meaningful coverage of fetch, transform, and render paths; avoid UI-only golden tests.

## Commit & Pull Request Guidelines
- Write commits in `<type>: <summary>` form (e.g., `feat: refresh atoms view`); keep summaries ≤72 chars.
- Reference tickets or PRs with `(#123)` when applicable.
- Squash noisy spikes before opening a PR; aim for reviewable, cohesive diffs.
- PRs should state intent, testing proof (`cargo test` / manual steps), and screenshots of new UI panels.
- Tag reviewers familiar with the touched domain (queries vs. UI) to accelerate merges.
