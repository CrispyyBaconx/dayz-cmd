# Offline Mode Design

## Goal

Add feature-parity offline mode to the Rust TUI launcher using DayZCommunityOfflineMode (DCOM), while improving storage and update safety by keeping launcher-managed mission content under the app data directory and syncing it into `DayZ/Missions` only when needed for launch.

## Product Behavior

- The main menu gains an `Offline Mode` entry.
- Offline mode discovers and offers:
  - launcher-managed DCOM missions
  - any other mission folders already present under `DayZ/Missions`
- The launcher checks GitHub Releases for newer DCOM builds opportunistically.
- If GitHub is unavailable but a managed DCOM copy is already installed, offline launch remains available and the UI shows a warning instead of blocking.
- If no managed DCOM copy is installed, the launcher offers install before offline launch.
- Before launch, the launcher lets the user:
  - choose a mission
  - choose optional mods
  - toggle spawn enablement
- The launcher remembers the selected mods and spawn toggle per mission.
- Namalsk offline launches always include workshop mods `2289456201` and `2289461232`, matching the old script.
- Before syncing managed mission content into `DayZ/Missions`, the launcher detects local drift and asks before overwriting local edits.
- The launcher never edits unmanaged mission folders in place; it stages a runtime copy for launch and applies spawn toggles only to that runtime copy.
- Offline launch uses the same effective game arguments as the old shell launcher:
  - `-filePatching`
  - `-mission=./Missions/<mission>`
  - selected `-mod=...`
  - `-doLogs`
  - `-scriptDebug=true`
  - normal profile launch options

## Legacy Parity

The old shell launcher installs DCOM from `Arkensor/DayZCommunityOfflineMode`, stores it under `DayZ/Missions`, lets the user choose any mission folder found there, optionally updates selected workshop mods, forces Namalsk dependencies, toggles `HIVE_ENABLED`, and launches with mission/file-patching arguments.

This design preserves that user-visible capability but changes the storage model:

- the launcher owns its managed DCOM copy under app data
- `DayZ/Missions` remains a launch target, not the source of truth
- unmanaged mission folders under `DayZ/Missions` are still discoverable and launchable
- unmanaged missions are treated as read-only source content by the launcher

## Storage Model

### App Data Layout

Under `Config.data_dir`, add an offline root:

- `offline/state.json`
- `offline/releases/<tag>/`
- `offline/releases/<tag>/Missions/<mission>/...`
- `offline/runtime/`
- `offline/tmp/`

### `offline/state.json`

Stores launcher-managed offline metadata:

- installed DCOM tag, if any
- latest known GitHub tag, if available
- managed mission names extracted from the installed release
- last successful update check timestamp

This file is launcher-owned state only. It does not store user preferences.

### Profile Persistence

Extend `Profile` with per-mission offline preferences:

- selected mod IDs
- spawn enabled/disabled

Preferences must be keyed by mission identity, not mission name alone.

- managed DCOM missions use `managed:<mission>`
- unmanaged missions use `existing:<canonical-path-hash>`

Suggested shape:

```json
{
  "offline": {
    "managed:DayZCommunityOfflineMode.ChernarusPlus": {
      "mod_ids": [1564026768],
      "spawn_enabled": true
    }
  }
}
```

This keeps offline preferences alongside favorites, history, and launch options.

## Architecture

### `src/api/offline_releases.rs`

Small client focused on DCOM release metadata:

- fetch latest stable release from `Arkensor/DayZCommunityOfflineMode`
- ignore prereleases and drafts
- resolve the selected tag
- provide download URL for the release tarball

This module should stay mostly pure around JSON parsing and HTTP behavior.

### `src/offline/storage.rs`

Launcher-managed storage helpers:

- read/write `offline/state.json`
- compute managed paths under app data
- enumerate managed missions from the installed release
- determine whether a managed tag is already installed

This is the source of truth for launcher-owned DCOM content.

### `src/offline/discovery.rs`

Mission discovery and classification:

- discover launcher-managed DCOM missions
- discover unmanaged mission folders already under `DayZ/Missions`
- merge them into one list for UI presentation
- mark each mission as `Managed` or `Existing`
- assign a stable mission identity key for profile persistence
- disambiguate duplicate folder names in the UI when both managed and existing missions share the same display name

This preserves the old script's "show missions in `DayZ/Missions`" behavior without losing track of which ones the launcher owns.

### `src/offline/sync.rs`

Sync planning and drift detection:

 - compare managed mission content with the target directory in `DayZ/Missions`
 - detect drift in an existing managed mission target
 - generate a sync plan for copy/update
 - perform the sync after user confirmation when overwrite is required

Drift should be determined from content shape, not timestamps alone. The goal is to avoid silently overwriting local edits.

Drift must be defined narrowly:

- differences caused only by launcher-controlled normalization are ignored
- launcher-controlled normalization includes:
  - `HIVE_ENABLED` toggles in the runtime copy
  - launcher-owned marker files, if any
- overwrite confirmation is required only when non-normalized content differs from the managed source

Managed mission sync should use a runtime target under `DayZ/Missions` and should preserve user-edited non-launcher files by prompting before replacement.

### `src/offline/launch.rs`

Offline launch assembly:

- resolve mission path
- build selected mod IDs
- force-add Namalsk dependencies when needed
 - update `HIVE_ENABLED` in the runtime mission copy's `core/CommunityOfflineClient.c`
 - assemble offline launch arguments

This module should be mostly pure except for the file edit to the runtime `CommunityOfflineClient.c`.

### `src/ui/offline_browser.rs`

Mission-selection screen:

- show install/update status
- show GitHub warning when network check fails
- list managed and existing missions
- offer install/update actions for managed DCOM

### `src/ui/offline_setup.rs`

Per-mission setup screen:

- show selected mission
- preload remembered mod and spawn settings
- allow mod selection from the installed mods DB
- launch after sync/update checks

### `src/app.rs`

Minimal orchestration:

- initialize offline state paths
- load managed offline state
- trigger opportunistic DCOM update checks on entering offline mode
- route install/update/sync/launch actions
- surface warnings and failures as status messages

## Runtime Flow

### Entering Offline Mode

1. User selects `Offline Mode` from the main menu.
2. App discovers:
   - managed DCOM installation and missions from app data
   - mission folders already present in `DayZ/Missions`
3. App performs a GitHub Releases check for DCOM with the normal request timeout.
4. If the check fails, the browser screen still opens; any installed managed DCOM remains usable.
5. If no managed DCOM copy exists:
   - install is enabled when GitHub metadata is available
   - install is disabled with an explanatory message when GitHub metadata is unavailable

### Installing Or Updating DCOM

1. App fetches the latest stable DCOM release metadata.
2. App downloads the release tarball into `offline/tmp`.
3. App extracts it into a staging directory under `offline/tmp`.
4. App validates the extracted layout before promotion:
   - expected `Missions/...` subtree exists
   - at least one mission is present
   - expected `CommunityOfflineClient.c` path exists for managed DCOM missions
5. App atomically promotes the staging directory into `offline/releases/<tag>/`.
6. App updates `offline/state.json` only after promotion succeeds.
7. App refreshes the offline mission list.

The install target is app data, not `DayZ/Missions`.

On startup or before a new install attempt, the app should clean up stale staging directories left behind by interrupted installs.

### Launching A Managed Mission

1. User selects a managed mission.
2. App loads remembered per-mission settings.
3. User confirms mod selection and spawn toggle.
4. App ensures required workshop mods exist or offers mod update before launch.
5. App computes a sync plan from managed mission content to the runtime mission target in `DayZ/Missions`.
6. If non-normalized target content differs from the managed copy, app asks before overwriting.
7. If the user cancels, launch is aborted and no files are changed.
8. App applies the sync.
9. App updates `HIVE_ENABLED` in the runtime copy.
10. App assembles launch args and launches DayZ.

### Launching An Existing Unmanaged Mission

1. User selects a mission that already exists under `DayZ/Missions` but is not launcher-managed.
2. App copies that mission into a launcher-owned runtime target under `DayZ/Missions`.
3. User configures mods and spawn toggle.
4. App updates `HIVE_ENABLED` in the runtime copy.
5. App launches with the offline args using the runtime copy, not the original mission directory.

This matches the old script's ability to launch arbitrary mission folders already present in `DayZ/Missions`.

## Mod Behavior

- The mod picker uses the existing installed mods DB.
- If selected mods are missing, the launcher reuses the existing workshop-download flow before launch.
- Namalsk mission launches force-add:
  - `2289456201`
  - `2289461232`
- Selected mods are symlinked into the DayZ install using the existing mod-linking logic.

## Error Handling

Offline mode must fail closed for destructive operations but remain usable when possible:

- GitHub check failure: warning only if a managed install already exists
- missing managed install and GitHub failure: install unavailable, existing unmanaged missions still launch
- tarball download/extract failure: show error, keep previous managed install if present
- interrupted install or partial extract: staging content is ignored until validation succeeds and promotion completes
- drift detected in target mission: require explicit confirmation before overwrite
- canceled overwrite prompt: leave target files untouched and abort launch
- missing `CommunityOfflineClient.c`: fail launch with a clear status message
- missing required workshop mods and failed download: fail launch with a clear status message

## Testing

### Unit Tests

- release metadata parsing and latest-tag selection
- offline state read/write and path derivation
- mission discovery merging managed and unmanaged missions
- sync planning and drift detection
- canceled drift confirmation leaves runtime target untouched
- Namalsk dependency injection
- offline launch arg construction
- `HIVE_ENABLED` file toggle behavior
- remembered per-mission settings serialization
- duplicate mission-name discovery with distinct identity keys
- interrupted install cleanup and staging validation

### Integration-Style Tests

- entering offline mode with GitHub unavailable but a managed install present
- entering offline mode with no managed install and an unmanaged mission present
- entering offline mode with no managed install and GitHub unavailable disables install cleanly
- launching a managed mission with drift prompts before overwrite
- launching Namalsk forces its required mods
- remembered mission settings reload on a later visit
- unmanaged mission launch uses a runtime copy and leaves the source mission unchanged

## Rejected Alternatives

### Direct Install Into `DayZ/Missions`

Rejected because it makes launcher updates and cleanup riskier, and it turns the game install into mutable launcher state.

### Managing Only DCOM And Hiding Other Missions

Rejected because the old launcher exposes any mission folder already under `DayZ/Missions`, and that parity is useful for custom/local offline content.

### Importing Every Existing Mission Into Managed Storage

Rejected because it adds migration complexity without clear product value for the first offline slice.
