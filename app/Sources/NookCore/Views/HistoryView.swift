import SwiftUI

public struct HistoryView: View {
    @ObservedObject var state: PanelState
    @State private var chatToDelete: UUID?
    @State private var showDeleteConfirmation = false

    public init(state: PanelState) {
        self.state = state
    }

    public var body: some View {
        VStack(spacing: 0) {
            HStack {
                Text("Chat History")
                    .font(.system(size: 14, weight: .semibold))
                Spacer()
                Button(action: {
                    state.startNewChat()
                }) {
                    HStack(spacing: 4) {
                        Image(systemName: "plus.circle.fill")
                        Text("New chat")
                    }
                    .font(.system(size: 12, weight: .medium))
                    .foregroundColor(.accentColor)
                }
                .buttonStyle(.plain)
            }
            .padding(.horizontal, 14)
            .padding(.vertical, 10)
            .background(Color(NSColor.controlBackgroundColor))

            Divider()

            if state.historyChats.isEmpty {
                VStack(spacing: 8) {
                    Spacer()
                    Image(systemName: "bubble.left.and.bubble.right")
                        .font(.system(size: 24))
                        .foregroundColor(.secondary)
                    Text("No past conversations")
                        .font(.system(size: 13))
                        .foregroundColor(.secondary)
                    Spacer()
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            } else {
                List {
                    ForEach(state.historyChats, id: \.chatId) { chat in
                        HStack {
                            VStack(alignment: .leading, spacing: 2) {
                                Text(chat.title.isEmpty ? "New chat" : chat.title)
                                    .font(.system(size: 13, weight: .medium))
                                    .lineLimit(1)
                                Text(formattedDate(chat.updatedAt))
                                    .font(.system(size: 11))
                                    .foregroundColor(.secondary)
                            }

                            Spacer()

                            Button(action: {
                                chatToDelete = chat.chatId
                                showDeleteConfirmation = true
                            }) {
                                Image(systemName: "trash")
                                    .font(.system(size: 12))
                                    .foregroundColor(.secondary)
                            }
                            .buttonStyle(.plain)
                        }
                        .contentShape(Rectangle())
                        .onTapGesture {
                            state.openPastChat(selectedChatId: chat.chatId)
                        }
                        .padding(.vertical, 2)
                    }
                }
                .listStyle(.plain)
            }
        }
        .confirmationDialog(
            "Delete Conversation",
            isPresented: $showDeleteConfirmation,
            titleVisibility: .visible
        ) {
            Button("Delete", role: .destructive) {
                if let id = chatToDelete {
                    state.deletePastChat(chatIdToDelete: id)
                    chatToDelete = nil
                }
            }
            Button("Cancel", role: .cancel) {
                chatToDelete = nil
            }
        } message: {
            Text("Are you sure you want to delete this chat and its messages?")
        }
    }

    private func formattedDate(_ isoString: String) -> String {
        let formatter = ISO8601DateFormatter()
        formatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        if let date = formatter.date(from: isoString) {
            let outputFormatter = DateFormatter()
            outputFormatter.dateStyle = .short
            outputFormatter.timeStyle = .short
            return outputFormatter.string(from: date)
        }
        return isoString
    }
}
