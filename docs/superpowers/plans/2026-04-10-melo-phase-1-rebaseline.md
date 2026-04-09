# Melo Phase 1 Rebaseline Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Converge the current phase-1 skeleton into the architecture promised by the spec by fully landing the core runtime dependencies, replacing the temporary persistence path, and finishing the CLI help surface.

**Architecture:** Treat the existing implementation as a functional skeleton, not the finished phase-1 target. Rebaseline from that state by converting persistence to `SeaORM`, wiring real metadata and playback adapters, completing the TUI/daemon communication path, and tightening the CLI help contract. Keep the already-shipped tests and behaviors where possible, but replace placeholder implementations with production ones in small, verifiable slices.

**Tech Stack:** Rust 2024, SeaORM, SeaORM Migration, Tokio, Axum, Tower HTTP, Tokio Tungstenite, Rodio, Lofty, Clap, Ratatui, Crossterm, Unicode Width, Mime Guess, Tracing.

---

## Context

This plan starts from the current repository state after the first implementation sweep:

- phase-1 skeleton code already exists
- the original plan is preserved in [2026-04-09-melo-phase-1.md](c:/home/Projects/rust/melo/docs/superpowers/plans/2026-04-09-melo-phase-1.md)
- the updated dependency requirements live in [2026-04-09-melo-design.md](c:/home/Projects/rust/melo/docs/superpowers/specs/2026-04-09-melo-design.md)

This rebaseline plan only covers the remaining convergence work.

## Rebaseline targets

The following dependencies must be truly used by the end of this plan:

- `sea-orm`
- `sea-orm-migration`
- `lofty`
- `rodio`
- `tokio-tungstenite`
- `tower-http`
- `unicode-width`
- `mime_guess`
- `tracing`
- `tracing-subscriber`

The following remain explicitly non-blocking for phase 1:

- `inquire`
- `utoipa`

## File structure impact

### Existing files that will be heavily reworked

- Modify: `Cargo.toml`
- Modify: `src/main.rs`
- Modify: `src/core/config/settings.rs`
- Modify: `src/core/db/bootstrap.rs`
- Modify: `src/core/db/connection.rs`
- Modify: `src/core/db/maintenance.rs`
- Modify: `src/domain/library/metadata.rs`
- Modify: `src/domain/library/repository.rs`
- Modify: `src/domain/library/service.rs`
- Modify: `src/domain/player/backend.rs`
- Modify: `src/domain/player/service.rs`
- Modify: `src/daemon/app.rs`
- Modify: `src/daemon/server.rs`
- Modify: `src/cli/args.rs`
- Modify: `src/cli/run.rs`
- Modify: `src/cli/client.rs`
- Modify: `src/tui/app.rs`
- Modify: `src/tui/mod.rs`
- Modify: `src/test_support.rs`

### New files expected in this convergence pass

- Create: `src/core/db/entities/mod.rs`
- Create: `src/core/db/entities/artists.rs`
- Create: `src/core/db/entities/albums.rs`
- Create: `src/core/db/entities/songs.rs`
- Create: `src/core/db/entities/playlists.rs`
- Create: `src/core/db/entities/playlist_entries.rs`
- Create: `src/core/db/entities/artwork_refs.rs`
- Create: `src/core/db/migrator.rs`
- Create: `src/core/db/migrations/mod.rs`
- Create: `src/core/db/migrations/m20260410_000001_initial.rs`
- Create: `src/domain/library/lofty_reader.rs`
- Create: `src/domain/player/rodio_backend.rs`
- Create: `src/tui/client.rs`
- Create: `src/tui/ws_client.rs`
- Create: `src/tui/theme.rs`
- Create: `src/tui/ui/mod.rs`
- Create: `src/tui/ui/layout.rs`
- Create: `src/tui/ui/sidebar.rs`
- Create: `src/tui/ui/content.rs`
- Create: `src/tui/ui/playbar.rs`
- Create: `src/tui/ui/popup.rs`

### Tests to update or add

- Modify: `tests/db_bootstrap.rs`
- Modify: `tests/library_scan.rs`
- Modify: `tests/player_service.rs`
- Modify: `tests/api_server.rs`
- Modify: `tests/cli_help.rs`
- Modify: `tests/cli_remote.rs`
- Modify: `tests/tui_app.rs`
- Create: `tests/seaorm_repository.rs`
- Create: `tests/lofty_reader.rs`

---

### Task A: Migrate persistence from `rusqlite` to `SeaORM`

**Dependencies landed by this task:**

- `sea-orm`
- `sea-orm-migration`

**Files:**

- Modify: `Cargo.toml`
- Modify: `src/core/db/bootstrap.rs`
- Modify: `src/core/db/connection.rs`
- Modify: `src/core/db/maintenance.rs`
- Modify: `src/domain/library/repository.rs`
- Modify: `src/domain/playlist/repository.rs`
- Modify: `src/test_support.rs`
- Create: `src/core/db/entities/mod.rs`
- Create: `src/core/db/entities/artists.rs`
- Create: `src/core/db/entities/albums.rs`
- Create: `src/core/db/entities/songs.rs`
- Create: `src/core/db/entities/playlists.rs`
- Create: `src/core/db/entities/playlist_entries.rs`
- Create: `src/core/db/entities/artwork_refs.rs`
- Create: `src/core/db/migrator.rs`
- Create: `src/core/db/migrations/mod.rs`
- Create: `src/core/db/migrations/m20260410_000001_initial.rs`
- Test: `tests/db_bootstrap.rs`
- Test: `tests/seaorm_repository.rs`

**Steps:**

- [ ] Update the old plan status note to reference this rebaseline plan
- [ ] Write a failing integration test that initializes the database through the new `Migrator` and asserts the schema exists without calling any `rusqlite` bootstrap helper directly
- [ ] Run that test and confirm it fails because the `SeaORM` migration path is missing
- [ ] Add SeaORM entities and an initial migration matching the current schema
- [ ] Replace `connect()` so production repositories receive a SeaORM database connection instead of a `rusqlite::Connection`
- [ ] Convert library and playlist repositories to use SeaORM queries for create/list/update paths
- [ ] Restrict `rusqlite` usage to tests and the standalone beets migration script only
- [ ] Run `cargo test --test db_bootstrap -- --nocapture`
- [ ] Run `cargo test --test seaorm_repository -- --nocapture`
- [ ] Commit with `refactor: converge persistence on seaorm`

---

### Task B: Replace the placeholder metadata reader with a real `Lofty` implementation

**Dependencies landed by this task:**

- `lofty`
- `mime_guess`

**Files:**

- Modify: `src/domain/library/metadata.rs`
- Modify: `src/domain/library/service.rs`
- Modify: `src/domain/library/assets.rs`
- Create: `src/domain/library/lofty_reader.rs`
- Test: `tests/library_scan.rs`
- Test: `tests/lofty_reader.rs`

**Steps:**

- [ ] Write a failing test for `LoftyMetadataReader` using a small tagged fixture file and assert title/lyrics/format extraction
- [ ] Run the new `lofty_reader` test and confirm the failure comes from the missing concrete reader implementation
- [ ] Implement `LoftyMetadataReader` and move sidecar/embedded precedence resolution into the scan service using the configured priority order
- [ ] Use `mime_guess` when storing or returning artwork MIME values
- [ ] Replace any remaining placeholder reader references in production code
- [ ] Run `cargo test --test lofty_reader -- --nocapture`
- [ ] Run `cargo test --test library_scan -- --nocapture`
- [ ] Commit with `feat: wire library scan to lofty metadata reader`

---

### Task C: Replace the fake playback path with a real `Rodio` backend

**Dependencies landed by this task:**

- `rodio`

**Files:**

- Modify: `src/domain/player/backend.rs`
- Modify: `src/domain/player/service.rs`
- Create: `src/domain/player/rodio_backend.rs`
- Test: `tests/player_service.rs`

**Steps:**

- [ ] Write a failing test that proves `PlayerService` can still be exercised against a fake backend while the production backend type becomes `RodioBackend`
- [ ] Run the player test and verify the failure is caused by the missing real backend integration
- [ ] Implement `RodioBackend` behind the existing `PlaybackBackend` trait
- [ ] Update daemon app wiring so production state uses `RodioBackend`, while tests continue to use a fake or noop backend
- [ ] Run `cargo test --test player_service -- --nocapture`
- [ ] Commit with `feat: wire player service to rodio backend`

---

### Task D: Complete daemon middleware, logging, and Rust WebSocket client wiring

**Dependencies landed by this task:**

- `tokio-tungstenite`
- `tower-http`
- `tracing`
- `tracing-subscriber`

**Files:**

- Modify: `src/main.rs`
- Modify: `src/daemon/app.rs`
- Modify: `src/daemon/server.rs`
- Modify: `src/api/player.rs`
- Create: `src/api/ws.rs`
- Create: `src/tui/client.rs`
- Create: `src/tui/ws_client.rs`
- Test: `tests/api_server.rs`
- Test: `tests/cli_remote.rs`

**Steps:**

- [ ] Write a failing test that checks the daemon router exposes the WebSocket route and that the TUI client can consume a pushed player snapshot
- [ ] Run the API/TUI communication tests and verify the failure is due to the missing WebSocket path or client
- [ ] Add `tower-http` trace middleware to the daemon router
- [ ] Initialize `tracing-subscriber` in the binary entry point with environment-based filtering
- [ ] Implement the daemon WebSocket endpoint and the TUI Rust client based on `tokio-tungstenite`
- [ ] Run `cargo test --test api_server -- --nocapture`
- [ ] Run `cargo test --test cli_remote -- --nocapture`
- [ ] Commit with `feat: add websocket client path and daemon middleware`

---

### Task E: Make the TUI use real layout/render code and correct CJK widths

**Dependencies landed by this task:**

- `unicode-width`
- `ratatui` (formalized, not just linked transitively)

**Files:**

- Modify: `src/tui/app.rs`
- Modify: `src/tui/mod.rs`
- Create: `src/tui/theme.rs`
- Create: `src/tui/ui/mod.rs`
- Create: `src/tui/ui/layout.rs`
- Create: `src/tui/ui/sidebar.rs`
- Create: `src/tui/ui/content.rs`
- Create: `src/tui/ui/playbar.rs`
- Create: `src/tui/ui/popup.rs`
- Test: `tests/tui_app.rs`

**Steps:**

- [ ] Write a failing TUI rendering-focused test that checks CJK titles do not break width calculations in list rows
- [ ] Run the TUI tests and verify the failure is tied to missing width-aware rendering helpers
- [ ] Introduce a real TUI layout and rendering layer
- [ ] Use `unicode-width` in text measurement helpers for song titles and related columns
- [ ] Keep the current keyboard-action behavior green while moving from state-only TUI code to UI-backed code
- [ ] Run `cargo test --test tui_app -- --nocapture`
- [ ] Commit with `feat: formalize tui layout and unicode width handling`

---

### Task F: Complete CLI help documentation and examples

**Dependencies landed by this task:**

- No new dependencies; this task formalizes the `clap` help surface promised by the spec

**Files:**

- Modify: `src/cli/args.rs`
- Test: `tests/cli_help.rs`

**Steps:**

- [ ] Write failing help-output tests for:
  - `melo --help`
  - `melo library --help`
  - `melo playlist --help`
  - `melo db --help`
- [ ] Verify they fail because `about`, `long_about`, and example text are missing
- [ ] Add descriptive `about` text for top-level and first-level commands
- [ ] Add `long_about` text and examples for commands with tricky boundaries, especially `queue`, `playlist`, and `organize`
- [ ] Run `cargo test --test cli_help -- --nocapture`
- [ ] Commit with `docs: expand CLI help and command examples`

---

### Task G: Final convergence verification and dependency sanity pass

**Dependencies landed by this task:**

- Confirms the required dependency matrix rather than adding new crates

**Files:**

- Modify: `Cargo.toml`
- Modify: `docs/superpowers/plans/2026-04-09-melo-phase-1.md`
- Test: all existing test suites

**Steps:**

- [ ] Update the old phase-1 plan so completed tasks remain checked and partially completed tasks reference this rebaseline plan
- [ ] Review `Cargo.toml` and confirm that every phase-1-mandatory dependency is actually used in production code
- [ ] Confirm that non-mandatory dependencies (`inquire`, `utoipa`) remain explicitly optional for later work
- [ ] Run `cargo test --tests -- --nocapture`
- [ ] Run `cargo check`
- [ ] Run `python -m unittest scripts.tests.test_beets_to_melo -v`
- [ ] Commit with `chore: complete phase 1 dependency convergence`

---

## Success criteria

This rebaseline plan is complete when all of the following are true:

- production persistence uses `SeaORM`
- migrations use `sea-orm-migration`
- production metadata reading uses `Lofty`
- production playback uses `Rodio`
- TUI Rust-side daemon communication uses `tokio-tungstenite`
- daemon middleware/logging uses `tower-http` + `tracing`
- TUI width handling uses `unicode-width`
- sidecar MIME inference uses `mime_guess`
- CLI help output clearly documents command boundaries and examples
- the original phase-1 plan is preserved as history, but no longer acts as the active execution source
