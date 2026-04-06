# Offline Mode Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add feature-parity offline mode backed by DayZCommunityOfflineMode, with launcher-managed storage, safe runtime copies, remembered per-mission settings, and launch-time sync/update behavior.

**Architecture:** Add a new `offline` subsystem that owns DCOM release state, mission discovery, runtime sync planning, and offline launch assembly. Keep unmanaged missions read-only by copying them into launcher-owned runtime targets before launch, and keep managed DCOM installs under app data with staging and atomic promotion.

**Tech Stack:** Rust, reqwest blocking client, serde, std fs/path/process, ratatui, cargo test, cargo clippy

---

## File Map

Note: the spec’s `offline/runtime/` app-data directory is treated as out of scope for this slice. Runtime mission copies for launch live under `DayZ/Missions`, which is the path the game actually consumes.

- Create: `src/offline/mod.rs`
- Create: `src/offline/types.rs`
- Create: `src/offline/storage.rs`
- Create: `src/offline/install.rs`
- Create: `src/offline/discovery.rs`
- Create: `src/offline/sync.rs`
- Create: `src/offline/launch.rs`
- Create: `src/api/offline_releases.rs`
- Create: `src/ui/offline_browser.rs`
- Create: `src/ui/offline_setup.rs`
- Modify: `src/main.rs`
- Modify: `src/api/mod.rs`
- Modify: `src/app.rs`
- Modify: `src/config.rs`
- Modify: `src/profile.rs`
- Modify: `src/launch.rs`
- Modify: `src/ui/main_menu.rs`
- Modify: `src/ui/mod.rs`
- Modify: `src/ui/popup.rs`
- Modify: `docs/superpowers/plans/2026-04-06-offline-mode.md`

### Task 1: Offline State, Identity, And Release Metadata

**Files:**
- Create: `src/offline/mod.rs`
- Create: `src/offline/types.rs`
- Create: `src/offline/storage.rs`
- Create: `src/api/offline_releases.rs`
- Modify: `src/main.rs`
- Modify: `src/api/mod.rs`
- Modify: `src/config.rs`
- Modify: `src/profile.rs`
- Test: `src/offline/storage.rs`
- Test: `src/api/offline_releases.rs`
- Test: `src/profile.rs`

- [ ] **Step 1: Write the failing tests**

Add inline tests for:
- managed offline state read/write round-trip
- mission identity keys:

```rust
assert_eq!(
    mission_identity_key(MissionSource::Managed, "DayZCommunityOfflineMode.ChernarusPlus", None),
    "managed:DayZCommunityOfflineMode.ChernarusPlus"
);
```

- existing mission identity keys being path-derived and stable
- DCOM release metadata parsing that ignores prereleases/drafts and selects the latest stable tag
- profile serialization for remembered offline settings:

```rust
profile.offline.insert(
    "managed:DayZCommunityOfflineMode.ChernarusPlus".into(),
    OfflineMissionPrefs { mod_ids: vec![1564026768], spawn_enabled: true },
);
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test offline::storage::tests:: api::offline_releases::tests:: profile::tests::offline`
Expected: FAIL because the offline state/types/modules do not exist yet.

- [ ] **Step 3: Write minimal implementation**

Implement:
- `src/offline/types.rs` with small data types such as:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OfflineState {
    pub installed_tag: Option<String>,
    pub latest_known_tag: Option<String>,
    pub managed_missions: Vec<String>,
    pub last_check_ts: Option<i64>,
}
```

- `src/offline/storage.rs` helpers for:
  - `offline_root(config: &Config) -> PathBuf`
  - `offline_state_path(config: &Config) -> PathBuf`
  - `load_offline_state(...) -> Result<OfflineState>`
  - `save_offline_state(...) -> Result<()>`
  - `mission_identity_key(...) -> String`
- `src/api/offline_releases.rs` with the DCOM GitHub Releases client and tarball URL selection
- `src/profile.rs` additions for `offline: BTreeMap<String, OfflineMissionPrefs>`
- `src/config.rs` additions for offline paths only if helpers need explicit config fields; otherwise keep this in `offline/storage.rs`
- crate/module declarations in `src/main.rs` and `src/api/mod.rs`

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test offline::storage::tests:: api::offline_releases::tests:: profile::tests::offline`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/offline/mod.rs src/offline/types.rs src/offline/storage.rs src/api/offline_releases.rs src/main.rs src/api/mod.rs src/config.rs src/profile.rs
git commit -m "feat: add offline state and release metadata"
```

### Task 2: Managed Install Workflow And Mission Discovery

**Files:**
- Create: `src/offline/install.rs`
- Create: `src/offline/discovery.rs`
- Modify: `src/offline/mod.rs`
- Modify: `src/offline/storage.rs`
- Modify: `src/api/offline_releases.rs`
- Test: `src/offline/install.rs`
- Test: `src/offline/discovery.rs`
- Test: `src/offline/storage.rs`

- [ ] **Step 1: Write the failing tests**

Add inline tests for:
- downloading the selected DCOM tarball into `offline/tmp`
- extracting the tarball into a staging directory
- failed download/extract leaving the previously installed managed release untouched
- discovering managed missions from `offline/releases/<tag>/Missions`
- discovering unmanaged mission folders under `DayZ/Missions`
- duplicate display names producing distinct identity keys and disambiguated display labels
- cleaning stale staging directories
- validating extracted DCOM layout before promotion
- promotion moving validated content into `offline/releases/<tag>/` without mutating `offline/state.json`

Use focused tempdir fixtures such as:

```rust
fs::create_dir_all(root.join("offline/tmp/install-123/Missions/DayZCommunityOfflineMode.ChernarusPlus"))?;
fs::write(root.join("offline/tmp/install-123/Missions/DayZCommunityOfflineMode.ChernarusPlus/core/CommunityOfflineClient.c"), "HIVE_ENABLED = true;")?;
```

Use an install workflow assertion like:

```rust
let result = install_release(&config, &release_info, &client);
assert!(result.is_err());
assert_eq!(load_offline_state(&config)?.installed_tag.as_deref(), Some("v1.0.0"));
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test offline_install offline_discovery offline_storage`
Expected: FAIL because install, discovery, and promotion helpers are incomplete.

- [ ] **Step 3: Write minimal implementation**

Implement:
- `src/offline/install.rs` with a thin workflow that owns:
  - tarball download into `offline/tmp`
  - extract into staging
  - call `validate_extracted_release(...)`
  - call `promote_release(...)`
  - return the successfully promoted tag and mission list without writing state
- managed install staging helpers in `src/offline/storage.rs`:
  - `staging_dir_for_tag(...)`
  - `release_dir_for_tag(...)`
  - `cleanup_stale_staging(...)`
  - `validate_extracted_release(...)`
  - `promote_release(...)`
- `src/offline/discovery.rs` types and helpers like:

```rust
pub enum MissionSource {
    Managed,
    Existing,
}

pub struct OfflineMission {
    pub id: String,
    pub name: String,
    pub source: MissionSource,
    pub source_path: PathBuf,
    pub runtime_name: String,
}
```

- merged mission enumeration that follows the spec’s managed/existing behavior
- keep `offline/state.json` writes out of this task’s helpers; app-level orchestration will own the “promotion succeeded, now persist state” ordering

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test offline_install offline_discovery offline_storage`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/offline/mod.rs src/offline/storage.rs src/offline/install.rs src/offline/discovery.rs src/api/offline_releases.rs
git commit -m "feat: add offline install and mission discovery"
```

### Task 3: Runtime Copy, Drift Detection, And Offline Launch Builder

**Files:**
- Create: `src/offline/sync.rs`
- Create: `src/offline/launch.rs`
- Modify: `src/offline/mod.rs`
- Modify: `src/launch.rs`
- Modify: `src/profile.rs`
- Test: `src/offline/sync.rs`
- Test: `src/offline/launch.rs`
- Test: `src/launch.rs`

- [ ] **Step 1: Write the failing tests**

Add inline tests for:
- unmanaged missions being copied into a launcher-owned runtime target without touching the source tree
- managed runtime sync detecting only non-normalized drift
- canceled overwrite leaving target files untouched
- `HIVE_ENABLED` toggling in the runtime copy only
- Namalsk dependency injection adding `2289456201` and `2289461232`
- offline args preserving selected mod IDs in `-mod=@...;@...`
- offline args preserving normal profile launch options such as `-nosplash`
- offline launch args containing:

```rust
assert!(args.contains(&"-filePatching".to_string()));
assert!(args.contains(&"-mission=./Missions/dayz-cmd-offline-DayZCommunityOfflineMode.ChernarusPlus".to_string()));
assert!(args.contains(&"-doLogs".to_string()));
assert!(args.contains(&"-scriptDebug=true".to_string()));
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test offline_sync offline_launch launch_offline`
Expected: FAIL because the sync and offline launch helpers do not exist yet.

- [ ] **Step 3: Write minimal implementation**

Implement:
- `src/offline/sync.rs` with:
  - runtime target naming, for example `dayz-cmd-offline-<mission>`
  - copy helpers for managed and existing missions
  - drift detection that ignores launcher-controlled normalization only
  - an overwrite decision result that app/UI can consume
- `src/offline/launch.rs` with:
  - `inject_required_mods(...)`
  - `set_hive_enabled(...)`
  - `build_offline_launch_args(...)`
- `src/launch.rs` additions if a dedicated offline launch-args helper keeps app code cleaner

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test offline_sync offline_launch launch_offline`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/offline/mod.rs src/offline/sync.rs src/offline/launch.rs src/launch.rs src/profile.rs
git commit -m "feat: add offline runtime sync and launch builder"
```

### Task 4: Offline Browser And Setup Screens

**Files:**
- Create: `src/ui/offline_browser.rs`
- Create: `src/ui/offline_setup.rs`
- Modify: `src/ui/mod.rs`
- Modify: `src/ui/main_menu.rs`
- Modify: `src/ui/popup.rs`
- Modify: `src/profile.rs`
- Test: `src/ui/offline_browser.rs`
- Test: `src/ui/offline_setup.rs`

- [ ] **Step 1: Write the failing tests**

Add screen tests for:
- main menu now including `Offline Mode`
- offline browser showing managed and existing missions
- install action disabled when GitHub metadata is unavailable and no managed install exists
- duplicate mission names rendering with disambiguated labels
- offline setup preloading remembered mod IDs and spawn toggle
- launching from setup returning the expected action payload

Example assertion shape:

```rust
assert!(screen.items().iter().any(|item| item.label.contains("Offline Mode")));
assert_eq!(screen.spawn_enabled(), Some(true));
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test offline_browser offline_setup main_menu`
Expected: FAIL because the new screens and menu item do not exist yet.

- [ ] **Step 3: Write minimal implementation**

Implement:
- `src/ui/offline_browser.rs` to render:
  - install/update state
  - GitHub warning/status
  - merged mission list
- `src/ui/offline_setup.rs` to render:
  - selected mission
  - selected mod count
  - spawn toggle
  - launch/update actions
- new `Action`/`ScreenId` variants in `src/ui/mod.rs` for offline flow
- `src/ui/main_menu.rs` menu entry wiring
- popup reuse in `src/ui/popup.rs` for overwrite-confirm behavior if a new confirm action is needed

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test offline_browser offline_setup main_menu`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/ui/offline_browser.rs src/ui/offline_setup.rs src/ui/mod.rs src/ui/main_menu.rs src/ui/popup.rs src/profile.rs
git commit -m "feat: add offline browser and setup screens"
```

### Task 5: App Wiring, Mod Downloads, And Launch Execution

**Files:**
- Modify: `src/app.rs`
- Modify: `src/ui/mod.rs`
- Modify: `src/steam/workshop.rs`
- Modify: `src/mods/manager.rs`
- Modify: `src/profile.rs`
- Test: `src/app.rs`
- Test: `src/mods/manager.rs`

- [ ] **Step 1: Write the failing tests**

Add integration-style tests for:
- entering offline mode with GitHub unavailable but a managed install present
- entering offline mode with no managed install and an unmanaged mission present
- entering offline mode with no managed install and GitHub unavailable disables install cleanly
- successful install/update persisting `offline/state.json` only after promotion succeeds
- failed install/update keeping the current managed tag and release directory intact
- launching a managed mission with drift prompt, cancel path leaves files untouched
- launching an unmanaged mission uses a runtime copy and leaves the source mission unchanged
- remembered mission settings reload on a later visit
- missing workshop mods trigger the existing download workflow before launch

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test app_offline mods_manager`
Expected: FAIL because the app does not yet integrate the offline workflow.

- [ ] **Step 3: Write minimal implementation**

Implement in `src/app.rs`:
- offline state in `App`
- on-enter discovery/update-check behavior for the offline browser
- action handling for:
  - install/update DCOM
  - open setup
  - confirm overwrite
  - offline launch
- persist `offline/state.json` only after `install_release(...)` returns a successfully promoted tag
- profile persistence for remembered mission settings
- reuse the existing pending mod-download flow before offline launch

Keep the launch path ordered:

```rust
// 1. ensure/update required workshop mods
// 2. prepare runtime mission copy
// 3. toggle HIVE_ENABLED in runtime copy
// 4. build offline args
// 5. launch and persist remembered settings
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test app_offline mods_manager`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/app.rs src/ui/mod.rs src/steam/workshop.rs src/mods/manager.rs src/profile.rs
git commit -m "feat: wire offline mode flow"
```

### Task 6: Final Verification And Plan Closeout

**Files:**
- Modify: `docs/superpowers/plans/2026-04-06-offline-mode.md`

- [ ] **Step 1: Run targeted verification**

Run: `cargo test offline_releases offline_storage offline_install offline_discovery offline_sync offline_launch profile::tests::offline launch::tests:: main_menu offline_browser offline_setup app_offline mods_manager`
Expected: PASS

- [ ] **Step 2: Run full verification**

Run: `cargo test`
Expected: PASS

- [ ] **Step 3: Run lint verification**

Run: `cargo clippy --all-targets --all-features -- -D warnings`
Expected: PASS

- [ ] **Step 4: Mark plan progress**

Update this plan file checkboxes to reflect completed work.

- [ ] **Step 5: Commit**

```bash
git add docs/superpowers/plans/2026-04-06-offline-mode.md
git commit -m "docs: update offline mode plan status"
```
