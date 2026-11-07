import AppKit
import SwiftUI
import Combine

/// Borderless panel that never takes focus.
final class NotchPanel: NSPanel {
    override var canBecomeKey: Bool { false }
    override var canBecomeMain: Bool { false }
}

@MainActor
final class NotchPanelController {
    let store = ShelfStore()
    let nowPlaying = NowPlaying()
    let clipboard = ClipboardStore()
    let monitor = SystemMonitor()
    let calendar = CalendarStore()
    let converter = ConvertModel()
    let news = NewsStore()
    let docker = DockerStore()
    let lyrics = LyricsStore()
    private var cancellables = Set<AnyCancellable>()
    private let state = PanelState()
    private var geo = NotchGeometry.current()
    private var homeGeo = NotchGeometry.current()
    private let pets = PetController()
    private var balls: [NSPanel] = []
    private var ballScreens: [NSScreen] = []
    private var panel: NotchPanel!
    private var monitors: [Any] = []
    private var hoverTimer: Timer?
    private var dragCountAtMouseDown = 0
    private var fileDragActive = false

    func start() {
        buildPanel()
        rebuildBalls()
        pets.start(geo: homeGeo, nowPlaying: nowPlaying)
        Settings.shared.$ballsEnabled.receive(on: DispatchQueue.main).sink { [weak self] _ in MainActor.assumeIsolated { self?.rebuildBalls() } }.store(in: &cancellables)
        // keep the window the same size as the panel when the tab or the mirror size changes
        state.$tab.receive(on: DispatchQueue.main).sink { [weak self] _ in MainActor.assumeIsolated { self?.updateFrame() } }.store(in: &cancellables)
        Settings.shared.$mirrorLevel.receive(on: DispatchQueue.main).sink { [weak self] _ in MainActor.assumeIsolated { self?.updateFrame() } }.store(in: &cancellables)
        NotificationCenter.default.publisher(for: UserDefaults.didChangeNotification).receive(on: DispatchQueue.main)
            .sink { [weak self] _ in MainActor.assumeIsolated {
                guard let self else { return }
                self.updateFrame()
                if UserDefaults.standard.bool(forKey: "musicShowLyrics") { self.lyrics.load(for: self.nowPlaying.track) }
                self.lyrics.refreshLive()
            } }.store(in: &cancellables)
        // with lyrics on, fetch them for each new song even if the Music tab is closed
        nowPlaying.$track.receive(on: DispatchQueue.main).sink { [weak self] t in MainActor.assumeIsolated {
            guard let self, UserDefaults.standard.bool(forKey: "musicShowLyrics") else { return }
            self.lyrics.load(for: t)
            self.lyrics.refreshLive()   // new position reading: re-sync the ribbon
        } }.store(in: &cancellables)
        lyrics.positionProvider = { [weak self] in
            guard let self else { return (0, false) }
            return MainActor.assumeIsolated { (self.nowPlaying.estimatedPosition, self.nowPlaying.isPlaying) }
        }
        // the system audio is only listened to while the beat option is on AND something is playing
        Settings.shared.$haloBeat.receive(on: DispatchQueue.main).sink { [weak self] _ in MainActor.assumeIsolated { self?.updateAudio() } }.store(in: &cancellables)
        nowPlaying.$track.map { $0?.isPlaying == true }.removeDuplicates().receive(on: DispatchQueue.main)
            .sink { [weak self] _ in MainActor.assumeIsolated { self?.updateAudio() } }.store(in: &cancellables)
        nowPlaying.start()
        clipboard.start()
        monitor.start()
        calendar.start()
        news.start()
        docker.start()
        // Song change alert: only when the notch is closed and the title really changed.
        nowPlaying.$track.removeDuplicates { $0?.title == $1?.title && $0?.app == $1?.app }
            .scan((nil, nil) as (Track?, Track?)) { ($0.1, $1) }
            .sink { [weak self] old, new in
                guard let self, let old, let new, new.isPlaying, !new.title.isEmpty, old.title != new.title,
                      Settings.shared.alertsSong, !self.state.expanded else { return }
                AlertCenter.shared.post(symbol: "music.note", title: new.title, detail: new.artist, artwork: new.artworkURL)
            }.store(in: &cancellables)
        installMonitors()
        NotificationCenter.default.addObserver(forName: NSApplication.didChangeScreenParametersNotification,
                                               object: nil, queue: .main) { [weak self] _ in
            MainActor.assumeIsolated { self?.screensChanged() }
        }
    }

    // MARK: window
    private var currentSize: CGSize { geo.panelSize(tab: state.tab, mirrorLevel: Settings.shared.mirrorLevel) }
    private var currentWindowRect: NSRect { geo.windowRect(for: currentSize, collapsed: !state.expanded) }
    private func updateAudio() {
        if Settings.shared.haloBeat && nowPlaying.isPlaying { AudioLevel.shared.start() } else { AudioLevel.shared.stop() }
    }
    func updateFrame() { panel.setFrame(currentWindowRect, display: true) }

    private func buildPanel() {
        panel = NotchPanel(contentRect: currentWindowRect, styleMask: [.borderless, .nonactivatingPanel],
                           backing: .buffered, defer: false)
        panel.level = .statusBar
        panel.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .stationary, .ignoresCycle]
        panel.isOpaque = false
        panel.backgroundColor = .clear
        panel.hasShadow = false
        panel.isMovable = false
        panel.ignoresMouseEvents = true   // collapsed: clicks pass through
        panel.contentView = NSHostingView(rootView: NotchRootView(state: state, store: store, nowPlaying: nowPlaying, clipboard: clipboard, monitor: monitor, calendar: calendar, converter: converter, news: news, docker: docker, lyrics: lyrics, geo: geo))
        updateFrame()
        panel.orderFrontRegardless()
    }

    private func screensChanged() {
        if state.expanded { close() }
        homeGeo = NotchGeometry.current()
        useGeo(homeGeo)
        rebuildBalls()
        pets.reconfigure(homeGeo)
    }

    private func useGeo(_ g: NotchGeometry) {
        geo = g
        panel.contentView = NSHostingView(rootView: NotchRootView(state: state, store: store, nowPlaying: nowPlaying, clipboard: clipboard, monitor: monitor, calendar: calendar, converter: converter, news: news, docker: docker, lyrics: lyrics, geo: geo))
        updateFrame()
    }

    /// One ball per screen other than the notch screen.
    private func rebuildBalls() {
        balls.forEach { $0.orderOut(nil) }
        balls = []; ballScreens = []
        guard Settings.shared.ballsEnabled else { return }
        for s in NSScreen.screens where s != homeGeo.screen {
            balls.append(Ball.makePanel(for: s, nowPlaying: nowPlaying))
            ballScreens.append(s)
        }
        updateBallVisibility()
    }
    private func updateBallVisibility() {
        for (b, s) in zip(balls, ballScreens) { b.alphaValue = (state.expanded && geo.screen == s) ? 0 : 1 }
    }
    /// Opens the panel on another screen (from its ball) and moves it back home when it closes.
    private func openOn(_ screen: NSScreen) {
        useGeo(NotchGeometry(screen: screen))
        open(pinned: true)
        updateBallVisibility()
    }

    // MARK: open / close
    private var graceUntil = Date.distantPast
    private var dwell = 0.0, outside = 0.0

    func open(forDrag: Bool = false, pinned: Bool = false) {
        outside = 0
        if pinned { graceUntil = Date().addingTimeInterval(3) }   // opened from the menu: cursor is far away
        guard !state.expanded else { return }
        if forDrag { state.tab = .shelf }
        else if Settings.shared.rememberTab, let t = NotchTab(rawValue: Settings.shared.lastTab), t != .mirror, t != .docker || docker.installed { state.tab = t }   // never reopen on the camera by itself
        else { state.tab = nowPlaying.isPlaying ? .music : .shelf }
        state.expanded = true
        pets.suppressed = true
        updateFrame()
        nowPlaying.fast = true; nowPlaying.refresh()
        panel.ignoresMouseEvents = false
        NSHapticFeedbackManager.defaultPerformer.perform(.alignment, performanceTime: .default)
    }
    func close() {
        guard state.expanded else { return }
        state.expanded = false
        pets.suppressed = false
        nowPlaying.fast = false
        panel.ignoresMouseEvents = true
        dwell = 0; outside = 0
        if geo.screen != homeGeo.screen { useGeo(homeGeo) } else { updateFrame() }
        updateBallVisibility()
    }
    func toggle() { state.expanded ? close() : open(pinned: true) }

    /// Polls the cursor 12 times a second. Simpler and more reliable than reacting to
    /// mouse-move events, which stop arriving while the cursor is over our own window.
    private func tick() {
        let p = NSEvent.mouseLocation
        let step = 0.08
        // Menu bar hidden means an app is full screen: stay out of the way if the user wants that.
        let fullscreen = geo.screen.visibleFrame.maxY >= geo.screen.frame.maxY - 1
        let hide = Settings.shared.hideInFullscreen && fullscreen
        panel.alphaValue = hide ? 0 : 1
        if hide { if state.expanded { close() }; return }
        if !state.expanded {
            if geo.notchRect.contains(p) { dwell += step; if dwell >= Settings.shared.openDelay { open() } }
            else if let s = ballScreens.first(where: { Ball.hitRect(on: $0).contains(p) }) {
                dwell += step; if dwell >= Settings.shared.openDelay { dwell = 0; openOn(s) }
            } else { dwell = 0 }
            return
        }
        let held = fileDragActive && NSEvent.pressedMouseButtons & 1 != 0
        if held || Date() < graceUntil || geo.rect(for: currentSize).insetBy(dx: -4, dy: -4).contains(p) {
            outside = 0
        } else {
            outside += step
            if outside >= 0.25 { close() }
        }
    }

    // MARK: global mouse monitoring (no permission needed for mouse events)
    private func installMonitors() {
        hoverTimer = Timer.scheduledTimer(withTimeInterval: 0.08, repeats: true) { [weak self] _ in
            MainActor.assumeIsolated { self?.tick() }
        }
        let dragPB = NSPasteboard(name: .drag)
        let handler: (NSEvent) -> Void = { [weak self] event in
            MainActor.assumeIsolated {
                guard let self else { return }
                let p = NSEvent.mouseLocation
                switch event.type {
                case .leftMouseDown:
                    self.dragCountAtMouseDown = dragPB.changeCount
                case .leftMouseDragged:
                    // a real file drag changes the drag pasteboard after mouse-down
                    let hasFiles = dragPB.canReadObject(forClasses: [NSURL.self],
                                                        options: [.urlReadingFileURLsOnly: true])
                    if hasFiles && dragPB.changeCount != self.dragCountAtMouseDown { self.fileDragActive = true }
                    if self.fileDragActive && !self.state.expanded {
                        if self.geo.hotRect.contains(p) { self.open(forDrag: true) }
                        else if let s = self.ballScreens.first(where: { Ball.hitRect(on: $0).insetBy(dx: -30, dy: -20).contains(p) }) {
                            self.useGeo(NotchGeometry(screen: s)); self.open(forDrag: true); self.updateBallVisibility()
                        }
                    }
                case .leftMouseUp:
                    self.fileDragActive = false
                default: break
                }
            }
        }
        if let m = NSEvent.addGlobalMonitorForEvents(matching: [.leftMouseDown, .leftMouseDragged, .leftMouseUp],
                                                      handler: handler) { monitors.append(m) }
        if let m = NSEvent.addLocalMonitorForEvents(matching: [.keyDown, .leftMouseUp]) { [weak self] e in
            MainActor.assumeIsolated {
                if e.type == .keyDown, e.keyCode == 53 { self?.close() }
                if e.type == .leftMouseUp { self?.fileDragActive = false }
            }
            return e
        } { monitors.append(m) }
    }
}
