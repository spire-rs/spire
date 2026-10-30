# Makefile for spire

# Shell-level logger (expands to a printf that runs in the shell).
define log
printf "[%s] [MAKE] [$(MAKECMDGOALS)] $(1)\n" "$$(date '+%Y-%m-%d %H:%M:%S')"
endef

# CI Commands (mirror GitHub Actions)
.PHONY: ci
ci: ## Runs all CI checks locally (check, fmt, clippy, test, docs, machete).
	@$(call log,Running cargo check...)
	@cargo check --workspace --no-default-features
	@cargo check --workspace --all-features
	@$(call log,Checking code formatting...)
	@cargo +nightly fmt --all -- --check
	@$(call log,Running clippy...)
	@cargo clippy --workspace -- -D warnings
	@cargo clippy --workspace --all-features --all-targets -- -D warnings
	@$(call log,Running tests...)
	@cargo test --all-features --workspace
	@$(call log,Building documentation...)
	@RUSTDOCFLAGS="--cfg docsrs -D warnings" cargo +nightly doc --no-deps --all-features --workspace
	@$(call log,Checking for unused dependencies...)
	@cargo machete
	@$(call log,All CI checks passed!)

.PHONY: fmt
fmt: ## Fixes code formatting.
	@$(call log,Fixing code formatting...)
	@cargo +nightly fmt --all
	@$(call log,Formatting fixed!)

# Book checks (mirror the Book workflow)
.PHONY: book-check
book-check: ## Builds the book and checks local links, heading anchors and assets offline.
	@mdbook build book
	@set -eu; \
	book_root=$$(mktemp -d); \
	trap 'rm -rf "$$book_root"' EXIT; \
	ln -s "$(CURDIR)/book/book" "$$book_root/spire"; \
	lychee --offline --include-fragments --index-files index.html --root-dir "$$book_root" 'book/book/**/*.html'

# Benchmarks
.PHONY: bench
bench: ## Runs the criterion benchmarks; pass flags with ARGS, e.g. ARGS=crawler.
	@$(call log,Running benchmarks...)
	@cargo bench --workspace --all-features --bench '*' -- $(ARGS)
	@$(call log,Reports are in target/criterion.)

# Security Commands (mirror GitHub Actions)
.PHONY: security
security: ## Runs security checks locally (cargo deny).
	@$(call log,Running cargo deny...)
	@cargo deny check all
	@$(call log,All security checks passed!)

# Release Commands
.PHONY: publish-dry-run
publish-dry-run: ## Packages and verifies every publishable crate.
	@$(call log,Running cargo publish dry run...)
	@cargo publish --workspace --dry-run
	@$(call log,Publish dry run passed!)
