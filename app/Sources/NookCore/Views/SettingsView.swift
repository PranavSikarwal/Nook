import SwiftUI

public struct SettingsView: View {
    @ObservedObject var state: PanelState

    public init(state: PanelState) {
        self.state = state
    }

    public var body: some View {
        VStack(spacing: 0) {
            HStack {
                Text("Settings")
                    .font(.system(size: 14, weight: .semibold))
                Spacer()
                Button(action: {
                    state.toggleSettings()
                }) {
                    Image(systemName: "xmark.circle.fill")
                        .font(.system(size: 14))
                        .foregroundColor(.secondary)
                }
                .buttonStyle(.plain)
            }
            .padding(.horizontal, 14)
            .padding(.vertical, 10)
            .background(Color(NSColor.controlBackgroundColor))

            Divider()

            Form {
                Section {
                    TextField("Base URL", text: $state.baseUrl)
                        .textFieldStyle(.roundedBorder)

                    TextField("Model Name", text: $state.model)
                        .textFieldStyle(.roundedBorder)

                    SecureField(state.hasApiKey ? "API Key (stored)" : "API Key", text: $state.apiKeyInput)
                        .textFieldStyle(.roundedBorder)

                    HStack {
                        Text("Global Hotkey")
                        Spacer()
                        Text(state.hotkeyDisplay)
                            .font(.system(size: 12, design: .monospaced))
                            .foregroundColor(.secondary)
                            .padding(.horizontal, 6)
                            .padding(.vertical, 2)
                            .background(Color.secondary.opacity(0.1))
                            .cornerRadius(4)
                    }
                }
            }
            .padding(14)

            Divider()

            HStack {
                Spacer()
                Button("Save") {
                    state.saveSettings()
                }
                .buttonStyle(.borderedProminent)
            }
            .padding(.horizontal, 14)
            .padding(.vertical, 10)
        }
    }
}
