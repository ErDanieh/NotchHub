import SwiftUI

enum NotchTab: String, CaseIterable {
    case shelf = "tray.and.arrow.down", music = "music.note", clipboard = "doc.on.clipboard", system = "cpu",
         calendar = "calendar", mirror = "camera", convert = "arrow.triangle.2.circlepath",
         news = "newspaper", docker = "shippingbox"
    var label: String {
        switch self { case .shelf: "Shelf"; case .music: "Music"; case .clipboard: "Clipboard"; case .system: "System"
            case .calendar: "Calendar"; case .mirror: "Mirror"; case .convert: "Convert"; case .news: "News"; case .docker: "Docker" }
    }
}

final class PanelState: ObservableObject {
    @Published var expanded = false
    @Published var dragHover = false
    @Published var tab: NotchTab = .shelf
}

struct NotchRootView: View {
    @ObservedObject var state: PanelState
    @ObservedObject var store: ShelfStore
    @ObservedObject var nowPlaying: NowPlaying
    @ObservedObject var clipboard: ClipboardStore
    @ObservedObject var monitor: SystemMonitor
    @ObservedObject var calendar: CalendarStore
    @ObservedObject var converter: ConvertModel
    @ObservedObject var news: NewsStore
    @ObservedObject var docker: DockerStore
    @ObservedObject var lyrics: LyricsStore
    @ObservedObject var alerts = AlertCenter.shared
    @ObservedObject var settings = Settings.shared
    let geo: NotchGeometry
    @AppStorage("musicShowLyrics") private var lyricsOpen = false

    /// Collapsed: the small corner radius of the hardware notch, so the glow hugs it.
    /// Expanded: the rounder panel.
    private var shape: UnevenRoundedRectangle {
        let r = state.expanded ? Tok.Radius.notchBottom : Tok.Radius.notchCollapsed
        return UnevenRoundedRectangle(bottomLeadingRadius: r, bottomTrailingRadius: r)
    }

    var body: some View {
        let toast = !state.expanded ? alerts.current : nil
        let size = geo.panelSize(tab: state.tab, mirrorLevel: settings.mirrorLevel)
        let isSong = toast?.symbol == "music.note"
        let live = !state.expanded && toast == nil && liveLyrics          // lyrics running under the closed notch
        let extra: CGFloat = toast == nil ? (live ? 2 * Tok.Layout.toastWing : 0) : (isSong ? Tok.Layout.songWing : 2 * Tok.Layout.toastWing)
        let w = state.expanded ? size.width : geo.notchSize.width + extra
        let h = state.expanded ? size.height : geo.notchSize.height + (live ? Tok.Layout.liveLyricsHeight : 0)
        // a song alert grows only to the right of the notch, so the shape is shifted by half its extra width
        let shift: CGFloat = (!state.expanded && isSong) ? Tok.Layout.songWing / 2 : 0
        VStack(spacing: 0) {
            ZStack(alignment: .top) {
                if let toast {
                    toastView(toast).transition(.opacity)
                }
                if live {
                    LiveLyricLine(lyrics: lyrics)
                        .padding(.top, geo.notchSize.height).transition(.opacity)
                }
                if state.expanded {
                    VStack(spacing: Tok.Space.xs) {
                        TabBar(state: state, showDocker: docker.installed)
                        Group {
                            switch state.tab {
                            case .shelf: ShelfTab(state: state, store: store)
                            case .music: MusicTab(nowPlaying: nowPlaying, lyrics: lyrics)
                            case .clipboard: ClipboardTab(store: clipboard)
                            case .system: SystemTab(m: monitor)
                            case .calendar: CalendarTab(cal: calendar)
                            case .mirror: MirrorTab()
                            case .convert: ConvertTab(model: converter, shelf: store)
                            case .news: NewsTab(news: news)
                            case .docker: DockerTab(docker: docker)
                            }
                        }
                    }
                    .padding(.top, geo.notchSize.height + Tok.Space.xs)
                    .padding([.horizontal, .bottom], Tok.Space.lg)
                    .transition(.opacity)
                }
            }
            .frame(width: w, height: h, alignment: .top)
            .background(shape.fill(Tok.Color.bg))
            .background { halo(w: w, h: h) }      // behind the black shape: the glow only shows outside it
            .offset(x: shift)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .top)
        .animation(Tok.Motion.panel, value: state.expanded)
        .animation(Tok.Motion.panel, value: alerts.current)
        .animation(Tok.Motion.panel, value: settings.mirrorLevel)
        .animation(Tok.Motion.panel, value: lyricsOpen)
        .animation(Tok.Motion.panel, value: liveLyrics)
        .animation(Tok.Motion.panel, value: state.tab)
        .animation(.easeInOut(duration: 0.4), value: nowPlaying.isPlaying)
    }

    /// Synced lyrics of the current song, when the user has the lyrics switched on in the Music tab.
    private var liveLines: [LyricLine] {
        if case .ready(let l) = lyrics.state { return l.synced }
        return []
    }
    private var liveLyrics: Bool { lyricsOpen && nowPlaying.isPlaying && !liveLines.isEmpty }

    /// Dynamic-Island style, one line on each side of the notch.
    /// Songs: cover on the left, title over artist on the right. Other alerts: icon and title left, detail right.
    private func toastView(_ a: NotchAlert) -> some View {
        if a.symbol == "music.note" { return AnyView(songToast(a)) }
        let wing = Tok.Layout.toastWing - Tok.Space.md
        return AnyView(HStack(spacing: 0) {
            HStack(spacing: Tok.Space.sm) {
                Image(systemName: a.symbol).foregroundStyle(settings.haloColor)
                Text(a.title).font(Tok.Font.sm.weight(.semibold)).foregroundStyle(Tok.Color.text).lineLimit(1)
            }.frame(width: wing, alignment: .trailing)
            Spacer().frame(width: geo.notchSize.width + Tok.Space.md)
            Text(a.detail).font(Tok.Font.sm).foregroundStyle(Tok.Color.textMuted).lineLimit(1)
                .frame(width: wing, alignment: .leading)
        }
        .frame(height: geo.notchSize.height)
        .accessibilityElement(children: .combine)
        .accessibilityLabel("\(a.title). \(a.detail)"))
    }

    /// Song change: everything on the right of the notch. Cover, then title over artist.
    private func songToast(_ a: NotchAlert) -> some View {
        HStack(spacing: 0) {
            Spacer().frame(width: geo.notchSize.width)
            HStack(spacing: Tok.Space.sm) {
                Group {
                    if let u = a.artwork {
                        AsyncImage(url: u) { $0.resizable().scaledToFill() } placeholder: { Tok.Color.surfaceHover }
                    } else { ZStack { Tok.Color.surfaceHover; Image(systemName: "music.note").foregroundStyle(settings.haloColor) } }
                }
                .frame(width: 24, height: 24).clipShape(RoundedRectangle(cornerRadius: 5))
                VStack(alignment: .leading, spacing: 0) {
                    Text(a.title).font(.system(size: 12, weight: .semibold)).foregroundStyle(Tok.Color.text).lineLimit(1)
                    Text(a.detail).font(.system(size: 10)).foregroundStyle(Tok.Color.textMuted).lineLimit(1)
                }
                Spacer(minLength: 0)
            }
            .padding(.horizontal, Tok.Space.md)
            .frame(width: Tok.Layout.songWing, alignment: .leading)
        }
        .frame(height: geo.notchSize.height)
        .accessibilityElement(children: .combine)
        .accessibilityLabel("\(a.title). \(a.detail)")
    }

    /// Neon glow around the notch while something is playing (colour, intensity and speed from Settings).
    @ViewBuilder private func halo(w: CGFloat, h: CGFloat) -> some View {
        if (nowPlaying.isPlaying || settings.haloAlways) && settings.haloEnabled {
            // Behind the black shape, so the glow only shows around the notch, never inside it.
            HaloLayer(color: NSColor(settings.haloColor), intensity: settings.haloIntensity,
                      pulseSeconds: settings.pulseSeconds,
                      cornerRadius: state.expanded ? Tok.Radius.notchBottom : Tok.Radius.notchCollapsed,
                      beat: settings.haloBeat && nowPlaying.isPlaying)
                .frame(width: w, height: h)
            .allowsHitTesting(false)
            .transition(.opacity)
        }
    }
}

struct TabBar: View {
    @ObservedObject var state: PanelState
    var showDocker: Bool          // the Docker tab only exists if docker is installed
    @ObservedObject var settings = Settings.shared
    var body: some View {
        HStack(spacing: Tok.Space.xs) {
            ForEach(settings.visibleTabs(dockerInstalled: showDocker), id: \.self) { TabButton(tab: $0, state: state) }
        }
        .frame(height: Tok.Layout.tabBarHeight)
    }
}

/// A tab icon. Click to switch. Holding a file over it for a moment also switches
/// (like spring-loaded folders), so you can carry a file to Convert without letting go.
struct TabButton: View {
    let tab: NotchTab
    @ObservedObject var state: PanelState
    @State private var targeted = false
    @State private var pending: DispatchWorkItem?

    var body: some View {
        let selected = state.tab == tab
        Button { state.tab = tab; Settings.shared.lastTab = tab.rawValue } label: {
            Image(systemName: tab.rawValue).font(.system(size: 13))
                .foregroundStyle(selected || targeted ? Tok.Color.text : Tok.Color.textMuted)
                .frame(width: 38, height: Tok.Layout.tabBarHeight - 4)
                .background(targeted ? Tok.Color.accent.opacity(0.6) : selected ? Tok.Color.surface : .clear, in: Capsule())
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .accessibilityLabel(tab.label)
        .onDrop(of: [.fileURL], isTargeted: $targeted) { _ in false }   // never takes the file, only switches the tab
        .onChange(of: targeted) { _, on in
            pending?.cancel()
            guard on, !selected else { return }
            let work = DispatchWorkItem { MainActor.assumeIsolated { state.tab = tab } }
            pending = work
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.35, execute: work)
        }
    }
}

struct MusicTab: View {
    @ObservedObject var nowPlaying: NowPlaying
    @ObservedObject var lyrics: LyricsStore
    @AppStorage("musicShowLyrics") private var showLyrics = false

    var body: some View {
        VStack(spacing: Tok.Space.sm) {
            if let t = nowPlaying.track {
                if showLyrics { lyricsMode(t) } else { playerMode(t) }
                if t.duration > 0 {
                    ProgressBar(position: t.position, duration: t.duration, color: Settings.shared.haloColor) { nowPlaying.seek(to: $0) }
                }
            } else {
                Text(nowPlaying.permissionDenied
                     ? "Allow NotchHub to control your player in System Settings > Privacy > Automation."
                     : nowPlaying.chromeJSOff
                     ? "To see YouTube: in Chrome, menu View > Developer > Allow JavaScript from Apple Events."
                     : "Nothing playing. Start Music, Spotify or YouTube in Chrome.")
                    .font(Tok.Font.sm).foregroundStyle(Tok.Color.textMuted).multilineTextAlignment(.center)
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
            }
        }
        .padding(Tok.Space.md)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(Tok.Color.surface, in: RoundedRectangle(cornerRadius: Tok.Radius.lg))
        .clipShape(RoundedRectangle(cornerRadius: Tok.Radius.lg))
    }

    /// Cover, title, transport buttons and the lyrics switch.
    private func playerMode(_ t: Track) -> some View {
        HStack(spacing: Tok.Space.lg) {
            artwork(t, size: 64)
            VStack(alignment: .leading, spacing: Tok.Space.xs) {
                Text(t.title).font(Tok.Font.lg).foregroundStyle(Tok.Color.text).lineLimit(1)
                Text(t.artist).font(Tok.Font.sm).foregroundStyle(Tok.Color.textMuted).lineLimit(1)
                Text(t.app.shortName).font(Tok.Font.xs).foregroundStyle(Tok.Color.neon)
            }
            Spacer(minLength: 0)
            HStack(spacing: Tok.Space.md) {
                control("backward.fill", "Previous track") { nowPlaying.send(.previous) }
                control(t.isPlaying ? "pause.fill" : "play.fill", t.isPlaying ? "Pause" : "Play", big: true) { nowPlaying.send(.playPause) }
                control("forward.fill", "Next track") { nowPlaying.send(.next) }
                control("text.quote", "Show lyrics") { withAnimation(Tok.Motion.fast) { showLyrics = true } }
            }
        }
    }

    /// Slim header with the same controls, and the lyrics below it.
    private func lyricsMode(_ t: Track) -> some View {
        VStack(spacing: Tok.Space.xs) {
            HStack(spacing: Tok.Space.sm) {
                artwork(t, size: 30)
                VStack(alignment: .leading, spacing: 0) {
                    Text(t.title).font(Tok.Font.sm.weight(.semibold)).foregroundStyle(Tok.Color.text).lineLimit(1)
                    Text(t.artist).font(Tok.Font.xs).foregroundStyle(Tok.Color.textMuted).lineLimit(1)
                }
                Spacer(minLength: 0)
                small("backward.fill", "Previous track") { nowPlaying.send(.previous) }
                small(t.isPlaying ? "pause.fill" : "play.fill", t.isPlaying ? "Pause" : "Play") { nowPlaying.send(.playPause) }
                small("forward.fill", "Next track") { nowPlaying.send(.next) }
                small("music.note", "Back to player", highlighted: true) { withAnimation(Tok.Motion.fast) { showLyrics = false } }
            }
            LyricsBody(nowPlaying: nowPlaying, store: lyrics)
                .background(Tok.Color.bg.opacity(0.5), in: RoundedRectangle(cornerRadius: Tok.Radius.md))
                .clipShape(RoundedRectangle(cornerRadius: Tok.Radius.md))
        }
    }

    @ViewBuilder private func artwork(_ t: Track, size: CGFloat) -> some View {
        Group {
            if let url = t.artworkURL {
                AsyncImage(url: url) { $0.resizable().scaledToFill() } placeholder: { Tok.Color.surfaceHover }
            } else {
                ZStack { Tok.Color.surfaceHover; Image(systemName: "music.note").foregroundStyle(Tok.Color.textMuted) }
            }
        }
        .frame(width: size, height: size)
        .clipShape(RoundedRectangle(cornerRadius: size > 40 ? Tok.Radius.md : 6))
        .accessibilityHidden(true)
    }

    private func control(_ symbol: String, _ label: String, big: Bool = false, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Image(systemName: symbol).font(.system(size: big ? 22 : 16))
                .foregroundStyle(big ? Tok.Color.onAccent : Tok.Color.text)
                .frame(width: big ? 44 : 32, height: big ? 44 : 32)
                .background(big ? Tok.Color.accent : Tok.Color.surfaceHover, in: Circle())
        }.buttonStyle(.plain).accessibilityLabel(label)
    }

    private func small(_ symbol: String, _ label: String, highlighted: Bool = false, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Image(systemName: symbol).font(.system(size: 11))
                .foregroundStyle(highlighted ? Tok.Color.onAccent : Tok.Color.text)
                .frame(width: 24, height: 24)
                .background(highlighted ? Tok.Color.accent : Tok.Color.surfaceHover, in: Circle())
        }.buttonStyle(.plain).accessibilityLabel(label)
    }
}

/// Seekable progress bar with elapsed and total time. Dragging previews, releasing seeks.
struct ProgressBar: View {
    let position: Double, duration: Double
    let color: Color
    let onSeek: (Double) -> Void
    @State private var dragFraction: Double?

    private var fraction: Double { dragFraction ?? min(1, max(0, position / duration)) }
    private func fmt(_ s: Double) -> String { String(format: "%d:%02d", Int(s) / 60, Int(s) % 60) }

    var body: some View {
        HStack(spacing: Tok.Space.sm) {
            Text(fmt(fraction * duration)).font(Tok.Font.xs).monospacedDigit().foregroundStyle(Tok.Color.textMuted)
            GeometryReader { geo in
                ZStack(alignment: .leading) {
                    Capsule().fill(Tok.Color.surfaceHover)
                    Capsule().fill(color).frame(width: max(4, geo.size.width * fraction))
                }
                .frame(height: 5).frame(maxHeight: .infinity)
                .contentShape(Rectangle())
                .gesture(DragGesture(minimumDistance: 0)
                    .onChanged { dragFraction = min(1, max(0, $0.location.x / geo.size.width)) }
                    .onEnded { v in
                        let f = min(1, max(0, v.location.x / geo.size.width))
                        dragFraction = nil
                        onSeek(f * duration)
                    })
            }.frame(height: 16)
            Text(fmt(duration)).font(Tok.Font.xs).monospacedDigit().foregroundStyle(Tok.Color.textMuted)
        }
        .accessibilityElement(children: .ignore)
        .accessibilityLabel("Progress")
        .accessibilityValue("\(fmt(position)) of \(fmt(duration))")
    }
}


/// The line being sung right now, shown under the closed notch.
struct LiveLyricLine: View {
    @ObservedObject var lyrics: LyricsStore

    var body: some View {
        let text = lyrics.liveLine
        Text(text.isEmpty ? "♪" : text)
            .font(.system(size: 12, weight: .medium)).foregroundStyle(Tok.Color.text)
            .lineLimit(1).minimumScaleFactor(0.8).truncationMode(.tail)
            .padding(.horizontal, Tok.Space.md)
            .frame(height: Tok.Layout.liveLyricsHeight)
            .id(text)
            .transition(.opacity)
            .animation(.easeInOut(duration: 0.25), value: text)
            .accessibilityHidden(true)
    }
}
