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

- Rust 1.83 or later
- Git
- Familiarity with async Rust and the Tokio ecosystem

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

```bash
# Navigate to the workspace
cd spire

# Build the project
cargo build

# Build with all features
cargo build --all-features

# Run tests
cargo test --workspace --all-features

# Check formatting
cargo fmt --all -- --check

# Run clippy
cargo clippy --workspace --all-targets --all-features -- -D warnings
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

## Code Style

This project follows the standard Rust style guidelines:

- Run `cargo fmt` before committing
- Ensure `cargo clippy` passes with no warnings
- Add documentation comments (`///`) for public APIs
- Keep functions focused and reasonably sized
- Use meaningful variable and function names
- Maximum line length: 100 characters (enforced by rustfmt)

### Documentation

- Document all public APIs with doc comments
- Include examples in doc comments where appropriate
- Keep the README.md up to date with new features

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

- Rust version: [e.g. 1.83.0]
- OS: [e.g. Ubuntu 22.04]
- Spire version: [e.g. 0.2.0]

**Additional context** Any other relevant information.
```

## Feature Requests

We welcome feature requests! Please create an issue with:

- A clear description of the feature
- Use cases and benefits
- Any implementation ideas you have
- Whether you'd be willing to implement it

## Questions?

If you have questions about contributing, feel free to:

- Open a discussion on GitHub
- Ask in the issue tracker

## License

By contributing, you agree that your contributions will be licensed under the
MIT License.

## Recognition

Contributors will be recognized in the project's README and releases. Thank you
for making this project better!
