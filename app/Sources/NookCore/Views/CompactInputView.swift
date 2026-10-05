import SwiftUI

public struct CompactInputView: View {
    @ObservedObject var state: PanelState
    @State private var text: String = ""

    public init(state: PanelState) {
        self.state = state
    }

    public var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            AttachmentsView(state: state)

            HStack(spacing: 8) {
                // Paperclip button
                Button(action: {
                    AttachmentValidator.openFilePicker { attachments in
                        state.addAttachments(attachments)
                    }
                }) {
                    Image(systemName: "paperclip")
                        .foregroundColor(.secondary)
                        .font(.system(size: 14))
                }
                .buttonStyle(.plain)
                .disabled(!state.isConnected)

                // Input field
                TextField(state.isConnected ? "Ask anything..." : "Nook daemon not running", text: $text)
                    .textFieldStyle(.plain)
                    .font(.system(size: 14))
                    .disabled(!state.isConnected)
                    .onSubmit {
                        submit()
                    }

                // History toggle
                Button(action: {
                    state.toggleHistory()
                }) {
                    Image(systemName: "clock.arrow.circlepath")
                        .foregroundColor(state.viewMode == .history ? .accentColor : .secondary)
                        .font(.system(size: 14))
                }
                .buttonStyle(.plain)
                .disabled(!state.isConnected)

                // Settings gear
                Button(action: {
                    state.toggleSettings()
                }) {
                    Image(systemName: "gearshape")
                        .foregroundColor(state.viewMode == .settings ? .accentColor : .secondary)
                        .font(.system(size: 14))
                }
                .buttonStyle(.plain)

                // Send / Stop button
                if state.isStreaming {
                    Button(action: {
                        state.stopStreaming()
                    }) {
                        Image(systemName: "stop.circle.fill")
                            .foregroundColor(.red)
                            .font(.system(size: 18))
                    }
                    .buttonStyle(.plain)
                } else {
                    Button(action: {
                        submit()
                    }) {
                        Image(systemName: "arrow.up.circle.fill")
                            .foregroundColor(canSubmit ? .accentColor : .secondary.opacity(0.4))
                            .font(.system(size: 18))
                    }
                    .buttonStyle(.plain)
                    .disabled(!canSubmit)
                }
            }
        }
        .padding(.horizontal, 14)
        .padding(.vertical, 10)
        .background(
            RoundedRectangle(cornerRadius: 12)
                .fill(Color(NSColor.windowBackgroundColor))
                .shadow(color: .black.opacity(0.15), radius: 10, x: 0, y: 4)
        )
        .overlay(
            RoundedRectangle(cornerRadius: 12)
                .stroke(Color.secondary.opacity(0.2), lineWidth: 1)
        )
        .onReceive(NotificationCenter.default.publisher(for: NSApplication.didBecomeActiveNotification)) { _ in
            checkClipboard()
        }
    }

    private var canSubmit: Bool {
        state.isConnected && (!text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || state.pendingAttachments.contains(where: { $0.isValid }))
    }

    private func submit() {
        let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
        if canSubmit {
            state.sendMessage(text: trimmed)
            text = ""
        }
    }

    private func checkClipboard() {
        // Automatically check if user pasted files or images
    }
}
