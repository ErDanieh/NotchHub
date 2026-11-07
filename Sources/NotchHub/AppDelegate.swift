import AppKit
import SwiftUI
import ServiceManagement

@MainActor
final class AppDelegate: NSObject, NSApplicationDelegate {
    private let controller = NotchPanelController()
    private var statusItem: NSStatusItem!
    private var loginItem: NSMenuItem!

    func applicationDidFinishLaunching(_ notification: Notification) {
        controller.start()
        buildMenu()
    }

    private func buildMenu() {
        statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.squareLength)
        statusItem.button?.image = NSImage(systemSymbolName: "tray.and.arrow.down", accessibilityDescription: "NotchHub")
        let menu = NSMenu()
        menu.addItem(withTitle: "Open shelf", action: #selector(openShelf), keyEquivalent: "").target = self
        menu.addItem(withTitle: "Clear shelf", action: #selector(clearShelf), keyEquivalent: "").target = self
        menu.addItem(.separator())
        menu.addItem(withTitle: "Reorder tabs…", action: #selector(openTabOrder), keyEquivalent: "").target = self
        menu.addItem(withTitle: "News sources…", action: #selector(openNewsManager), keyEquivalent: "").target = self
        menu.addItem(withTitle: "Settings…", action: #selector(openSettings), keyEquivalent: ",").target = self
        loginItem = menu.addItem(withTitle: "Launch at login", action: #selector(toggleLogin), keyEquivalent: "")
        loginItem.target = self
        menu.addItem(.separator())
        menu.addItem(withTitle: "Quit NotchHub", action: #selector(quit), keyEquivalent: "q").target = self
        menu.delegate = self
        statusItem.menu = menu
    }

    @objc private func openShelf() { controller.toggle() }
    @objc private func clearShelf() { controller.store.clear() }
    private var settingsWindow: NSWindow?
    @objc private func openSettings() {
        if settingsWindow == nil {
            let w = NSWindow(contentViewController: NSHostingController(rootView: SettingsView()))
            w.title = "NotchHub Settings"
            w.styleMask = [.titled, .closable]
            w.isReleasedWhenClosed = false
            settingsWindow = w
        }
        NSApp.activate(ignoringOtherApps: true)
        settingsWindow?.center()
        settingsWindow?.makeKeyAndOrderFront(nil)
    }
    @objc private func openNewsManager() { NewsManagerWindow.show() }
    private var tabOrderWindow: NSWindow?
    @objc private func openTabOrder() {
        if tabOrderWindow == nil {
            let w = NSWindow(contentViewController: NSHostingController(rootView: TabOrderView()))
            w.title = "Reorder tabs"
            w.styleMask = [.titled, .closable]
            w.isReleasedWhenClosed = false
            tabOrderWindow = w
        }
        NSApp.activate(ignoringOtherApps: true)
        tabOrderWindow?.center()
        tabOrderWindow?.makeKeyAndOrderFront(nil)
    }
    @objc private func quit() { NSApp.terminate(nil) }
    @objc private func toggleLogin() {
        do {
            if SMAppService.mainApp.status == .enabled { try SMAppService.mainApp.unregister() }
            else { try SMAppService.mainApp.register() }
        } catch {
            let a = NSAlert(); a.messageText = "Could not change launch at login"
            a.informativeText = "This only works from the packaged NotchHub.app (Scripts/build-app.sh). \(error.localizedDescription)"
            a.runModal()
        }
    }
}

extension AppDelegate: NSMenuDelegate {
    func menuWillOpen(_ menu: NSMenu) {
        loginItem.state = SMAppService.mainApp.status == .enabled ? .on : .off
    }
}
