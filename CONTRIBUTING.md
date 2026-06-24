# Contributing to PRX Voice Engine

Thank you for your interest in contributing!

## Prerequisites

- Rust stable 1.85+ (edition 2024)
- macOS / Linux
- Speech models downloaded locally — see [MODELS.md](MODELS.md)

## Building

```bash
cargo build --workspace                  # Build all crates
cargo test --workspace                   # Run all tests
cargo run -p prx-voice-bin               # Run the server (defaults to :3000)
cargo clippy --workspace --all-targets -- -D warnings  # Lint
cargo fmt --check                        # Format check
```

## Code Standards

This project follows strict production-grade Rust standards:

1. **NO** `unwrap()`, `expect()`, `todo!()`, `unimplemented!()` in production code
2. **NO** dead code (unused variables, imports, parameters)
3. Explicit error handling with `thiserror` and the `?` operator
4. Use `parking_lot::Mutex` (sync) or `tokio::sync::Mutex` (async), not `std::sync::Mutex`
5. No blocking calls inside async tasks; spawned tasks must handle their own errors
6. Parameterized SQL only — never build queries with string formatting
7. Never log secrets; use structured tracing fields
8. Doc comments on all public items

## Pull Request Process

1. Fork the repo and create a feature branch
2. Make your changes following the code standards above
3. Ensure all tests pass: `cargo test --workspace`
4. Ensure no clippy warnings: `cargo clippy --workspace --all-targets -- -D warnings`
5. Ensure formatting: `cargo fmt`
6. Open a PR with a clear description answering:
   - What problem does this solve?
   - Does it change the session state machine, event schema, or API contract?
   - Does it change an adapter (ASR / Agent / TTS) interface?
   - How was it tested?

## Commit Messages

- Use English for commit messages
- Use conventional format: `feat:`, `fix:`, `refactor:`, `docs:`, `test:`, `ci:`
- Keep the first line under 72 characters

## License

By contributing, you agree that your contributions will be licensed under the
MIT license.
