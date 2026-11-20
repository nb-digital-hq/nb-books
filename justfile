# NB Books: unified entry points.
#
# Every recipe delegates to the pnpm scripts and cargo commands that the
# guardians and the CI already run, so there is a single definition of each
# check. `just --list` prints this file's recipes with their descriptions.

set shell := ["bash", "-euo", "pipefail", "-c"]

tauri_dir := "src-tauri"

# List the available recipes.
default:
    @just --list

# Install JavaScript dependencies, Git hooks and the Playwright browser.
setup:
    pnpm install
    pnpm exec playwright install chromium
    pnpm pdfium

# Run the desktop application in development mode.
dev:
    pnpm tauri dev

# Run the web interface alone against the simulated bridge.
dev-web:
    pnpm dev

# Frontend unit tests (Vitest).
test-web:
    pnpm test

# Interface walkthroughs in a real browser (Playwright).
test-e2e:
    pnpm test:e2e

# Rust unit, integration and chaos tests.
test-rust:
    cd {{tauri_dir}} && cargo test

# Every test suite.
test: test-web test-e2e test-rust

# Formatting and lints, Rust warnings treated as errors.
lint:
    pnpm lint
    pnpm check
    cd {{tauri_dir}} && cargo fmt --all -- --check
    cd {{tauri_dir}} && cargo clippy --all-targets --all-features -- -D warnings

# Apply formatters.
fmt:
    pnpm format
    cd {{tauri_dir}} && cargo fmt --all

# Full definition of done: guardians, tests and Rust checks.
verify:
    pnpm verify
    cd {{tauri_dir}} && cargo fmt --all -- --check
    cd {{tauri_dir}} && cargo clippy --all-targets --all-features -- -D warnings
    cd {{tauri_dir}} && cargo test

# Privacy and supply-chain audit: network egress, log redaction, paths, licences.
audit:
    pnpm check:network
    pnpm check:registro
    pnpm check:rutas
    pnpm check:capacidades
    pnpm check:politica
    pnpm check:notices
    cd {{tauri_dir}} && cargo deny check

# Documentation integrity: broken links, empty or duplicated files, truncation.
docs:
    pnpm check:docs

# Regenerate the IPC bindings and the published extension schema.
bindings:
    pnpm check:bindings
    pnpm check:esquema

# Build the Linux packages (.deb, .rpm, AppImage and .tar.gz).
package-linux:
    NO_STRIP=1 pnpm empaquetar:linux

# Install the locally built .tar.gz for the current user.
install-local:
    pnpm instalar:local

# Write SHA256SUMS and the provenance record for a release folder.
provenance:
    pnpm procedencia

# Apply the GitHub protections that require a public repository (free plan):
# default-branch ruleset and private vulnerability reporting. Run once, right
# after the repository is made public.
github-public repo="nb-digital-hq/nb-books":
    gh api -X PUT repos/{{repo}}/private-vulnerability-reporting
    printf '%s' '{"name":"main","target":"branch","enforcement":"active","conditions":{"ref_name":{"include":["~DEFAULT_BRANCH"],"exclude":[]}},"rules":[{"type":"deletion"},{"type":"non_fast_forward"},{"type":"pull_request","parameters":{"required_approving_review_count":0,"dismiss_stale_reviews_on_push":true,"require_code_owner_review":false,"require_last_push_approval":false,"required_review_thread_resolution":true}}]}' | gh api -X POST repos/{{repo}}/rulesets --input -
