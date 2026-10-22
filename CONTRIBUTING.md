# Contributing

Thank you for your interest in contributing to Spire! We appreciate your help in
making this project better.

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Getting Started](#getting-started)
- [Development Setup](#development-setup)
- [Making Changes](#making-changes)
- [Testing](#testing)
- [Code Style](#code-style)
- [Submitting Changes](#submitting-changes)
- [Reporting Bugs](#reporting-bugs)
- [Feature Requests](#feature-requests)

## Code of Conduct

This project follows the
[Rust Code of Conduct](https://www.rust-lang.org/policies/code-of-conduct). By
participating, you are expected to uphold this code.

## Getting Started

### Prerequisites

- Rust 1.95 or later, and a nightly toolchain for formatting
- Git
- Familiarity with async Rust and the Tokio ecosystem
- [cargo-deny](https://github.com/EmbarkStudios/cargo-deny) and
  [cargo-machete](https://github.com/bnjbvr/cargo-machete), for the security
  and dependency checks

```bash
rustup toolchain install nightly --component rustfmt
cargo install cargo-deny cargo-machete --locked
```

1. Fork the repository on GitHub
2. Clone your fork locally:
   ```bash
   git clone https://github.com/YOUR_USERNAME/spire.git
   cd spire
   ```
3. Add the upstream repository:
   ```bash
   git remote add upstream https://github.com/spire-rs/spire.git
   ```

## Development Setup

### Building the Project

The `Makefile` mirrors the CI checks:

```bash
# Build the project with all features
cargo build --all-features

# Check, format, clippy, tests, docs and machete, as CI runs them
make ci

# Fix the formatting with nightly rustfmt
make fmt

# Check advisories, licenses and sources with cargo-deny
make security

# Package and verify every publishable crate
make publish-dry-run
```

## Making Changes

1. Create a new branch for your changes:
   ```bash
   git checkout -b feature/your-feature-name
   ```

2. Make your changes following our [code style](#code-style)

3. Add tests for any new functionality

4. Ensure all tests pass:
   ```bash
   cargo test --workspace --all-features
   ```

5. Update documentation if needed

## Testing

- Write unit tests for new functionality
- Ensure all tests pass before submitting
- Add integration tests for complex features
- Test with both default and all features enabled:
  ```bash
  cargo test --workspace
  cargo test --workspace --all-features
  cargo test --workspace --no-default-features
  ```

### Benchmarks

The criterion benchmarks in each crate's `benches/` measure the hot paths:
the crawler's own overhead, frontiers, seeds and seen sets, the redb store,
routing, HTML extraction and sitemap parsing. Run them all, or filter by
name:

```bash
make bench
make bench ARGS=frontier
```

To check a change for regressions, save a baseline on `main` and compare
your branch against it:

```bash
make bench ARGS="--save-baseline main"
make bench ARGS="--baseline main"
```

CI compiles the benchmarks but does not run them: timings on shared runners
are too noisy to gate a merge. Reports are written to `target/criterion`.

## Code Style

This project follows the standard Rust style guidelines:

- Format with nightly rustfmt (`make fmt`), which applies the import grouping
  in `rustfmt.toml`
- Ensure `cargo clippy` passes with no warnings
- Add documentation comments (`///`) for public APIs
- Keep functions focused and reasonably sized
- Use meaningful variable and function names
- Maximum line length: 100 characters (enforced by rustfmt)

### Documentation

- Document all public APIs with doc comments
- Include examples in doc comments where appropriate
- Explain new features in [the book](book/src), and keep the READMEs short

The book is built with [mdBook](https://rust-lang.github.io/mdBook/), and
published to GitHub Pages from `main`. Preview it while writing:

```bash
cargo install mdbook
mdbook serve book --open
```

Its examples are doctests of the `spire` crate, so `make test` compiles them,
and runs those that need no network. Mark examples that crawl a real site
`no_run`, and hide boilerplate with `# ` lines, which both rustdoc and mdBook
hide.

## Submitting Changes

1. Commit your changes with clear, descriptive commit messages:
   ```bash
   git commit -m "Add feature: brief description"
   ```

2. Push your changes to your fork:
   ```bash
   git push origin feature/your-feature-name
   ```

3. Create a Pull Request on GitHub with:
   - A clear title describing the change
   - A detailed description of what changed and why
   - Reference to any related issues
   - Any breaking changes highlighted

### Pull Request Guidelines

- Keep PRs focused on a single feature or fix
- Update CHANGELOG.md with your changes
- Ensure CI passes (tests, formatting, clippy)
- Be responsive to review feedback
- Squash commits if requested

## Reporting Bugs

If you find a bug, please create an issue on GitHub with:

- A clear, descriptive title
- Steps to reproduce the issue
- Expected behavior
- Actual behavior
- Rust version and operating system
- Any relevant code snippets or error messages

### Bug Report Template

```markdown
**Describe the bug** A clear and concise description of the bug.

**To Reproduce** Steps to reproduce the behavior:

1. ...
2. ...

**Expected behavior** What you expected to happen.

**Actual behavior** What actually happened.

**Environment**

- Rust version: [e.g. 1.95.0]
- OS: [e.g. Ubuntu 22.04]
- Spire version: [e.g. 0.5.0]

**Additional context** Any other relevant information.
```

## Feature Requests

We welcome feature requests! Please create an issue with:

- A clear description of the feature
- Use cases and benefits
- Any implementation ideas you have
- Whether you'd be willing to implement it

## Questions?

If you have questions about contributing, feel free to ask in the issue
tracker.

## License

By contributing, you agree that your contributions will be licensed under the
MIT License.
