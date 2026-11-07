import Foundation
import CoreAudio
import AudioToolbox

/// Listens to what the Mac is playing (system output) and turns the bass and volume into a 0...1 level
/// that the halo follows. Uses a Core Audio "process tap" (public API, macOS 14.2+). It only runs while
/// music is playing and the option is on, and macOS asks for "System Audio Recording" permission the first time.
final class AudioLevel: @unchecked Sendable {
    static let shared = AudioLevel()

    /// 0...1, written by the audio thread, read by the halo.
    private(set) var level: Float = 0
    /// Shown in Settings so a failure is visible.
    private(set) var status = "Off"

    private var tapID = AudioObjectID(kAudioObjectUnknown)
    private var aggregateID = AudioObjectID(kAudioObjectUnknown)
    private var procID: AudioDeviceIOProcID?
    private let queue = DispatchQueue(label: "notchhub.audio", qos: .userInteractive)
    private var running = false
    private var lastFailure: Date?

    // DSP state (audio thread only)
    private var lowpass: Float = 0, env: Float = 0, peak: Float = 0.02

    func start() {
        guard !running else { return }
        if let f = lastFailure, Date().timeIntervalSince(f) < 30 { return }     // do not hammer a failing tap
        guard #available(macOS 14.2, *) else { status = "Needs macOS 14.2 or newer"; return }
        // 1. tap the whole system output (stereo mix of every app)
        let desc = CATapDescription(stereoGlobalTapButExcludeProcesses: [])
        desc.name = "NotchHub beat"
        desc.isPrivate = true
        desc.muteBehavior = .unmuted
        var tap = AudioObjectID(kAudioObjectUnknown)
        var err = AudioHardwareCreateProcessTap(desc, &tap)
        guard err == noErr else { lastFailure = Date(); status = "Could not listen (error \(err)). Allow System Audio Recording in System Settings > Privacy & Security."; return }
        tapID = tap

        // 2. a private aggregate device that contains the tap, so we can read it like an input
        guard let outUID = Self.defaultOutputUID() else { lastFailure = Date(); status = "No output device"; cleanup(); return }
        let dict: [String: Any] = [
            kAudioAggregateDeviceNameKey: "NotchHub beat tap",
            kAudioAggregateDeviceUIDKey: UUID().uuidString,
            kAudioAggregateDeviceMainSubDeviceKey: outUID,
            kAudioAggregateDeviceIsPrivateKey: true,
            kAudioAggregateDeviceIsStackedKey: false,
            kAudioAggregateDeviceTapAutoStartKey: true,
            kAudioAggregateDeviceSubDeviceListKey: [[kAudioSubDeviceUIDKey: outUID]],
            kAudioAggregateDeviceTapListKey: [[kAudioSubTapDriftCompensationKey: true, kAudioSubTapUIDKey: desc.uuid.uuidString]],
        ]
        var agg = AudioObjectID(kAudioObjectUnknown)
        err = AudioHardwareCreateAggregateDevice(dict as CFDictionary, &agg)
        guard err == noErr else { lastFailure = Date(); status = "Could not create audio device (error \(err))"; cleanup(); return }
        aggregateID = agg

        // 3. read the audio
        var sampleRate: Double = 44100
        var size = UInt32(MemoryLayout<Double>.size)
        var addr = AudioObjectPropertyAddress(mSelector: kAudioDevicePropertyNominalSampleRate, mScope: kAudioObjectPropertyScopeGlobal, mElement: kAudioObjectPropertyElementMain)
        AudioObjectGetPropertyData(agg, &addr, 0, nil, &size, &sampleRate)
        let alpha = Float(1 - exp(-2 * Double.pi * 160 / max(8000, sampleRate)))      // ~160 Hz low-pass: the bass

        err = AudioDeviceCreateIOProcIDWithBlock(&procID, agg, queue) { [unowned self] _, inData, _, _, _ in
            self.process(inData, alpha: alpha, sampleRate: Float(sampleRate))
        }
        guard err == noErr, let procID else { lastFailure = Date(); status = "Could not read audio (error \(err))"; cleanup(); return }
        err = AudioDeviceStart(agg, procID)
        guard err == noErr else { lastFailure = Date(); status = "Could not start listening (error \(err))"; cleanup(); return }
        running = true
        lastFailure = nil
        status = "Listening"
    }

    func stop() {
        lastFailure = nil
        guard running || tapID != kAudioObjectUnknown else { status = "Off"; level = 0; return }
        cleanup()
        running = false
        status = "Off"
        level = 0
    }

    private func cleanup() {
        if aggregateID != kAudioObjectUnknown {
            if let procID { AudioDeviceStop(aggregateID, procID); AudioDeviceDestroyIOProcID(aggregateID, procID) }
            AudioHardwareDestroyAggregateDevice(aggregateID)
        }
        if #available(macOS 14.2, *), tapID != kAudioObjectUnknown { AudioHardwareDestroyProcessTap(tapID) }
        procID = nil; aggregateID = AudioObjectID(kAudioObjectUnknown); tapID = AudioObjectID(kAudioObjectUnknown)
    }

    // MARK: DSP (audio thread)
    private func process(_ list: UnsafePointer<AudioBufferList>, alpha: Float, sampleRate: Float) {
        let buffers = UnsafeMutableAudioBufferListPointer(UnsafeMutablePointer(mutating: list))
        guard let first = buffers.first, first.mDataByteSize > 0 else { return }
        let interleaved = buffers.count == 1
        let channels = max(1, interleaved ? Int(first.mNumberChannels) : buffers.count)
        let frames = Int(first.mDataByteSize) / MemoryLayout<Float>.size / (interleaved ? channels : 1)
        guard frames > 0 else { return }
        var bassSum: Float = 0, allSum: Float = 0
        for i in 0..<frames {
            var mono: Float = 0
            for c in 0..<channels {
                let b = interleaved ? buffers[0] : buffers[c]
                guard let data = b.mData?.assumingMemoryBound(to: Float.self) else { continue }
                mono += interleaved ? data[i * channels + c] : data[i]
            }
            mono /= Float(channels)
            lowpass += alpha * (mono - lowpass)
            bassSum += lowpass * lowpass
            allSum += mono * mono
        }
        let bass = (bassSum / Float(frames)).squareRoot(), all = (allSum / Float(frames)).squareRoot()
        let energy = 0.75 * bass * 2.2 + 0.25 * all          // bass counts most, scaled up because it is quieter than the full signal
        // envelope: instant attack, fast release, so each beat is a short flash
        let blockSeconds = Float(frames) / sampleRate
        env = max(energy, env * pow(0.5, blockSeconds / 0.12))
        // follow the song's own loudness so quiet and loud songs both use the whole range
        peak = max(energy, peak * pow(0.5, blockSeconds / 6))
        let floor: Float = 0.004
        let norm = peak > floor ? min(1, env / peak) : 0
        level = norm * norm       // squared: only real peaks are bright, the rest stays calm
    }

    // MARK: helpers
    private static func defaultOutputUID() -> String? {
        var dev = AudioObjectID(kAudioObjectUnknown)
        var size = UInt32(MemoryLayout<AudioObjectID>.size)
        var addr = AudioObjectPropertyAddress(mSelector: kAudioHardwarePropertyDefaultOutputDevice, mScope: kAudioObjectPropertyScopeGlobal, mElement: kAudioObjectPropertyElementMain)
        guard AudioObjectGetPropertyData(AudioObjectID(kAudioObjectSystemObject), &addr, 0, nil, &size, &dev) == noErr else { return nil }
        var uid: Unmanaged<CFString>?
        var usize = UInt32(MemoryLayout<Unmanaged<CFString>?>.size)
        var uaddr = AudioObjectPropertyAddress(mSelector: kAudioDevicePropertyDeviceUID, mScope: kAudioObjectPropertyScopeGlobal, mElement: kAudioObjectPropertyElementMain)
        guard AudioObjectGetPropertyData(dev, &uaddr, 0, nil, &usize, &uid) == noErr else { return nil }
        return uid?.takeRetainedValue() as String?
    }
}
