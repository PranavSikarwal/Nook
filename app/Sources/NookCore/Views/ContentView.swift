import SwiftUI

public struct ContentView: View {
    @ObservedObject var state: PanelState

    public init(state: PanelState) {
        self.state = state
    }

    public var body: some View {
        VStack(spacing: 0) {
            // Disconnected status bar if daemon is offline
            if !state.isConnected {
                HStack(spacing: 6) {
                    Circle()
                        .fill(Color.orange)
                        .frame(width: 8, height: 8)
                    Text("Nook daemon not running")
                        .font(.system(size: 11, weight: .medium))
                        .foregroundColor(.secondary)
                    Spacer()
                }
                .padding(.horizontal, 14)
                .padding(.top, 6)
                .padding(.bottom, 2)
            }

            // Body content area depending on mode
            switch state.viewMode {
            case .compact:
                EmptyView()

            case .expanded:
                TranscriptView(state: state)
                    .frame(height: 380)
                Divider()

            case .history:
                HistoryView(state: state)
                    .frame(height: 380)
                Divider()

            case .settings:
                SettingsView(state: state)
                    .frame(height: 280)
                Divider()
            }

            // Bottom input bar
            CompactInputView(state: state)
        }
        .frame(width: 640)
        .background(
            RoundedRectangle(cornerRadius: 14)
                .fill(Color(NSColor.windowBackgroundColor))
        )
        .clipShape(RoundedRectangle(cornerRadius: 14))
        .overlay(
            RoundedRectangle(cornerRadius: 14)
                .stroke(Color.secondary.opacity(0.2), lineWidth: 1)
        )
        .animation(.easeInOut(duration: 0.2), value: state.viewMode)
    }
}
