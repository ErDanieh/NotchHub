# Recon map: NotchBox - Easier Drag & Drop (macOS)

Scope: the notch shelf (drag files in, hold, drag out, AirDrop) plus the small widget tabs that live in the same notch panel
For: the user's own Mac, local, always on (personal use, not for sale)
Date: 2026-11-07

## Sources

| # | source | URL | notes |
| --- | --- | --- | --- |
| 1 | App Store listing | https://apps.apple.com/vn/app/notchbox-easier-drag-drop/id6737410946?mt=12 | description, feature list, version history. Screenshots not readable (placeholders) |
| 2 | Developer help page (Notion) | https://decisive-store-bd5.notion.site/NotchBox-12be1846c58e80e38078c84d8a4a79a2 | renders with JavaScript, nothing readable. A search snippet says shelf files are cleared when the app quits (unverified) |
| 3 | Privacy policy / terms | https://decisive-store-bd5.notion.site/Privacy-Policy-12be1846c58e80bbb2d5e8d4addee834 | not read |
| 4 | Comparable apps (public listings) | Dropover, NotchNook, NotchShelf (open source, macOS 14+), Droppy | for patterns the original does not document |
| 5 | The user's own copy | n/a | NOT yet used. Needed for exact states and screens (see gaps) |

Gaps: no screenshots or videos are available, so screens below are inferred from the feature list. Step 1 for the next pass: the user installs the free trial and records or screenshots each tab into `replica/screens/`.

## Core loop

Drag a file toward the notch, it opens a drop zone, you drop it, it stays there, and you drag it out later into any app (or onto AirDrop).

## Screens

| ID | screen | route / how to reach | purpose | key components | states seen |
| --- | --- | --- | --- | --- | --- |
| S01 | Notch collapsed | always present at the notch | idle pill/notch overlay | notch-shaped window, now-playing hint | idle, music playing (inferred) |
| S02 | Notch expanded (hub) | hover or click the notch | tab host for all widgets | tab bar, content area | open, closed |
| S03 | File shelf | tab, or auto-open on drag | hold files | file tiles, drop zone, AirDrop target | empty, filled, drag-over (inferred) |
| S04 | Music | tab | album art, controls | art, beat animation, play/pause | no player, playing |
| S05 | Calendar | tab | day schedule, month view | event list, month grid | no events, events, no permission |
| S06 | Clipboard history | tab | recent copies | list, copy-back | empty, filled |
| S07 | Timers | tab | Pomodoro, countdown, stopwatch | timer display, start/stop | idle, running |
| S08 | Mini browser | tab | search without switching | web view, search field | blank, loaded |
| S09 | System tools | tab | network speed, camera mirror | charts, camera view | no permission, live |
| S10 | Quick tools | tab | notes, water tracker, counter, days-left, bookmarks, app shortcuts | lists, buttons | empty, filled |
| S11 | Settings | menu bar icon | colours, positions, shortcuts, upgrade | forms | free, Pro |
| S12 | Pro upgrade | settings | in-app purchase | paywall | n/a (skip, see out of scope) |

## Flows

```
F01 Store a file on the shelf
    S01 -> (drag file near notch) S03 -> drop
    happy path clicks: 0 (drag and drop only)
    edge: many files, folders, very large files, same file twice, app quits (are files kept?)

F02 Move a file out of the shelf
    S02 -> S03 -> drag tile into any app/Finder
    happy path clicks: 1 (open) + 1 drag
    edge: file moved/deleted on disk, drag out to a full-screen app

F03 Share a file with AirDrop
    S03 -> drag tile onto AirDrop target
    edge: no AirDrop devices

F04 Run a timer / read the clipboard / check the calendar
    S01 -> S02 -> choose tab (S05..S10)
    edge: calendar permission denied, camera permission denied

F05 Change settings
    menu bar icon -> S11
```

## Components

| component | variants | states | used on |
| --- | --- | --- | --- |
| Notch window | collapsed, expanded | idle, hover, dragging-over, closing | all |
| Tab bar | icon tabs | selected, hover | S02 |
| File tile | file, image, folder | normal, selected, dragging | S03 |
| Drop zone | shelf, AirDrop | idle, targeted | S03 |
| List row | clipboard, event, note | normal, hover | S05 S06 S10 |
| Timer | pomodoro, countdown, stopwatch | idle, running, finished | S07 |
| Haptic feedback | n/a | on drop, on select (v1.1.47) | S03 |

## Inferred data model

```
ShelfItem   id, file_url (or bookmark), name, type, added_at, pinned
            evidence: F01, F02, store description "temporary or frequent storage"
            confidence: medium (whether it copies or references files is unknown)
ClipEntry   id, text/image, copied_at
            evidence: S06 feature "recent history"          confidence: medium
Note        id, text, created_at                             confidence: guess
Timer       kind, duration, started_at                       confidence: guess
Shortcut    id, app_bundle_id, order                         confidence: medium
Settings    colours, tab order, clipboard position, calendar view (month/day), launch at login
            evidence: changelog 1.1.51, 1.1.53, 1.1.64       confidence: medium
```

Relationships: Settings 1-1 app; ShelfItem, ClipEntry, Note, Shortcut each belong to the single local user. No server, no accounts.

## Feature matrix

See `features.csv`. Must: 8, should: 9, could: 6, skip: 3.

## Out of scope (cannot or should not be cloned)

- Their name, icon, artwork, copy and screenshots. The clone gets its own.
- Pro purchase and licensing. Your own copy is free for you.
- Music-service integrations that need partner deals or private APIs. Use the system Now Playing info and public APIs only.
- Translations to 17 languages (could, later).

## Size

Screens 12, flows 5, entities 6. Hard parts: (1) a borderless always-on-top window shaped around the hardware notch that also works on Macs without a notch, (2) catching drags system-wide to open the drop zone, (3) now-playing for Apple Music/Spotify/browsers without private APIs. Size: M (a few weeks for the full hub), S (a weekend) for only the shelf plus AirDrop.

Suggested build: ship the shelf first (S01, S02, S03, F01-F03), then add tabs one at a time.
