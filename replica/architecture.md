# Architecture: a native Mac notch hub (a rebuild of NotchBox's core features)

Personal use, one Mac, always on. Decision: **native macOS app**, not web. Everything the product does (notch window, system-wide drag detection, AirDrop, Now Playing, EventKit) is an OS feature a web stack cannot touch. Your machine already has Xcode 27.2 and Swift 6.4, and a notched display, so it can be built and tested here.

## Stack

| layer | choice | why |
| --- | --- | --- |
| language / UI | Swift + SwiftUI, AppKit for the window | SwiftUI for content, AppKit `NSPanel` for the notch overlay (SwiftUI alone cannot do it) |
| min OS | macOS 14 | original needs 13; 14 gives `@Observable` and simpler SwiftData/animations. Your Mac is far above it |
| window | borderless non-activating `NSPanel`, level `.statusBar`, `canJoinAllSpaces` + `fullScreenAuxiliary` | stays above all apps and Spaces without stealing focus |
| notch geometry | `NSScreen.safeAreaInsets.top` + `auxiliaryTopLeftArea/RightArea` | gives the real notch size; fallback to a fixed top-centre pill on Macs without a notch |
| drag detection | a transparent hot-zone panel at the notch that registers as a drag destination (`NSDraggingDestination`), plus a global `NSEvent` monitor on `.leftMouseDragged` to know a drag is in progress | public APIs only; global monitors need no special permission for mouse events |
| drag out | `NSDraggingSource` / SwiftUI `onDrag` with `NSItemProvider(fileURL)` | lets any app accept the file |
| AirDrop | `NSSharingService(named: .sendViaAirDrop)` | official; opens the system AirDrop picker |
| database | SQLite through GRDB (or SwiftData) | local, one file, no server |
| settings | `UserDefaults` / `@AppStorage` | |
| login item | `SMAppService.mainApp.register()` | "always on" after reboot, no helper app |
| menu bar | `MenuBarExtra` (or `NSStatusItem`) | |
| music | AppleScript / ScriptingBridge for Music and Spotify | the system MediaRemote framework is private, so it is out; `MPNowPlayingInfoCenter` only reports your own app. Needs the Automation permission. Browsers are a later "could" |
| calendar | EventKit | needs Calendars permission |
| clipboard | poll `NSPasteboard.general.changeCount` every ~0.5s | no push API exists |
| camera / network | AVFoundation / `NWPathMonitor` + `getifaddrs` byte counters | |
| auth, payments, email, jobs, hosting | none | single local user. Pro/paywall is out of scope (skip rows) |
| distribution | build and run from Xcode, or Developer ID sign + notarize if you want to move it | not the App Store, so no sandbox required; still use security-scoped bookmarks where cheap |

## Schema

Tables: 5 (shelf_items, clip_entries, notes, shortcuts, counters). Access rules: none needed (one local user, files in `~/Library/Application Support`). Full SQL in `schema.sql`.

Key decision, shelf storage: **reference by default, copy as an option.** Reference (bookmark) is instant and uses no disk but breaks if the file moves. Copy survives moves and restarts but duplicates large files. This also settles the open question from recon (does the shelf survive quit): here it is a setting, default keep.

## API (internal: no network)

| entry point | does | who | input | output | flow |
| --- | --- | --- | --- | --- | --- |
| `DragMonitor.start()` | watches global drags, opens the drop zone near the notch | app | mouse events | open/close event | F01 |
| `Shelf.add(urls)` | stores items | drop zone | `[URL]` / pasteboard items | `[ShelfItem]` | F01 |
| `Shelf.remove(id)` / `clear()` | deletes | UI | id | none | F01 |
| `Shelf.provider(for:)` | item provider for drag-out | UI | id | `NSItemProvider` | F02 |
| `AirDrop.share(items)` | opens the AirDrop picker | AirDrop target | `[URL]` | none | F03 |
| `TabHost` | switches widget tabs | UI | tab id | view | F04 |
| `Settings` | read/write prefs, login item | settings window | key/value | none | F05 |

Webhooks: none. Jobs: clipboard poller (0.5s), music poller (1s while the Music tab is visible), cleanup of old clipboard entries (on launch, keep last 200).

## The parts that bite

- **Notch window:** it must sit in the menu-bar band without being pushed down, and survive display changes (plug/unplug, resolution, fullscreen apps, Spaces). Recompute the frame on `NSApplication.didChangeScreenParametersNotification`.
- **Drag detection:** a drag from another app only shows up as mouse events; only the panel under the cursor gets the drop. Expand the hot zone as soon as a drag is detected, then let it collapse after a timeout.
- **Focus:** the panel must not become key or activate the app, or it steals typing from the user's work (`.nonactivatingPanel`).
- **Permissions:** Calendar, Camera, Automation (Music/Spotify) each prompt once; denied states are screens too (S05, S09 recon rows).
- **File references:** security-scoped bookmarks go stale; show a "missing" tile and let the user remove it.
- **Clipboard:** may contain passwords. Respect `org.nspasteboard.ConcealedType`/`TransientType` and skip those entries; give an ignore-app list.
- **Login item and "always on":** `SMAppService` can be switched off in System Settings; also restart the app if it crashes (a launchd `KeepAlive` agent if you want that).
- **Time zones, idempotency, races:** none apply (no server). Clipboard writes and shelf drops from different threads: keep DB writes on one actor.

## Build order

1. **Vertical slice** (S01, S02, S03, F01 to F03; tables `shelf_items`; no routes, 5 types): menu bar app, notch panel that opens on drag, drop a file, see the tile, drag it out, AirDrop it. Proves the window, drag in, drag out. This is also the "S weekend" version.
2. **Must-haves:** launch at login, keep items across restarts, remove items, no-notch fallback, settings window (S11).
3. **Should-haves**, one tab at a time, in this order: clipboard (S06), timers (S07), quick tools (S10), calendar (S05), music (S04).
4. **Could-haves:** Quick Look, mini browser (S08), system tools (S09), languages.
5. **Fixes** from `/replica-entrepreneur`: little to find for a personal clone, so this step can be skipped. `/replica-brand` is also optional if you never share it.
