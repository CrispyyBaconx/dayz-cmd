# Self-Update Design

## Goal

Add a safe launch-time self-update flow for the Rust TUI launcher that checks GitHub Releases, shows a 5-second auto-dismiss prompt defaulting to "No", runs a release-published installer script when the user confirms, and restarts the launcher after a successful update.

## Product Behavior

- On startup, the launcher performs a short GitHub Releases check after config/profile load.
- If no newer release exists, startup continues with no visible interruption.
- If a newer release exists, the launcher shows a modal prompt before the main menu:
  - default selection is `No`
  - countdown starts at 5 seconds
  - timeout auto-selects `No`
  - `Yes` begins update
- If update succeeds, the launcher restarts itself.
- If update fails for any reason, the launcher shows a status/error message and continues normally.

## Source Of Truth

- Update metadata comes from GitHub Releases only.
- The launcher ignores drafts and prereleases by default.
- The latest stable release tag is compared against `CARGO_PKG_VERSION`.

## Release Contract

Each valid Linux release must publish an installer asset with a fixed predictable name:

- `dayz-ctl-installer-linux.sh`

The launcher will only use that asset name. It will not scrape arbitrary release assets or guess platform artifacts.

## Architecture

### `src/api/releases.rs`

Small GitHub Releases client responsible for:

- fetching latest release metadata
- filtering drafts/prereleases
- parsing the latest tag
- finding the Linux installer asset URL
- exposing a narrow release-check API

This module should be mostly pure parsing and HTTP behavior, with unit tests around JSON parsing and version selection.

### `src/update.rs`

Workflow coordinator responsible for:

- comparing current and latest versions
- downloading the installer script to a temp file
- marking it executable
- invoking it with explicit arguments/environment
- restarting the current executable on success

This module owns the risky process/filesystem behavior so the UI stays thin.

### `src/ui/update_prompt.rs`

Startup modal responsible for:

- showing the new version and countdown
- defaulting to `No`
- auto-dismissing after 5 seconds
- returning either `Skip` or `RunUpdate`

This should fit the existing screen stack/event-loop shape rather than creating a separate interaction model.

### `src/app.rs`

Minimal integration:

- run a session-scoped update check during startup
- push the update prompt screen when a newer release exists
- handle prompt selection by delegating to `update.rs`
- surface failures as status messages

## Runtime Flow

1. App loads config/profile.
2. App performs a GitHub Releases check with a short timeout.
3. If a newer release exists with the required installer asset, app pushes the update prompt.
4. If user chooses `No` or timeout expires, normal startup continues.
5. If user chooses `Yes`, app downloads the installer asset to a temp path.
6. App runs the installer with explicit version/repo inputs.
7. If installer exits successfully, app restarts the current executable and exits the current process.
8. If any step fails, app reports failure and continues normally.

## Installer Invocation

The app should execute the downloaded installer script with explicit inputs, for example:

- env:
  - `DAYZ_CTL_VERSION=<release tag>`
- args:
  - optional repo/asset inputs if needed later

The script is responsible for installing the selected release. The app is responsible only for download, execution, cleanup, and restart.

## Restart Semantics

- Restart uses `std::env::current_exe()`.
- The updated launcher is spawned after installer success.
- The current process exits after the new process is successfully spawned.
- If restart spawn fails, the user gets a status message instead of silent exit.

## Configuration

Initial version keeps configuration minimal:

- GitHub owner/repo defaults can be derived from config constants or config fields.
- Timeout values should reuse existing request-timeout patterns where reasonable.
- No background scheduler or periodic polling in this slice.

## Error Handling

The updater must fail closed and never block ordinary use:

- release check timeout: continue normally
- malformed release JSON: continue normally
- missing installer asset: continue normally
- download failure: show error, continue normally
- installer non-zero exit: show error, continue normally
- restart failure: show error, continue normally

## Testing

### Unit Tests

- `src/api/releases.rs`
  - parses latest stable release
  - ignores prereleases/drafts
  - finds expected installer asset
- `src/ui/update_prompt.rs`
  - countdown expires to `No`
  - default selection is `No`
  - explicit `Yes/No` key handling works
- `src/update.rs`
  - builds installer command correctly
  - handles missing asset URL
  - handles restart command construction

### Integration-Style Tests

- app startup path pushes update prompt only when newer release exists
- timeout path dismisses prompt and continues startup
- successful update path requests restart

## Rejected Alternatives

### Direct Binary Replacement

Rejected because replacing the running binary in-process is riskier around permissions, partial writes, and restart semantics.

### Manual Link-Only Prompt

Rejected because it does not satisfy the requested "Yes should update and restart" behavior.

### Reusing The Current Repo `install` Script As-Is

Rejected for now because the existing script is still aligned with the legacy Bash-era installation flow and is not yet a clean Rust release installer contract.
