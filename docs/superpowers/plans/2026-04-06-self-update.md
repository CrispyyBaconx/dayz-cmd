# Self-Update Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a launch-time self-update prompt backed by GitHub Releases and a release-published installer script, with a 5-second default-to-no timeout and restart after successful update.

**Architecture:** Keep update logic in small seams: a GitHub Releases client, an update workflow module, a startup prompt screen, and minimal app integration. Use test-first slices so parsing, countdown behavior, and process orchestration are covered before wiring the full flow.

**Tech Stack:** Rust, reqwest blocking client, ratatui, serde, std process/fs/env, cargo test

---

### Task 1: Release Metadata Client

**Files:**
- Create: `src/api/releases.rs`
- Modify: `src/api/mod.rs`
- Test: `src/api/releases.rs`

- [ ] **Step 1: Write the failing tests**

Add tests for parsing latest stable release metadata, ignoring prereleases/drafts, and selecting the `dayz-ctl-installer-linux.sh` asset URL.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test api::releases::tests::`
Expected: FAIL because the module does not exist yet.

- [ ] **Step 3: Write minimal implementation**

Implement the GitHub Releases client and pure parsing helpers with a narrow API for “is update available for current version?”.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test api::releases::tests::`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/api/releases.rs src/api/mod.rs
git commit -m "feat: add github release update client"
```

### Task 2: Update Workflow

**Files:**
- Create: `src/update.rs`
- Modify: `src/main.rs`
- Test: `src/update.rs`

- [ ] **Step 1: Write the failing tests**

Add tests for installer command construction, temp script handling boundaries, and restart command setup.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test update::tests::`
Expected: FAIL because the workflow module does not exist yet.

- [ ] **Step 3: Write minimal implementation**

Implement installer download, executable temp-file preparation, child-process invocation, and restart spawning helpers behind small testable functions.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test update::tests::`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/update.rs src/main.rs
git commit -m "feat: add script-assisted self-update workflow"
```

### Task 3: Startup Update Prompt

**Files:**
- Create: `src/ui/update_prompt.rs`
- Modify: `src/ui/mod.rs`
- Test: `src/ui/update_prompt.rs`

- [ ] **Step 1: Write the failing tests**

Add tests for default `No` selection, countdown expiry to dismiss, and `Yes/No` key handling.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test update_prompt::tests::`
Expected: FAIL because the screen does not exist yet.

- [ ] **Step 3: Write minimal implementation**

Implement a ratatui startup modal screen with a 5-second countdown and explicit result handling.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test update_prompt::tests::`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/ui/update_prompt.rs src/ui/mod.rs
git commit -m "feat: add startup self-update prompt"
```

### Task 4: App Integration

**Files:**
- Modify: `src/app.rs`
- Modify: `src/config.rs`
- Modify: `src/ui/main_menu.rs`
- Test: `src/app.rs`

- [ ] **Step 1: Write the failing tests**

Add focused tests for “newer release pushes prompt” and “timeout/default-no path continues startup”.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test app::tests::update`
Expected: FAIL because the startup flow does not integrate update state yet.

- [ ] **Step 3: Write minimal implementation**

Add session-scoped update-check state, startup prompt injection, status-message handling, and update execution/restart handoff.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test app::tests::update`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/app.rs src/config.rs src/ui/main_menu.rs
git commit -m "feat: wire startup self-update flow"
```

### Task 5: Final Verification

**Files:**
- Modify: `docs/superpowers/plans/2026-04-06-self-update.md`

- [ ] **Step 1: Run verification**

Run: `cargo test`
Expected: PASS

- [ ] **Step 2: Run lint verification**

Run: `cargo clippy --all-targets --all-features -- -D warnings`
Expected: PASS

- [ ] **Step 3: Mark plan progress**

Update this plan file checkboxes to reflect completed work.
