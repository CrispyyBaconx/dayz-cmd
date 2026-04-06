# Launch Parity Steamworks-Only Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the remaining gameplay and launch-parity gaps using a Steamworks-only architecture: unknown direct-connect setup, `vm.max_map_count` gating, a Steamworks-backed installed-mod refresh action, and the user-facing offline-mode flow.

**Architecture:** Introduce a shared app-owned launch-prep state so launch paths stop branching in UI screens. Build the remaining features on top of that seam in this order: shared prep, unknown direct connect, sysctl gate, mod refresh, offline backend launch helpers, then offline browser/setup/app wiring.

**Tech Stack:** Rust, ratatui, serde, std fs/process, Steamworks, cargo test

---

### Task 1: Shared Launch Prep Foundation

**Files:**
- Modify: `src/app.rs`
- Modify: `src/ui/mod.rs`
- Modify: `src/launch.rs`
- Test: `src/app.rs`

- [ ] **Step 1: Write the failing tests**

Add app-level tests that prove launch input is app-owned rather than screen-owned:

- known server launch can read target state from a shared pending launch record
- direct connect launch can read `ip`, `port`, selected mods, and password from the same shared record
- launch consumes one-shot password/prep state after building args

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test app::tests::`
Expected: FAIL because launch state is still split across `selected_server`, `direct_connect_target`, and ad hoc prompt fields.

- [ ] **Step 3: Write minimal implementation**

Add a small app-owned launch-prep type and route launch building through it. Keep the shape narrow:

```rust
enum LaunchTarget {
    KnownServer(usize),
    DirectConnect { ip: String, port: u16 },
    Offline { mission_id: String },
}

struct LaunchPrep {
    target: LaunchTarget,
    mod_ids: Vec<u64>,
    password: Option<String>,
    offline_spawn_enabled: Option<bool>,
}
```

Update `App::do_launch()` so it resolves the current launch target from this structure instead of reading multiple partially overlapping fields.
Offline launch data flow must stay explicit:

- `OfflineBrowserScreen` chooses `mission_id`
- `OfflineSetupScreen` loads/saves remembered values in `Profile.offline`
- the setup screen copies `mission_id`, selected `mod_ids`, and the current
  `spawn_enabled` value into `LaunchPrep`
- the offline launch helpers consume those values without inventing new hidden
  app state

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test app::tests::`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/app.rs src/ui/mod.rs src/launch.rs
git commit -m "refactor: unify launch preparation state"
```

### Task 2: Unknown Direct Connect Setup

**Files:**
- Create: `src/ui/direct_connect_setup.rs`
- Modify: `src/ui/direct_connect.rs`
- Modify: `src/ui/password_prompt.rs`
- Modify: `src/ui/mod.rs`
- Modify: `src/app.rs`
- Modify: `src/launch.rs`
- Test: `src/ui/direct_connect.rs`
- Test: `src/ui/direct_connect_setup.rs`
- Test: `src/app.rs`
- Test: `src/launch.rs`

- [ ] **Step 1: Write the failing tests**

Add tests covering:

- unknown direct connect does not launch immediately
- it pushes a setup screen instead
- the setup screen can toggle installed mod IDs from `mods_db`
- the setup screen can store an optional password in shared launch prep by
  reusing the existing password prompt flow in `src/ui/password_prompt.rs`
- confirming setup launches with `-connect`, `-port`, optional `-password`, and `-mod=@...`

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test direct_connect`
Expected: FAIL because unknown targets still jump straight into `Action::LaunchGame`.

- [ ] **Step 3: Write minimal implementation**

Create `DirectConnectSetupScreen` that owns only input collection:

```rust
struct DirectConnectSetupScreen {
    selected_mod_ids: Vec<u64>,
    password: Option<String>,
}
```

Change `DirectConnectScreen` so:

- known cached server targets still route to normal known-server launch
- unknown targets populate shared launch prep and push `ScreenId::DirectConnectSetup`

Use local installed mods only. Do not add any remote lookup.
Reuse the existing `PasswordPromptScreen` rather than creating a second password
entry widget.
Add an explicit launch-arg seam for direct-connect mods so the implementation
does not need ad hoc string assembly in UI code, for example by extending
`build_direct_connect_args(...)` to accept selected mod IDs directly.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test direct_connect`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/ui/direct_connect.rs src/ui/direct_connect_setup.rs src/ui/password_prompt.rs src/ui/mod.rs src/app.rs
git commit -m "feat: add unknown direct connect setup flow"
```

### Task 3: `vm.max_map_count` Startup Gate

**Files:**
- Create: `src/ui/info_screen.rs`
- Modify: `src/config.rs`
- Modify: `src/ui/mod.rs`
- Modify: `src/ui/popup.rs`
- Modify: `src/main.rs`
- Modify: `src/app.rs`
- Test: `src/config.rs`
- Test: `src/app.rs`
- Test: `src/ui/popup.rs`

- [ ] **Step 1: Write the failing tests**

Add tests for:

- parsing and comparing current `vm.max_map_count`
- building the exact manual commands shown by the original launcher
- startup pushing a confirm screen when the value is below `1048576`
- the `connect` CLI path refusing to bypass the same gate
- declining the prompt surfacing the commands and exiting
- confirming the prompt executing the fix path and exiting on failure

- [ ] **Step 2: Run test to verify it fails**

Run:

- `cargo test config::tests::`
- `cargo test app::tests::`
- `cargo test ui::popup::tests::`

Expected: FAIL because no sysctl helpers or startup gate exist.

- [ ] **Step 3: Write minimal implementation**

Add focused helpers in `src/config.rs`:

```rust
pub const REQUIRED_MAX_MAP_COUNT: u64 = 1_048_576;

enum MaxMapCountState {
    Ready(u64),
    NeedsFix(u64),
    UnsupportedPlatform,
}

pub fn current_max_map_count_state() -> Result<MaxMapCountState>;
pub fn max_map_count_commands() -> [String; 2];
pub fn fix_max_map_count() -> Result<()>;
```

Use `ConfirmAction::FixMaxMapCount` in `ConfirmScreen`. On decline, push an `InfoScreen` that shows the two commands, then exit. On confirm, run:

```bash
echo "vm.max_map_count=1048576" | sudo tee /etc/sysctl.d/50-dayz.conf
sudo sysctl -w vm.max_map_count=1048576
```

Do not continue into the launcher when the gate fails or is declined.
Apply the same gate to the CLI `connect` path in `src/main.rs` before it reaches
`run_direct_connect()`.
Treat unsupported-platform fallback explicitly through the helper contract:

- `UnsupportedPlatform` means `/proc/sys/vm/max_map_count` is absent and the app
  continues
- `NeedsFix(current)` means the value is present and below the required minimum
- `Ready(current)` means the gate is satisfied
- `Err(...)` means the file was expected but could not be read or parsed, which
  is a blocking startup error on Linux

- [ ] **Step 4: Run test to verify it passes**

Run:

- `cargo test config::tests::`
- `cargo test app::tests::`
- `cargo test ui::popup::tests::`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/config.rs src/ui/info_screen.rs src/ui/mod.rs src/ui/popup.rs src/main.rs src/app.rs
git commit -m "feat: gate startup on vm.max_map_count"
```

### Task 4: Refresh Installed Mods

**Files:**
- Modify: `src/ui/config_screen.rs`
- Modify: `src/ui/mod.rs`
- Modify: `src/app.rs`
- Modify: `src/mods/manager.rs`
- Test: `src/ui/config_screen.rs`
- Test: `src/app.rs`
- Test: `src/mods/manager.rs`

- [ ] **Step 1: Write the failing tests**

Add tests covering:

- deriving installed Workshop IDs from `mods_db`
- config action routing to a dedicated `Action::RefreshInstalledMods`
- empty mod database returns a clear no-op status
- missing Steam handle returns a clear no-op status
- successful refresh path queues all installed Workshop IDs

- [ ] **Step 2: Run test to verify it fails**

Run:

- `cargo test mods::manager::tests::`
- `cargo test ui::config_screen::tests::`
- `cargo test app::tests::`

Expected: FAIL because the config menu has no refresh action and app orchestration does not support it.

- [ ] **Step 3: Write minimal implementation**

Add a helper in `src/mods/manager.rs`:

```rust
pub fn installed_mod_ids(db: &ModsDb) -> Vec<u64> {
    db.mods.iter().map(|m| m.id).collect()
}
```

Add `Action::RefreshInstalledMods` and handle it in `App` by:

- collecting installed IDs
- calling Steamworks download/update on them
- reusing existing pending-download status handling
- rescanning mods after completion
- surfacing status text that says the game itself updates through Steam

- [ ] **Step 4: Run test to verify it passes**

Run:

- `cargo test mods::manager::tests::`
- `cargo test ui::config_screen::tests::`
- `cargo test app::tests::`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/ui/config_screen.rs src/ui/mod.rs src/app.rs src/mods/manager.rs
git commit -m "feat: refresh installed workshop mods"
```

### Task 5: Offline Runtime Sync And Launch Helpers

**Files:**
- Create: `src/offline/sync.rs`
- Create: `src/offline/launch.rs`
- Modify: `src/offline/mod.rs`
- Modify: `src/profile.rs`
- Modify: `src/launch.rs`
- Test: `src/offline/sync.rs`
- Test: `src/offline/launch.rs`

- [ ] **Step 1: Write the failing tests**

Add tests for:

- deriving a runtime mission target under `DayZ/Missions`
- detecting drift for managed mission targets before overwrite
- requiring explicit confirmation when managed sync would replace non-normalized
  local edits
- forcing Namalsk dependency mods
- toggling `HIVE_ENABLED` in the runtime mission copy
- building offline launch args with:
  - `-filePatching`
  - `-mission=./Missions/<runtime-mission>`
  - selected `-mod=...`
  - `-doLogs`
  - `-scriptDebug=true`
  - normal profile launch options

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test offline::sync::tests:: offline::launch::tests::`
Expected: FAIL because runtime sync and offline launch assembly helpers do not exist.

- [ ] **Step 3: Write minimal implementation**

Add narrow helpers:

```rust
pub fn runtime_target_name(mission_name: &str) -> String;
pub fn sync_runtime_mission(... ) -> Result<PathBuf>;
pub fn build_offline_launch_args(... ) -> Vec<String>;
```

Keep file-copy and `HIVE_ENABLED` mutation in the offline modules, not in UI code. Reuse `Profile.offline` for remembered mod IDs and spawn toggle.
`sync_runtime_mission(...)` must return enough information for the app/UI layer
to distinguish:

- no overwrite needed
- overwrite confirmation required because non-normalized content drifted
- hard failure

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test offline::sync::tests:: offline::launch::tests::`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/offline/mod.rs src/offline/sync.rs src/offline/launch.rs src/profile.rs src/launch.rs
git commit -m "feat: add offline runtime sync and launch helpers"
```

### Task 6: Offline Browser, Setup, And App Integration

**Files:**
- Create: `src/ui/offline_browser.rs`
- Create: `src/ui/offline_setup.rs`
- Modify: `src/ui/main_menu.rs`
- Modify: `src/ui/mod.rs`
- Modify: `src/app.rs`
- Modify: `src/offline/discovery.rs`
- Modify: `src/offline/install.rs`
- Test: `src/ui/offline_browser.rs`
- Test: `src/ui/offline_setup.rs`
- Test: `src/app.rs`

- [ ] **Step 1: Write the failing tests**

Add tests covering:

- main menu exposes `Offline Mode`
- offline browser shows discovered managed and existing missions
- offline browser still opens when GitHub metadata fetch fails but local
  managed or existing missions are available
- offline setup preloads remembered per-mission preferences
- offline setup blocks early with a clear status/error when `dayz_path` is
  unavailable
- offline setup stores selected mods and spawn toggle into shared launch prep
- app routes offline install/update/setup/launch correctly

- [ ] **Step 2: Run test to verify it fails**

Run:

- `cargo test offline_browser`
- `cargo test offline_setup`
- `cargo test app::tests::`

Expected: FAIL because there is no user-facing offline flow yet.

- [ ] **Step 3: Write minimal implementation**

Build the two UI screens and route them through `App`:

- `OfflineBrowserScreen` for mission list and install/update availability
- `OfflineSetupScreen` for mods and spawn toggle

Use the existing offline discovery/install backend plus Task 5 helpers for sync and launch. Reuse the shared launch-prep pipeline rather than calling launch directly from the screens.
Make the GitHub-release check advisory for browser entry: local mission discovery
must still populate the browser and keep launch available when remote metadata
lookup fails.
Key `Profile.offline` by the stable offline mission identity from discovery,
not the display name, and fail before launch with a clear message when no valid
`dayz_path` exists for runtime mission sync.

- [ ] **Step 4: Run test to verify it passes**

Run:

- `cargo test offline_browser`
- `cargo test offline_setup`
- `cargo test app::tests::`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/ui/offline_browser.rs src/ui/offline_setup.rs src/ui/main_menu.rs src/ui/mod.rs src/app.rs src/offline/discovery.rs src/offline/install.rs
git commit -m "feat: add offline mode browser and setup flow"
```

### Task 7: Final Verification And Plan Status

**Files:**
- Modify: `docs/superpowers/plans/2026-04-06-launch-parity-steamworks.md`

- [ ] **Step 1: Run verification**

Run: `cargo test`
Expected: PASS

- [ ] **Step 2: Run lint verification**

Run: `cargo clippy --all-targets --all-features -- -D warnings`
Expected: PASS

- [ ] **Step 3: Mark plan progress**

Update this plan file checkboxes to reflect completed work.

- [ ] **Step 4: Commit**

```bash
git add docs/superpowers/plans/2026-04-06-launch-parity-steamworks.md
git commit -m "docs: mark launch parity plan progress"
```
