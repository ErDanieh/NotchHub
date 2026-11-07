<div align="center">

<img src="docs/poster.jpg" alt="NotchHub" width="720">

# NotchHub

### Give your MacBook's notch a job.

A tiny, always-on menu-bar app that turns the notch into a shelf, a music player, a clipboard, a calendar and more.<br>
Hover it. Drop a file on it. A pixel pet lives next to it.

![macOS 14+](https://img.shields.io/badge/macOS-14%2B-000?logo=apple&logoColor=white)
![Swift](https://img.shields.io/badge/Swift-5.9-F05138?logo=swift&logoColor=white)
![Native](https://img.shields.io/badge/100%25-native%20SwiftUI-3DA9FF)
![No tracking](https://img.shields.io/badge/telemetry-none-2ea44f)

<img src="docs/demo.gif" alt="NotchHub demo" width="720">

[Watch the full 25 s demo](docs/demo.mp4)

</div>

---

## Install in 60 seconds

You need a Mac with macOS 14 or newer and the Xcode command line tools (`xcode-select --install`).

```bash
git clone https://github.com/ErDanieh/NotchHub.git
cd NotchHub
./Scripts/build-app.sh
```

That is all. The script builds the app, installs it in `~/Applications/NotchHub.app` and opens it.
Run it again any time to update: it replaces the old copy by itself.

Hover the notch (or the little ball on your other screens) and it opens. To start it with your Mac, use the menu bar icon, then **Launch at login**.

> Optional: `brew install ffmpeg` adds MP3 output to the converter.

## What it does

| | |
|---|---|
| **Shelf** | Drop files on the notch and pick them up anywhere. AirDrop card, image previews. Hold a file and hover a tab to switch to it. |
| **Music** | Apple Music, Spotify and YouTube / YouTube Music in Chrome. Play, pause, skip, a real progress bar you can drag. |
| **Lyrics** | Synced lyrics in the Music tab and, if you want, line by line under the closed notch. |
| **Halo** | A neon glow around the notch while music plays. Your colour, intensity and speed. Can pulse to the beat, or stay on all the time. |
| **Clipboard** | History of what you copied. Passwords from password managers are ignored. Pin items, copy them again. |
| **Calendar** | Today and the next days, month view, and a heads-up before each event. |
| **Mirror** | Your camera in the notch, resizable. The camera is only on while the tab is open. |
| **System** | CPU, memory, network, battery, disk and the apps using the most. Alerts when something spikes. |
| **Converter** | Images (JPG, PNG, HEIC...), PDF to images and back, audio and video. Plus an image **weight reducer**: type the size you want and it does the rest. |
| **News** | Tech, AI and markets headlines in Spanish. |
| **Docker** | Start, stop and restart your containers. Only shows up if Docker is installed. |
| **Pets** | A pixel pet walks along the menu bar, sleeps, peeks out from under the notch and dances when music plays. Nine to choose from. |
| **Other screens** | A small ball on every screen without a notch. Hover it and the full panel opens there. |

Every tab can be reordered or hidden from the menu bar icon (**Reorder tabs...**).

## Pets

Clawd (an orange block critter), Cat, Ghost, Robot, Slime, Frog, Space invader, Penguin and Bunny.
Pick one in **Settings, Pet**, and choose its size.

## Permissions

NotchHub asks only for what a feature needs, when you use it:

| Permission | Used for |
|---|---|
| Automation (Music, Spotify, Chrome) | Reading and controlling playback |
| Calendars | The calendar tab and event alerts |
| Camera | The mirror tab |
| System Audio Recording | Only if you turn on "halo follows the beat". Nothing is recorded or saved |

For YouTube in Chrome, enable **View, Developer, Allow JavaScript from Apple Events**.

Everything runs on your Mac. No account, no analytics, no servers of ours. Lyrics come from [LRCLIB](https://lrclib.net) and news from public RSS feeds.

## Settings

Menu bar icon, then **Settings...** Halo colour and intensity, pulse speed, open delay, ball and pet options, alert types and thresholds, lyric timing, hide in full screen.

## Build and hack

```bash
swift build -c release        # compile only
./Scripts/build-app.sh        # build, sign, install, relaunch
```

Pure SwiftPM, no Xcode project. The code is in `Sources/NotchHub`: one file per feature (`Halo.swift`, `Lyrics.swift`, `Pets.swift`, `Converter.swift`...).
If you have an Apple Development certificate the script signs with it, so macOS keeps your permissions between builds. Otherwise it signs ad hoc.

The planning notes (feature map, architecture, build log) are in [`replica/`](replica).

## Credits

Inspired by NotchBox on the Mac App Store. This is an independent, from-scratch rewrite for personal use and shares no code or assets with it. Clawd-style pixel pets are original drawings.

---

<details>
<summary><b>Leer en español</b></summary>

**NotchHub** le da trabajo al notch de tu MacBook: estantería de archivos con AirDrop, música con letras y halo al ritmo, portapapeles, calendario, espejo, rendimiento, conversor de archivos, noticias, Docker y mascotas pixel art.

**Instalar** (macOS 14+ y las herramientas de línea de comandos de Xcode):

```bash
git clone https://github.com/ErDanieh/NotchHub.git
cd NotchHub
./Scripts/build-app.sh
```

Se instala en `~/Applications` y se abre solo. Pasa el ratón por el notch para abrirlo. Para que arranque con el Mac: icono de la barra de menús, **Launch at login**.

</details>
