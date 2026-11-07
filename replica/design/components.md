# Components (own design; roles measured from the NotchBox feature list, not copied)

Icons: SF Symbols (system, free to use in Mac apps). No NotchBox art. Font: system SF Pro, no font file. All copy written fresh.
Keyboard and VoiceOver apply to every component: real `Button`s, `.accessibilityLabel` on icon-only controls, visible focus ring (2pt accent).

NotchWindow
  variants  collapsed (matches hardware notch size), expanded (520x170), no-notch fallback (centred pill at top)
  states    idle, hover-open, drag-over (highlight border accent), closing, hidden in fullscreen video (setting)
  tokens    bg, radius notchBottom on bottom corners only, shadow panel, motion panel
  a11y      non-activating, never takes keyboard focus unless a text field is clicked; Esc closes
  used on   S01, S02

TabBar
  variants  icon-only, icon + label (setting)
  states    default, hover (surface-hover), selected (accent underline, text), disabled
  tokens    height tabBarHeight, text-muted -> text, space sm
  a11y      role tablist, arrow keys switch tabs, label = tab name
  used on   S02

DropZone
  variants  shelf, AirDrop
  states    idle (border dashed), targeted (accent border, surface-hover fill), rejected (danger border)
  tokens    radius lg, border, accent, danger
  a11y      announce "Drop to add to shelf" on targeted
  used on   S03

FileTile
  variants  file, image (thumbnail), folder, text, url, missing (file moved)
  states    default, hover (shows remove x), selected, dragging (60% opacity), missing (warning icon)
  tokens    size tile, radius md, surface, font xs for the name (2 lines, middle truncation)
  a11y      label = name + kind; Delete removes; Space opens Quick Look (could)
  used on   S03

ListRow
  variants  clipboard entry, calendar event, note, shortcut
  states    default, hover, pressed, empty
  tokens    surface-hover, font sm, space md
  a11y      one element, action = copy / open
  used on   S05 S06 S10

TimerDisplay
  variants  pomodoro (focus/break), countdown, stopwatch
  states    idle, running, paused, finished (success flash)
  tokens    font timer, accent for the running ring, success on finish
  a11y      announce time at start/stop only, not every second
  used on   S07

EmptyState
  variants  per tab
  states    n/a
  tokens    text-muted, font sm, one line plus one hint
  copy      written fresh, e.g. "Nothing here yet. Drag a file onto the notch."
  used on   S03 S05 S06 S10

PermissionPrompt
  variants  calendar, camera, automation (music)
  states    needed, denied (links to System Settings), granted
  tokens    surface, accent button, warning icon
  used on   S04 S05 S09

Button
  variants  primary (accent), secondary (surface), ghost, danger
  sizes     sm 24, md 32
  states    default, hover, pressed, focus-visible, disabled
  tokens    radius md, font sm/600, on-accent
  used on   S11, PermissionPrompt

Toggle / Picker / Stepper
  system SwiftUI controls in the Settings window, tinted with accent. Settings (S11) is a normal light/dark system window, not the black panel.
  used on   S11

Haptics: `NSHapticFeedbackManager.defaultPerformer.perform(.alignment)` on drop and on tab change.
