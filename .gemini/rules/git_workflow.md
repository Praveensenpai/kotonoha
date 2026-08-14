# Rule: Local Build First & Explicit Git Approval

1. **Local Compilation First**: Always compile and install the release binary locally to `~/.local/bin/kotonoha` using `cargo build --release && install -Dm 755 target/release/kotonoha ~/.local/bin/kotonoha` so the user can test changes locally first.
2. **Explicit Git Approval Required**: NEVER run `git add`, `git commit`, `git tag`, or `git push` automatically. Always ask the user if they would like to commit/tag/push, and only execute git commands when the user explicitly responds `yes`.
