---
name: Fix favorites migration
overview: "Add user-facing migration for the dayz-ctl to dayz-cmd data directory rename: a one-time startup prompt when legacy data is detected, plus a manual option in Config."
todos:
  - id: add-legacy-path
    content: "Add legacy_data_dir() helper to config.rs that returns ~/.local/share/dayz-ctl/, and a has_legacy_data() check"
    status: pending
  - id: impl-migration
    content: "Implement merge_legacy_profile() in profile.rs: load legacy profile, merge favorites/history/options with dedup"
    status: pending
  - id: add-confirm-action
    content: "Add MigrateLegacy variant to ConfirmAction, wire confirm/decline in popup.rs to call the merge and rename legacy file"
    status: pending
  - id: startup-prompt
    content: "On startup in main.rs, if legacy data exists and not yet migrated, push a Confirm(MigrateLegacy) screen before init_main_menu()"
    status: pending
  - id: config-menu-item
    content: "Add 'Migrate from dayz-ctl' item to ConfigScreen, conditionally shown when legacy profile exists, triggers the same Confirm dialog"
    status: pending
  - id: add-test
    content: "Add tests: merge deduplicates favorites, migration renames legacy file, has_legacy_data returns false after migration"
    status: pending
isProject: false
---

# Fix Favorites Regression via Profile Migration

## Root Cause

Commit `32ee0b7` changed `dirs_data_dir()` in [src/config.rs](src/config.rs) from `dayz-ctl` to `dayz-cmd`:

```rust
// Before (initial Rust rewrite, commit 97a9623):
directories::ProjectDirs::from("", "", "dayz-ctl")  // ~/.local/share/dayz-ctl/

// After (commit 32ee0b7):
directories::ProjectDirs::from("", "", "dayz-cmd")   // ~/.local/share/dayz-cmd/
```

Any favorites/history saved through the Rust TUI are stranded in `~/.local/share/dayz-ctl/profile.json`.

## Fix

Two ways to trigger migration, both leading to the same `ConfirmScreen` dialog:

### 1. One-time startup prompt

On startup, if `~/.local/share/dayz-ctl/profile.json` exists, push a `Confirm(MigrateLegacy)` screen that asks:

> "Legacy dayz-ctl config found. Migrate favorites and history?"

If the user confirms, merge and rename the legacy file. If they decline, do nothing (they can trigger it later from Config).

### 2. Config menu item

Add a conditional `"Migrate from dayz-ctl"` entry to the Config screen (only visible when the legacy profile file exists). Selecting it pushes the same `Confirm(MigrateLegacy)` screen.

## Key Files

- [src/config.rs](src/config.rs) -- add `legacy_data_dir()` helper and `has_legacy_data()` check
- [src/profile.rs](src/profile.rs) -- add `merge_legacy_profile()` that merges favorites/history from the legacy profile
- [src/ui/mod.rs](src/ui/mod.rs) -- add `MigrateLegacy` to `ConfirmAction` enum
- [src/ui/popup.rs](src/ui/popup.rs) -- wire message, confirm (merge + rename), decline (no-op pop) for `MigrateLegacy`
- [src/ui/config_screen.rs](src/ui/config_screen.rs) -- add `MigrateLegacyData` to `ConfigItem`, conditionally shown
- [src/main.rs](src/main.rs) -- on startup, if `has_legacy_data()`, push `Confirm(MigrateLegacy)` after `init_main_menu()`

## Implementation Details

### config.rs additions

```rust
pub fn legacy_data_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    Path::new(&home).join(".local/share/dayz-ctl")
}

pub fn has_legacy_data() -> bool {
    legacy_data_dir().join("profile.json").exists()
}
```

### profile.rs -- merge_legacy_profile()

```rust
pub fn merge_legacy_profile(current: &mut Profile, legacy_path: &Path) -> Result<()> {
    let legacy = Profile::load(legacy_path)?;
    for fav in &legacy.favorites {
        if !current.favorites.iter().any(|f| f.ip == fav.ip && f.port == fav.port) {
            current.favorites.push(fav.clone());
        }
    }
    for entry in &legacy.history {
        if !current.history.iter().any(|h| h.ip == entry.ip && h.port == entry.port) {
            current.history.push(entry.clone());
        }
    }
    Ok(())
}
```

### ui/mod.rs -- new ConfirmAction variant

```rust
pub enum ConfirmAction {
    // ... existing variants ...
    MigrateLegacy,
}
```

### ui/popup.rs -- wire the new action

- **message**: `"Legacy dayz-ctl config found. Migrate favorites and history?"`
- **yes_label**: `"Migrate"`
- **no_label**: `"Skip"`
- **confirm**: call `merge_legacy_profile()`, save profile, rename legacy file to `.migrated`, set status message
- **decline**: `Action::PopScreen` (no-op)

### ui/config_screen.rs -- conditional menu item

Add `ConfigItem::MigrateLegacyData` to `build_items()`:

```rust
if crate::config::has_legacy_data() {
    items.push(ConfigItem::MigrateLegacyData);
}
```

Label: `"Migrate from dayz-ctl"`. Action: `PushScreen(ScreenId::Confirm(ConfirmAction::MigrateLegacy))`.

### main.rs -- startup check

After `app.init_main_menu()`:

```rust
if crate::config::has_legacy_data() {
    app.process_action(Action::PushScreen(
        ScreenId::Confirm(ConfirmAction::MigrateLegacy),
    ));
}
```
