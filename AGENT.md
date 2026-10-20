# Agent Guidelines

A game built with Bevy 0.20 (Rust). Bevy's API changes a lot between
releases: check the docs and migration guides for the version in
`Cargo.toml` instead of writing code from memory.

## General

- Keep it simple. Write straightforward code that follows nearby patterns.
  Build the smallest version that solves the actual request. No
  abstractions or extra validation for theoretical cases.
- Every code path must be reachable today. No configurable limits, generic
  frameworks or "future-proof" layers that aren't needed now.
- Before committing, read your diff and compare it to similar existing
  code. If a new module is much larger than its neighbours, cut it down.
- Format with `cargo fmt` (hard tabs, max width 80) before every commit.
  `cargo clippy` must have no warnings.
- Separate logical steps inside functions with blank lines.

## Bevy / game structure

- One Plugin per feature (player, camera, ui, audio, ...), each in its own
  module. `main.rs` only builds the App and adds plugins.
- Systems are small and do one thing; prefer several small systems over
  one big one. When order matters, make it explicit with sets or
  `.chain()`.
- Data lives in Components and Resources, behaviour in systems. No game
  logic in methods on big structs.
- Use States for game phases (loading, menu, playing, paused). Gate systems
  with `run_if(in_state(..))` and clean up entities in `OnExit`.
- Plugins talk to each other through events/messages or observers, not
  shared mutable resources.
- Tunable gameplay values (speeds, sizes, timings) are named consts or a
  resource at the top of the module, never inline magic numbers.
- Assets live in `assets/` and are loaded through the `AssetServer`, never
  with hard-coded absolute paths.
- Use `Time` (`time.delta_secs()`) for all movement and timers so nothing
  depends on frame rate.

## Tests

- Small, offline unit tests of pure logic, next to the code in
  `#[cfg(test)] mod tests`.
- For system tests, build a minimal App with only the plugins needed and
  call `app.update()`. No tests that need a window or GPU.

## Commits

- Short imperative subject in plain words, then a body saying what changed
  and why (a list for several changes).
- Only commit code that builds and passes `cargo test`.

## Building

- Local development: `cargo run --features dev`
- Release: `cargo build --release` (never with the `dev` feature)
