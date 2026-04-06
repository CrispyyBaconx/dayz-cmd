# Launch Parity Steamworks-Only Design

## Goal

Close the remaining user-visible launcher parity gaps against the original
`dayz-ctl` shell script without reintroducing SteamCMD. The Rust TUI should:

- support unknown direct-connect launches with optional password and manual mod
  selection from locally installed Workshop mods
- gate startup on `vm.max_map_count >= 1048576` with the same confirm-or-print
  behavior as the original script
- replace the old SteamCMD-only "Force update game and all mods" action with a
  Steamworks-only `Refresh installed mods` action
- expose the offline-mode backend already present under `src/offline/*` through
  TUI screens and launch orchestration

## Constraints

- Steamworks only. No SteamCMD subprocesses, no `steamcmd +app_update`, and no
  dual backend.
- The launcher may manage Workshop items through the Steam client, but DayZ
  game updates remain owned by Steam itself.
- New launch paths should reuse one shared app-level launch pipeline rather
  than creating separate mini launchers in UI screens.

## Current State

Already present:

- Steamworks Workshop download/update support
- known-server launch flow
- protected known-server password prompt
- running-DayZ kill prompt
- offline storage, install metadata, and mission discovery primitives
- manual "Check for Updates" config action

Still missing:

- unknown direct connect setup for local mods and password
- `vm.max_map_count` startup gate
- user-facing offline browser/setup/launch flow
- Steamworks replacement for forced mod refresh

## Product Behavior

### 1. Unknown Direct Connect

When the user enters an IP and port not found in the cached server list:

1. the direct-connect screen records the target endpoint
2. instead of launching immediately, the app opens a setup screen
3. that setup screen allows:
   - selecting any locally installed mods from `mods_db`
   - optionally entering a server password
4. confirm launches DayZ with:
   - `-connect=<ip>`
   - `-port=<port>`
   - optional `-password=<password>`
   - optional `-mod=@...;@...`
   - normal profile launch options

This matches the old script's "manual pick local mods" behavior without trying
to discover remote server mods.

### 2. `vm.max_map_count` Startup Gate

At startup, before normal launcher use:

- read `/proc/sys/vm/max_map_count`
- if the value is at least `1048576`, continue normally
- if the value is below `1048576`, show a confirm screen

If the user confirms, run the same two operations the shell script used:

```bash
echo "vm.max_map_count=1048576" | sudo tee /etc/sysctl.d/50-dayz.conf
sudo sysctl -w vm.max_map_count=1048576
```

If the user declines:

- show the commands in a dedicated info/status screen
- exit instead of continuing into the launcher

If the fix command fails:

- surface the failure clearly
- exit rather than pretending the system is ready

### 3. Refresh Installed Mods

Config gains `Refresh installed mods`.

Behavior:

- use Steamworks to request download/update for every mod currently present in
  `mods_db`
- reuse the existing pending-download/progress flow in `App`
- when complete, rescan installed mods and refresh `mods_db`
- status copy must explicitly note that DayZ game updates happen through Steam,
  not through the launcher

This replaces the intent of the old SteamCMD action, but not its exact game
update semantics.

### 4. Offline Mode UI

Main menu gains `Offline Mode`.

Entering it should:

- discover managed DCOM missions from app data
- discover existing missions under `DayZ/Missions`
- opportunistically check for a newer DCOM release
- open a browser screen even if GitHub is unavailable, as long as missions are
  still available locally

Per-mission setup should allow:

- selecting a mission
- selecting local Workshop mods
- toggling spawn enablement
- remembering those preferences in `Profile.offline`

Launch should:

- reuse the shared launch-prep state
- reuse Steamworks download/update handling for selected mods
- use the offline backend to sync runtime mission content and build args

## Architecture

### Shared Launch Prep In `App`

Introduce a small app-owned launch-prep state instead of encoding launch
decisions in individual screens.

Suggested shape:

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
}
```

Responsibilities:

- screens only collect input and populate `LaunchPrep`
- `App::do_launch()` stays the single gate for:
  - sysctl readiness
  - running-DayZ prompt
  - password requirement if still missing
  - Workshop refresh/download handling
  - symlink setup
  - final launch

This keeps launch policy centralized.

## UI Additions

### `src/ui/direct_connect_setup.rs`

New screen for unknown direct-connect targets.

Responsibilities:

- display target endpoint
- list locally installed mods from `mods_db`
- toggle selection
- optionally open password prompt
- confirm into shared launch-prep state

### `src/ui/offline_browser.rs`

New mission browser.

Responsibilities:

- show managed/existing missions
- show install/update state
- route to offline setup

### `src/ui/offline_setup.rs`

New per-mission setup screen.

Responsibilities:

- preload remembered `Profile.offline` preferences
- select mods
- toggle spawn enablement
- submit shared launch-prep state

### `src/ui/sysctl_prompt.rs` or reuse `ConfirmScreen`

The sysctl gate can either:

- reuse `ConfirmScreen` with a new `ConfirmAction::FixMaxMapCount`, plus a
  follow-up message screen for the manual commands
- or use a dedicated prompt screen if the copy needs more room

Recommendation: reuse `ConfirmScreen` for the yes/no decision and add a small
read-only info screen for the manual commands.

## App Orchestration

### `src/app.rs`

Add orchestration for:

- startup sysctl check
- shared `LaunchPrep`
- direct-connect setup routing
- refresh-installed-mods action
- offline mission discovery/update/setup routing

Important rule: launch order should be deterministic.

Recommended order:

1. startup sysctl gate
2. launch target exists
3. running DayZ guard
4. password prompt if required and missing
5. refresh/download mods if needed
6. symlink/runtime sync
7. launch

### `src/config.rs`

Add system helpers:

- read current `vm.max_map_count`
- build the expected manual commands
- optionally execute the fix via `sudo`

Keep shell invocation details out of UI code.

### `src/offline/*`

The existing backend stays authoritative for:

- release metadata
- install staging/promotion
- mission discovery
- state persistence

Likely additional modules still needed for parity:

- `src/offline/sync.rs`
- `src/offline/launch.rs`

The UI/app layer should call these, not reproduce file-copy/launch rules.

## Steamworks-Only Refresh Semantics

`Refresh installed mods` should:

- collect installed mod IDs from `mods_db.mods`
- call Steamworks download/update for those Workshop IDs
- monitor progress using the same status mechanism already used for server
  mod downloads
- rescan local mods on completion

Copy guidance:

- menu label: `Refresh installed mods`
- success status: mention refreshed Workshop mods
- help/status copy: mention that DayZ game updates are handled by Steam

This avoids lying about game updates while preserving the useful "force my
mods to refresh now" workflow.

## Failure Handling

### Unknown Direct Connect

- no installed mods selected is valid
- empty password stays `None`
- invalid port keeps current validation behavior

### Sysctl Gate

- missing `/proc/sys/vm/max_map_count`: show error and continue only if the
  platform clearly does not expose the setting; on normal Linux, treat read
  failures as blocking
- `sudo` failure: show error and exit
- decline: show commands and exit

### Refresh Installed Mods

- no Steam handle: show a clear status message and do nothing
- empty `mods_db`: show "No installed mods to refresh"
- partial Workshop failure: keep progress/error status visible and do not claim
  success

### Offline Mode

- GitHub unavailable but managed install exists: allow use with warning
- GitHub unavailable and no managed install: existing unmanaged missions still
  work; managed install/update is disabled
- missing DayZ path: block offline launch with a clear message

## Testing

### App Tests

- startup sysctl low-value path pushes prompt
- declining sysctl gate exits after surfacing manual commands
- confirming sysctl gate runs fix and blocks on failure
- unknown direct connect routes to setup instead of immediate launch
- refresh installed mods handles empty mod list and no Steam handle
- offline launch-prep flows into shared launch handling

### UI Tests

- direct-connect setup screen stores selected mods and optional password
- offline browser shows managed and existing missions
- offline setup preloads remembered preferences
- config screen exposes `Refresh installed mods`

### Backend Tests

- sysctl helper parses `/proc/sys/vm/max_map_count`
- sysctl helper builds the exact two manual commands
- refresh-installed-mods selection derives IDs from `mods_db`
- offline sync/launch helpers build the right runtime target and launch args

## Implementation Order

Recommended order:

1. shared launch-prep state in `App`
2. unknown direct-connect setup flow
3. `vm.max_map_count` startup gate
4. `Refresh installed mods`
5. offline browser/setup screens
6. offline app integration and launch orchestration

This closes the highest-value gameplay gap first while still moving toward a
clean launch architecture instead of accumulating one-off branches.
