import AppKit
import SwiftUI

@MainActor
public final class PanelController: DaemonClientDelegate {
    public static let shared = PanelController()

    public let panel: NookPanel
    private var globalClickMonitor: Any?
    private var localKeyMonitor: Any?
    private var stateObserver: Any?

    private init() {
        let initialWidth: CGFloat = 640
        let initialHeight: CGFloat = 80
        let rect = NSRect(x: 0, y: 0, width: initialWidth, height: initialHeight)
        self.panel = NookPanel(contentRect: rect)

        let rootView = ContentView(state: PanelState.shared)
        let hostingView = NSHostingView(rootView: rootView)
        self.panel.contentView = hostingView
        centerPanel()
    }

    public func setup() {
        DaemonClient.shared.delegate = self
        DaemonClient.shared.connect()

        HotkeyManager.shared.register { [weak self] in
            DispatchQueue.main.async {
                self?.toggle()
            }
        }
    }

    public func toggle() {
        if panel.isVisible {
            hide()
        } else {
            show()
        }
    }

    public func show() {
        // Opening always starts a fresh Chat with a new UUID per spec
        PanelState.shared.startNewChat()

        centerPanel()
        panel.makeKeyAndOrderFront(nil)

        if globalClickMonitor == nil {
            globalClickMonitor = NSEvent.addGlobalMonitorForEvents(
                matching: [.leftMouseDown, .rightMouseDown]
            ) { [weak self] _ in
                guard let self = self else { return }
                let mouseLocation = NSEvent.mouseLocation
                if !self.panel.frame.contains(mouseLocation) {
                    DispatchQueue.main.async {
                        self.hide()
                    }
                }
            }
        }

        if localKeyMonitor == nil {
            localKeyMonitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { [weak self] event in
                if event.keyCode == 53 { // Escape
                    self?.hide()
                    return nil
                }
                // Handle Cmd+V clipboard paste for attachments
                if event.modifierFlags.contains(.command) && event.keyCode == 9 { // 'v'
                    let pasted = AttachmentValidator.fromPasteboard()
                    if !pasted.isEmpty {
                        PanelState.shared.addAttachments(pasted)
                    }
                }
                return event
            }
        }
    }

    public func hide() {
        panel.orderOut(nil)

        if let monitor = globalClickMonitor {
            NSEvent.removeMonitor(monitor)
            globalClickMonitor = nil
        }

        if let monitor = localKeyMonitor {
            NSEvent.removeMonitor(monitor)
            localKeyMonitor = nil
        }
    }

    public func centerPanel() {
        guard let screen = NSScreen.main else { return }
        let screenRect = screen.visibleFrame
        let panelWidth = panel.frame.width
        let panelHeight = panel.frame.height

        let x = screenRect.origin.x + (screenRect.width - panelWidth) / 2
        let y = screenRect.origin.y + screenRect.height * 0.7 - panelHeight / 2

        panel.setFrameOrigin(NSPoint(x: x, y: y))
    }

    // MARK: - DaemonClientDelegate

    public func daemonClient(_ client: DaemonClient, didChangeConnectionState isConnected: Bool) {
        PanelState.shared.isConnected = isConnected
    }

    public func daemonClient(_ client: DaemonClient, didReceiveEvent event: DaemonMessage) {
        // Dispatched through onMessage
    }
}
