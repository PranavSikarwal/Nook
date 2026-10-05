import AppKit
import Combine
import SwiftUI

@MainActor
public final class PanelController {
    public static let shared = PanelController()

    public let panel: NookPanel
    private var globalClickMonitor: Any?
    private var localKeyMonitor: Any?
    private var stateCancellable: AnyCancellable?

    private init() {
        let initialWidth: CGFloat = 640
        let initialHeight: CGFloat = 80
        let rect = NSRect(x: 0, y: 0, width: initialWidth, height: initialHeight)
        self.panel = NookPanel(contentRect: rect)

        let rootView = ContentView(state: PanelState.shared)
        let hostingView = NSHostingView(rootView: rootView)
        self.panel.contentView = hostingView
        centerPanel()

        stateCancellable = PanelState.shared.$viewMode
            .receive(on: DispatchQueue.main)
            .sink { [weak self] mode in
                self?.updateFrame(for: mode)
            }
    }

    public func setup() {
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
        // Opening always starts a fresh Chat in compact mode per spec
        PanelState.shared.startNewChat()

        updateFrame(for: .compact, animate: false)
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

        // Reset to clean compact state for next open if messages were sent
        if !PanelState.shared.messages.isEmpty {
            PanelState.shared.startNewChat()
            updateFrame(for: .compact, animate: false)
        }
    }

    public func centerPanel() {
        updateFrame(for: PanelState.shared.viewMode, animate: false)
    }

    private func targetHeight(for mode: ViewMode) -> CGFloat {
        switch mode {
        case .compact:
            return 80
        case .expanded:
            return 480
        case .history:
            return 480
        case .settings:
            return 380
        }
    }

    public func updateFrame(for mode: ViewMode, animate: Bool = true) {
        guard let screen = NSScreen.main else { return }
        let screenRect = screen.visibleFrame
        let panelWidth: CGFloat = 640
        let targetH = targetHeight(for: mode)

        let x = screenRect.origin.x + (screenRect.width - panelWidth) / 2
        let y = screenRect.origin.y + screenRect.height * 0.7 - targetH / 2

        let newFrame = NSRect(x: x, y: y, width: panelWidth, height: targetH)
        panel.setFrame(newFrame, display: true, animate: animate && panel.isVisible)
    }
}
