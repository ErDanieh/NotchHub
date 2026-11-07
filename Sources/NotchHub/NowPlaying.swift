import Foundation
import AppKit

/// What is playing, read with AppleScript (public, official route).
/// Music and Spotify: their scripting dictionaries. YouTube in Chrome: runs a tiny read-only
/// script in the tab, which Chrome only allows after the user turns on
/// View > Developer > Allow JavaScript from Apple Events.
/// Only asks apps that are already running, so it never launches them.
struct Track: Equatable {
    var app: PlayerApp
    var title: String
    var artist: String
    var isPlaying: Bool
    var artworkURL: URL?
    var position: Double = 0     // seconds
    var duration: Double = 0     // seconds, 0 = unknown / live
    var measuredAt = Date()      // when the position was really read (middle of the script run)
    var tab: String? = nil      // Chrome: "window,tab" of the YouTube tab
}

enum PlayerApp: String, CaseIterable {
    case music = "Music", spotify = "Spotify", chrome = "Google Chrome"
    var bundleID: String {
        switch self { case .music: "com.apple.Music"; case .spotify: "com.spotify.client"; case .chrome: "com.google.Chrome" }
    }
    var isRunning: Bool { !NSRunningApplication.runningApplications(withBundleIdentifier: bundleID).isEmpty }
    var shortName: String { self == .chrome ? "YouTube (Chrome)" : rawValue }
}

enum PlayerCommand: String { case playPause = "playpause", next = "next track", previous = "previous track" }

@MainActor
final class NowPlaying: ObservableObject {
    @Published private(set) var track: Track?
    @Published private(set) var permissionDenied = false
    @Published private(set) var chromeJSOff = false
    var isPlaying: Bool { track?.isPlaying == true }
    /// Position now, in seconds: the last reading plus the time since, so lyrics move smoothly between 1s readings.
    var estimatedPosition: Double {
        guard let t = track else { return 0 }
        return t.position + (t.isPlaying ? Date().timeIntervalSince(t.measuredAt) : 0)
    }

    private let queue = DispatchQueue(label: "notchhub.applescript")
    private var timer: Timer?
    /// True while the panel is open (progress bar needs 1s updates); closed, it checks every 3s to save CPU.
    var fast = false
    private var ticks = 0

    func start() {
        refresh()
        // Music and Spotify announce every change (song, play, pause) the moment it happens: react at once.
        for name in ["com.apple.Music.playerInfo", "com.spotify.client.PlaybackStateChanged"] {
            DistributedNotificationCenter.default().addObserver(forName: NSNotification.Name(name), object: nil, queue: .main) { [weak self] _ in
                MainActor.assumeIsolated { DispatchQueue.main.asyncAfter(deadline: .now() + 0.15) { MainActor.assumeIsolated { self?.refresh() } } }
            }
        }
        // Switching to Chrome or opening a player is a good moment to look again too.
        NSWorkspace.shared.notificationCenter.addObserver(forName: NSWorkspace.didActivateApplicationNotification, object: nil, queue: .main) { [weak self] _ in
            MainActor.assumeIsolated { self?.refresh() }
        }
        // Chrome has no change events, so it is polled: every second while something plays (the song can change
        // at any moment), every 2 seconds while nothing plays (to notice that music has started).
        timer = Timer.scheduledTimer(withTimeInterval: 1.0, repeats: true) { [weak self] _ in
            MainActor.assumeIsolated {
                guard let self else { return }
                self.ticks += 1
                if self.fast || self.isPlaying || self.ticks % 2 == 0 { self.refresh() }
            }
        }
    }

    func send(_ command: PlayerCommand) {
        guard let t = track else { return }
        let script: String
        if t.app == .chrome, let tab = t.tab {
            let js: String
            switch command {
            case .playPause: js = "var v=document.querySelector('video');if(v){v.paused?v.play():v.pause();}"
            case .next: js = "var b=document.querySelector('.next-button,.ytp-next-button');if(b){b.click();}"
            case .previous: js = "var b=document.querySelector('.previous-button,.ytp-prev-button');if(b&&b.offsetParent!==null){b.click();}else{var v=document.querySelector('video');if(v){v.currentTime=0;}}"
            }
            let parts = tab.split(separator: ",")
            script = "tell application \"Google Chrome\" to execute tab \(parts[1]) of window \(parts[0]) javascript \"\(js)\""
        } else {
            script = "tell application \"\(t.app.rawValue)\" to \(command.rawValue)"
        }
        queue.async { _ = Self.run(script) }
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.35) { MainActor.assumeIsolated { self.refresh() } }
    }

    /// Jump to a point in the current track or video (seconds).
    func seek(to seconds: Double) {
        guard let t = track else { return }
        let sec = max(0, seconds)
        let script: String
        if t.app == .chrome, let tab = t.tab {
            let parts = tab.split(separator: ",")
            script = "tell application \"Google Chrome\" to execute tab \(parts[1]) of window \(parts[0]) javascript \"var v=document.querySelector('video');if(v){function ts(s){var p=s.trim().split(':');var n=0;for(var i=0;i<p.length;i++){n=n*60+Number(p[i]);}return n;}var ti=document.querySelector('ytmusic-player-bar .time-info');var barPos=-1,barDur=0;if(ti){var q=ti.textContent.split('/');if(q.length==2){barPos=ts(q[0]);barDur=ts(q[1]);}}var vd=isFinite(v.duration)?v.duration:0;var dur=barDur>0?barDur:vd;var off=(barDur>0&&barPos>=0)?Math.ceil(barPos-v.currentTime):0;if(isNaN(off)||isNaN(dur)){off=0;dur=vd;}v.currentTime=\(Int(sec))-off;}\""
        } else {
            script = "tell application \"\(t.app.rawValue)\" to set player position to \(Int(sec))"
        }
        track?.position = sec
        track?.measuredAt = Date()
        queue.async { _ = Self.run(script) }
    }

    func refresh() {
        let running = PlayerApp.allCases.filter { $0.isRunning }
        queue.async {
            var found: [Track] = []
            var denied = false, jsOff = false
            for app in running {
                if app == .chrome {
                    let t0 = Date()
                    let r = Self.run(Self.chromeScript)
                    let mid = t0.addingTimeInterval(Date().timeIntervalSince(t0) / 2)
                    if r.error == -1743 { denied = true; continue }
                    if let m = r.message, m.localizedCaseInsensitiveContains("javascript") { jsOff = true; continue }
                    let p = (r.value ?? "").components(separatedBy: "§")
                    if p.count >= 7 {
                        found.append(Track(app: .chrome, title: p[1], artist: p[2], isPlaying: p[0] == "playing",
                                           artworkURL: URL(string: p[5]), position: (Double(p[3]) ?? 0) / 1000,
                                           duration: Double(p[4]) ?? 0, measuredAt: mid, tab: p[6]))
                    }
                    continue
                }
                // format: state§name§artist§position§duration§artwork   (whole seconds, so no locale decimals)
                let dur = app == .spotify ? "(round ((duration of current track) / 1000))" : "(round (duration of current track))"
                let art = app == .spotify ? "(artwork url of current track)" : "\"\""
                let script = """
                tell application "\(app.rawValue)"
                    try
                        return (player state as string) & "§" & (name of current track) & "§" & (artist of current track) & "§" & (round ((player position) * 1000)) & "§" & \(dur) & "§" & \(art)
                    on error
                        return "stopped"
                    end try
                end tell
                """
                let t0 = Date()
                let r = Self.run(script)
                let mid = t0.addingTimeInterval(Date().timeIntervalSince(t0) / 2)
                if r.error == -1743 { denied = true; continue }
                let p = (r.value ?? "").components(separatedBy: "§")
                guard p.count >= 6, p[0] != "stopped" else { continue }
                found.append(Track(app: app, title: p[1], artist: p[2], isPlaying: p[0] == "playing",
                                   artworkURL: URL(string: p[5]), position: (Double(p[3]) ?? 0) / 1000, duration: Double(p[4]) ?? 0, measuredAt: mid))
            }
            let best = found.first { $0.isPlaying } ?? found.first
            DispatchQueue.main.async {
                MainActor.assumeIsolated {
                    if self.track != best { self.track = best }
                    if self.permissionDenied != denied { self.permissionDenied = denied }
                    if self.chromeJSOff != jsOff { self.chromeJSOff = jsOff }
                }
            }
        }
    }

    /// Looks through Chrome's tabs for YouTube / YouTube Music. Returns the first playing one,
    /// else the first with a video. Format: state§title§artist§position§duration§artwork§window,tab
    nonisolated private static let chromeScript = """
    set js to "(function(){var v=document.querySelector('video');if(!v)return '';var m=navigator.mediaSession&&navigator.mediaSession.metadata;var title=(m&&m.title)?m.title:document.title.replace(/ - YouTube( Music)?$/,'');var artist=(m&&m.artist)?m.artist:'';if(!artist){var c=document.querySelector('ytd-channel-name a,#owner #channel-name a,.byline');artist=c?c.textContent.trim():'';}var art=(m&&m.artwork&&m.artwork.length)?m.artwork[m.artwork.length-1].src:'';function ts(s){var p=s.trim().split(':');var n=0;for(var i=0;i<p.length;i++){n=n*60+Number(p[i]);}return n;}var ti=document.querySelector('ytmusic-player-bar .time-info');var barPos=-1,barDur=0;if(ti){var q=ti.textContent.split('/');if(q.length==2){barPos=ts(q[0]);barDur=ts(q[1]);}}var vd=isFinite(v.duration)?v.duration:0;var dur=barDur>0?barDur:vd;var off=(barDur>0&&barPos>=0)?Math.ceil(barPos-v.currentTime):0;if(isNaN(off)||isNaN(dur)){off=0;dur=vd;}return (v.paused?'paused':'playing')+'§'+title+'§'+artist+'§'+Math.round((v.currentTime+off)*1000)+'§'+Math.round(dur)+'§'+art;})()"
    set fallback to ""
    tell application "Google Chrome"
        repeat with wi from 1 to (count of windows)
            repeat with ti from 1 to (count of tabs of window wi)
                set u to URL of tab ti of window wi
                if u contains "youtube.com" then
                    set r to execute tab ti of window wi javascript js
                    if r is not "" and r is not missing value then
                        set r to r & "§" & wi & "," & ti
                        if r starts with "playing" then return r
                        if fallback is "" then set fallback to r
                    end if
                end if
            end repeat
        end repeat
    end tell
    return fallback
    """

    /// Compiled scripts are kept: compiling the long Chrome script every second was the main CPU cost.
    nonisolated(unsafe) private static var compiled: [String: NSAppleScript] = [:]
    nonisolated private static let compileLock = NSLock()

    nonisolated private static func run(_ source: String) -> (value: String?, error: Int?, message: String?) {
        var err: NSDictionary?
        compileLock.lock()
        var script = compiled[source]
        if script == nil, compiled.count < 40 { script = NSAppleScript(source: source); compiled[source] = script }
        compileLock.unlock()
        let out = (script ?? NSAppleScript(source: source))?.executeAndReturnError(&err)
        if let err { return (nil, err[NSAppleScript.errorNumber] as? Int, err[NSAppleScript.errorMessage] as? String) }
        return (out?.stringValue, nil, nil)
    }
}
