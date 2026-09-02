# Project Rules & Guidelines

- **Explicit Approval Required**: Never edit any code files or make changes without first explaining:
  1. **WHY** you are making the proposed modification.
  2. **WHAT EFFECT or FIX** it will produce.
  3. Receiving explicit user approval before modifying any code.

- **Rust Codebase Rules**: Follow all rules defined in `RULES.md`:
  - Hard limits: <400 lines/file (300 soft), <60 lines/fn (40 soft), max 4 params, max 3 nesting depth.
  - Zero tolerance: No `#[allow(dead_code)]`, no `unwrap()`/`expect()` in prod, no old `mod.rs` (use `foldername.rs`), 0 compiler warnings.
  - Role-based architecture: `domain/`, `infra/`, `api/`.
  - Readability, DRY, and clean checklist.
