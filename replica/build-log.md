# Build log

| ID | date | status | missing / notes |
| --- | --- | --- | --- |
| shell | 2026-11-07 | partial | SwiftPM app (`NotchHub/`), menu bar icon, notch panel, tokens wired. Compiles; launches (process stayed alive 3s). Not seen on screen: the one screenshot attempt captured the wrong content and was deleted |
| S01 collapsed | 2026-11-07 | partial | black shape sized to the notch, click-through. Notch size from `safeAreaInsets` + auxiliary areas, fallback 180x24 |
| S02 expanded | 2026-11-07 | partial | opens on hover over notch or file drag near it; closes 0.45s after the cursor leaves; Esc closes. Only one tab (shelf), no tab bar yet |
| S03 shelf | 2026-11-07 | partial | empty state, tiles with icon, hover remove, missing-file tile, drop zone highlight, AirDrop target |
| F01 F02 F03 | 2026-11-07 | partial | written, NOT tested with a real drag |

Harder than expected / known risks:
- Data layer is a JSON file, not SQLite yet (same function names: add/remove/clear). Fine for the shelf; swap when clipboard history is added.
- Drag detection compares the drag pasteboard `changeCount` after mouse-down. This is the riskiest guess and the first thing to check by hand.
- `SMAppService` login item only works from the packaged `.app`, and macOS may ask you to approve it in System Settings > Login Items.
- Ad-hoc signed (`codesign -s -`), fine on this Mac. Gatekeeper will complain if you copy it to another Mac.
- The folder `Sources/NotchHub/Tokens.swift` is a copy of `replica/design/Tokens.swift`.

## 2026-11-07 phases 1 and 2
- Phase 1 done and confirmed by the user: progress bar and seek, Settings window, configurable halo, stable signing (revoked cert skipped).
- Phase 2 built, compiles, idle CPU about 1%: clipboard tab, system tab (CPU, memory, network, battery, disk, busiest processes), alerts shown as a widened notch (song change, high CPU/memory, low battery). Not yet confirmed by the user.
- Idle cost: NowPlaying polls every 3s with the panel closed, 1s when open.
- Phase 3 pending: calendar, mirror, file converter.

## Phase 3 (2026-11-07)
Calendar tab (EventKit, month grid, reminders), Mirror tab (AVFoundation, only runs while visible), Convert tab (ImageIO, PDFKit, AVFoundation; MP3 via ffmpeg). Converter engine tested headless with real files: JPG, HEIC, TIFF, images to PDF, PDF to JPG pages, compress PDF, M4A, MP3 all produced valid files. UI and permission prompts still need a manual check.
