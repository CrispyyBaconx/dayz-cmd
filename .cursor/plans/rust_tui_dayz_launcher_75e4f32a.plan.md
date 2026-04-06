---
name: Rust TUI DayZ Launcher
overview: "Rewrite dayz-ctl from a Bash script into a Rust TUI application using ratatui for the terminal UI and steamworks-rs for Steam/Workshop integration, preserving all major features: server browsing with fuzzy search, mod management, favorites/history, direct connect, news, config, and game launching."
todos:
  - id: scaffold
    content: Create Cargo project, define core types (Server, Mod, Profile, Config, LaunchOption), set up ratatui event loop skeleton with screen enum and crossterm backend
    status: completed
  - id: data-layer
    content: Implement HTTP fetching (dayzsalauncher.com servers, dayz.com news, BattleMetrics), JSON caching with TTL, and mod database scanning from workshop meta.cpp files
    status: completed
  - id: steam-integration
    content: Initialize steamworks-rs client with app ID 221100, implement UGC subscribe/download for workshop mods, player count query, and callback pump thread
    status: completed
  - id: tui-main-menu
    content: Build main menu screen with navigable list, news snippet header, stats bar (players online, server count, user name)
    status: completed
  - id: tui-server-browser
    content: Build server browser with fuzzy search via nucleo, sortable table columns, split-pane detail preview (players, map, mods, ping, geo), and keyboard navigation
    status: completed
  - id: tui-filters
    content: "Implement multi-select filter dialog supporting all current filters: official/community, modded/vanilla, password, 1PP/3PP, day/night, BattlEye, player count, map, mods, platform"
    status: completed
  - id: tui-config
    content: "Build config screen: launch options editor, Steam path, player name, mod info viewer, and settings persistence"
    status: completed
  - id: tui-dialogs
    content: Build direct connect dialog (IP/port input), confirm/alert popups, news viewer (scrollable article list), mod install progress display
    status: completed
  - id: mod-management
    content: Implement mod symlink creation (@id links in DayZ dir), installed vs required diff, managed mod tracking, bulk remove
    status: completed
  - id: game-launch
    content: Build launch arg construction from server data + profile options, spawn steam -applaunch with correct flags, process detection loop
    status: completed
  - id: favorites-history
    content: Implement favorites add/remove, history tracking with configurable size, desktop entry (.desktop file) generation
    status: completed
  - id: cli-mode
    content: "Add clap CLI: dayz-ctl connect <ip> <port> for direct connect from desktop entries, plus --version, --help"
    status: completed
  - id: polish
    content: Error handling, tracing/logging, graceful degradation when Steam not running, offline mode support, self-update check
    status: completed
isProject: false
---

# Rewrite dayz-ctl in Rust with ratatui + steamworks-rs

## Current State

The project is a single ~1700-line Bash script ([dayz-ctl](dayz-ctl)) that acts as a DayZ server browser and launcher for Linux. It uses:
- **gum** (Charm) for menus, prompts, spinners
- **fzf** for fuzzy server list with live preview
- **SteamCMD** for workshop mod downloads
- **dayzsalauncher.com API** for the server list
- **Steam Web API** for player counts
- **jq** for JSON, **geoiplookup/whois** for geo, **ping** for latency

## Architecture

```mermaid
graph TD
    subgraph ui [TUI Layer - ratatui]
        MainMenu[Main Menu]
        ServerBrowser[Server Browser]
        ServerDetail[Server Detail]
        FilterPanel[Filter Panel]
        ConfigScreen[Config Screen]
        NewsView[News Viewer]
        DirectConnect[Direct Connect]
        ModManager[Mod Manager]
    end

    subgraph core [Core Layer]
        AppState[App State]
        Profile[Profile Manager]
        Config[Config Manager]
        Launch[Game Launcher]
    end

    subgraph data [Data Layer]
        SteamClient[steamworks-rs Client]
        ServerAPI[Server API - reqwest]
        NewsAPI[News API]
        BattleMetrics[BattleMetrics API]
        ModDB[Mod Database]
    end

    MainMenu --> ServerBrowser
    MainMenu --> ConfigScreen
    MainMenu --> NewsView
    MainMenu --> DirectConnect
    ServerBrowser --> ServerDetail
    ServerBrowser --> FilterPanel
    ServerDetail --> ModManager
    ServerDetail --> Launch

    AppState --> Profile
    AppState --> Config
    AppState --> SteamClient
    ModManager --> SteamClient
    ServerBrowser --> ServerAPI
    Launch --> SteamClient
```

## Key Design Decisions

- **Server list source**: Keep using the dayzsalauncher.com HTTP API -- it provides a comprehensive, pre-aggregated server list. steamworks-rs `matchmaking` only wraps lobbies, not `ISteamMatchmakingServers`. We can optionally add the `a2s` crate later for live per-server queries (real-time player count, ping).

- **Mod downloads**: Replace SteamCMD entirely with steamworks-rs UGC (User Generated Content) API. `client.ugc()` can subscribe to and download workshop items natively, which is cleaner than shelling out to steamcmd.

- **Fuzzy search**: Use the `nucleo` crate (same engine powering Helix editor's picker) to replace fzf's fuzzy matching in the server browser.

- **Async**: Use `tokio` for HTTP requests and background data loading. steamworks-rs is callback-based (call `client.run_callbacks()` periodically), so Steam operations run on a dedicated thread.

- **TUI pattern**: Use ratatui's standard architecture -- an `App` struct with screen enum, crossterm event loop, and per-screen render/input handlers.

## Project Structure

```
dayz-ctl-rs/
  Cargo.toml
  steam_appid.txt              # Contains "221100"
  src/
    main.rs                    # Entry point, arg parsing, terminal setup
    app.rs                     # App state machine, event loop
    event.rs                   # Input/tick event system
    config.rs                  # Config file (dayz-ctl.conf equivalent)
    profile.rs                 # Profile JSON (favorites, history, options)
    steam/
      mod.rs                   # Steam client init + callback thread
      workshop.rs              # UGC: subscribe, download, status
    api/
      mod.rs                   # HTTP client setup
      servers.rs               # dayzsalauncher.com server list fetch
      news.rs                  # dayz.com news API
      battlemetrics.rs         # BattleMetrics server lookup
    server/
      mod.rs
      types.rs                 # Server, ServerFilter, enums
      filter.rs                # Filter logic (map, mods, players, etc.)
    mods/
      mod.rs
      types.rs                 # Mod info types
      manager.rs               # Scan installed, symlink, remove
    launch.rs                  # Build args, spawn steam, wait for process
    ui/
      mod.rs                   # Screen enum, shared render helpers
      theme.rs                 # Colors, styles, borders
      main_menu.rs             # Main menu screen
      server_browser.rs        # Server list + fuzzy search + preview
      server_detail.rs         # Detail view after selecting server
      filter.rs                # Multi-select filter dialog
      config_screen.rs         # Config menu
      news.rs                  # News article list/viewer
      direct_connect.rs        # IP/port input dialog
      input.rs                 # Text input widget wrapper
      popup.rs                 # Confirm/alert dialogs
```

## Dependencies (Cargo.toml)

```toml
[package]
name = "dayz-ctl"
version = "0.3.0"
edition = "2024"

[dependencies]
ratatui = "0.29"
crossterm = "0.28"
steamworks = "0.12"
tokio = { version = "1", features = ["full"] }
reqwest = { version = "0.12", features = ["json"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
nucleo = "0.5"
clap = { version = "4", features = ["derive"] }
anyhow = "1"
thiserror = "2"
chrono = { version = "0.4", features = ["serde"] }
directories = "6"
tracing = "0.1"
tracing-subscriber = "0.3"
tui-input = "0.11"
open = "5"                    # xdg-open replacement
surge-ping = "0.8"            # ICMP ping
maxminddb = "0.25"            # GeoIP (optional, with bundled DB)
```

## Feature Mapping (Bash to Rust)

- **Main menu** (gum choose) --> ratatui `List` widget with highlight, keyboard nav
- **Server browser** (fzf + preview) --> Split layout: left panel with fuzzy-searchable `Table`, right panel with server detail. `nucleo` for matching.
- **Server filters** (gum choose --no-limit) --> Multi-select list with checkboxes, popup dialogs for value inputs
- **Favorites/History** --> Same server browser but filtered from `profile.json`
- **Direct connect** --> Text input dialog (IP + port fields)
- **Config** --> Menu list with sub-dialogs for each option
- **News** --> Scrollable text view with article list
- **Mod install** --> Progress bar/spinner during download via steamworks UGC
- **Game launch** --> `std::process::Command` to run `steam -applaunch 221100 ...`
- **Desktop entries** --> Write `.desktop` files with `std::fs`
- **GeoIP/Ping** --> `maxminddb` + `surge-ping` (or `dns-lookup` + raw socket)
- **Profile** --> serde structs, read/write JSON

## Implementation Phases

### Phase 1 -- Scaffolding and Core Types
Set up the Cargo project, define all core data types (Server, Mod, Profile, Config, LaunchOptions), implement config/profile persistence, and create the basic ratatui event loop with screen navigation.

### Phase 2 -- Data Layer
Implement server list fetching from dayzsalauncher.com API via reqwest, news API, mod database scanning (read `meta.cpp` from workshop folders), and caching with TTL.

### Phase 3 -- Steam Integration
Initialize steamworks-rs client, implement workshop item download/subscribe via UGC API, get player count, and handle the callback pump on a background thread.

### Phase 4 -- TUI Screens
Build all UI screens: main menu, server browser with fuzzy search and split-pane detail, filter panel, config screen, news viewer, direct connect dialog, mod status display.

### Phase 5 -- Game Launch and Mod Management
Implement mod symlink management (`@workshopId` symlinks in DayZ dir), build launch argument construction, process spawning via `steam -applaunch`, and process detection.

### Phase 6 -- Polish
Favorites/history management, desktop entry generation, offline mode (DayZCommunityOfflineMode), error handling, logging, and CLI mode (`dayz-ctl connect IP PORT`).
