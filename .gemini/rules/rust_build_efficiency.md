# Rule: Fast Cached Rust Release Builds

1. **Do NOT use `cargo install` for iteration**: `cargo install` isolates build artifacts and recompiles all 240+ dependencies from scratch every run (~3 mins).
2. **Use Cached Release Build**: Always run:
   ```bash
   cargo build --release && cp target/release/kotonoha ~/.local/bin/kotonoha
   ```
   This reuses `./target/release/` cache so code updates compile in **~3 seconds**.
