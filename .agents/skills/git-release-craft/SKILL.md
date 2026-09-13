---
name: git-release-craft
description: >-
  Automates the complete Git release lifecycle: atomic conventional commits, semantic version
  bumps, rich aesthetic release descriptions with highlights and download snippets, tag creation,
  and mandatory active post-release verification (monitoring GitHub Actions CI/Release workflows
  until success and verifying attached release assets).
---

# `git-release-craft` Skill: Release Lifecycle & Workflow Verification

Automates production releases with aesthetic, well-formatted release notes and active verification of GitHub Actions CI/CD workflows and assets.

---

## 1. The Golden Rule: Never Fire-and-Forget

Creating a release is not finished when `git push` or `gh release create` exits. You must:
1. **Craft a Proper Description**: Never use blank releases or bare `--generate-notes`. Every release must follow the aesthetic highlight format.
2. **Actively Monitor Workflows**: Watch the triggered GitHub Actions CI/Release pipelines until completion.
3. **Verify Release Assets**: Confirm that compiled binaries, packages, or checksums are properly generated and attached.

---

## 2. Pre-Release Checklist

Before tagging or creating a release:

1. **Verify Local Quality**:
   - Rust: `cargo test --all-targets && cargo clippy --all-targets -- -D warnings && cargo fmt --check`
   - Python: `uv run pytest && uv run ruff check && uv run ruff format --check && uv run mypy .`
2. **Bump Semantic Version**:
   - Update `Cargo.toml`, `pyproject.toml`, or `package.json` to the target version `vX.Y.Z`.
3. **Commit Manifest Changes**:
   ```bash
   git add Cargo.toml Cargo.lock  # or pyproject.toml / uv.lock
   git commit -m "chore(release): bump to vX.Y.Z"
   ```

---

## 3. Aesthetic Release Note Template (Mandatory)

Every release must have a rich, beautifully structured description matching this format:

```markdown
# 🌸 <Project Name> vX.Y.Z ✨

<One-sentence punchy summary highlighting the theme or milestone of this release>

### 🌟 Key Highlights (or 🌸 What's New in vX.Y.Z)

- **• <Icon> <Feature / Fix Name>**: <Clear, concrete explanation of what changed and its user impact>.
- **• <Icon> <Feature / Fix Name>**: <Clear, concrete explanation of what changed and its user impact>.
- **• <Icon> <Feature / Fix Name>**: <Clear, concrete explanation of what changed and its user impact>.
- **• ✒️ Strict Codebase Quality**: <Mention code quality achievements: 100% tests passing, zero warnings, clean architecture>.

### 🚀 Direct Download & Run

```bash
<One-line curl installer, cargo install, or package manager command>
```
```

### Example Icons to Use:
- `✨` / `🌟` : Major new features
- `⚡` / `🚀` : Performance boosts, cloud offloading
- `📦` : Standalone binaries, asset bundling
- `🤫` / `🛡️` : Silent clutter-free logging, security, safety
- `🌸` / `🎨` : UI/TUI polish, aesthetics, styling
- `🐛` / `🔧` : Bug fixes, stability patches

---

## 4. Tagging & Publishing the Release

```bash
# 1. Create annotated tag locally
git tag -a vX.Y.Z -m "<Project Name> vX.Y.Z: <Short Summary>"

# 2. Push commit and tag to remote
git push origin <branch>
git push origin vX.Y.Z

# 3. Write release notes to a temporary file
cat << 'NOTES_EOF' > /tmp/release_notes.md
<Aesthetic Release Notes from Section 3>
NOTES_EOF

# 4. Create the official GitHub release
gh release create vX.Y.Z \
  --title "🌸 <Project Name> vX.Y.Z ✨" \
  --notes-file /tmp/release_notes.md

rm -f /tmp/release_notes.md
```

---

## 5. Mandatory Post-Release Workflow Verification

Immediately after publishing the release:

### Step 5.1: Retrieve Triggered Actions
```bash
gh run list --limit 4 --json databaseId,name,status,conclusion,headBranch,url
```

### Step 5.2: Watch and Verify Workflows
Wait for all active workflows (`Release`, `CI`, `Build`) to finish successfully:
```bash
# Watch the release workflow run
gh run watch <run_id>
```

Or poll the status if running in the background:
```bash
gh run view <run_id>
```

Confirm that all matrix jobs succeed (e.g., Linux x86_64, Linux ARM64, macOS Apple Silicon, macOS Intel).

### Step 5.3: Inspect Uploaded Assets
Confirm that release artifacts are physically attached to the release:
```bash
gh release view vX.Y.Z
```
Verify:
- Pre-compiled binaries/archives exist (e.g. `.tar.gz`, `.zip`, `.whl`).
- Checksums exist (e.g. `sha256:`).

---

## 6. Stale Tag & Release Cleanup (Hygiene)

When cleaning up older development or superseded tags:
```bash
# Delete GitHub release
gh release delete <tag> -y

# Delete remote git tag
git push origin --delete <tag>

# Delete local tag
git tag -d <tag>
```

---

## 7. Report Format to User

Always provide a verified release report:
1. Release tag & title link.
2. Summary of published highlights.
3. CI/CD workflow status (Jobs passed, run time).
4. List of uploaded artifacts with file sizes.
