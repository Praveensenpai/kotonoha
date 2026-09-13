# Project Rules & Guidelines

## 1. Explicit Approval Protocol (Mandatory)
Never edit any code files or execute destructive modifications without first explaining:
1. **WHY**: The specific rationale and root cause for the proposed change.
2. **WHAT EFFECT or FIX**: The exact behavior, bug fix, or improvement it will produce.
3. **APPROVAL**: Await explicit user confirmation before applying any file modifications.

---

## 2. Language & Engineering Standards
All implementations must strictly adhere to the corresponding domain skills in Karakuri:

- **Rust Projects** (`skills/build-tooling/rust-clean-code/`):
  - **Hard Limits**: <400 lines/file (300 soft), <60 lines/fn (40 soft), max 4 parameters, max 3 nesting depth.
  - **Zero Tolerance**: No `#[allow(dead_code)]`, no `unwrap()`/`expect()` in prod, no old `mod.rs` (use modern `foldername.rs`), 0 compiler/clippy warnings.
  - **Role-Based Architecture**: `domain/`, `infra/`, `api/`.
  - **DRY & Idiomatic**: Centralize shared logic, use traits and guard clauses.

- **Python Projects** (`skills/build-tooling/python-clean-code/`):
  - **Tooling**: Exclusively managed via `uv` (no raw `pip`).
  - **Typing & Formatting**: 100% type annotations, automated `ruff check --fix`, import sorting (`isort`), and `ruff format` on every change.
  - **Zero Tolerance**: No unverified `# type: ignore` or `# noqa`.

- **Releases & Versioning** (`skills/build-tooling/git-release-craft/`):
  - **Release Notes**: Aesthetic highlight format with icons and direct install commands.
  - **Workflow Verification**: Actively track GitHub Actions CI/Release runs until green before declaring release complete.

- **Repository Management** (`skills/build-tooling/git-repo-craft/`):
  - **Descriptions**: Minimal yet meaningful (<90 chars), zero filler words, no redundant repo name prefix.
  - **Topics & Verification**: Mandatory 4–8 curated kebab-case topics across domain, language, purpose, and ecosystem, verified via `gh repo view`.

- **Documentation & README Craft** (`skills/build-tooling/aesthetic-readme-craft/`):
  - **Visual Identity**: Thematic emoji + title, bold value proposition, cohesive Shields.io badges (flat-square/for-the-badge).
  - **Show, Don't Just Tell**: Mandatory visual diagram (ASCII box flow or Mermaid chart), `🪄 One-Liner Magic` installer, and emoji-categorized feature matrix.

- **Bash & Shell Scripts** (`skills/system-ops/bash-clean-code/`):
  - **Preamble**: Mandatory `set -euo pipefail` and `IFS=$'\n\t'`.
  - **Zero Tolerance**: Zero ShellCheck warnings (`shellcheck -x`), mandatory trap cleanups for tempfiles, XDG compliance, strict variable quoting.
