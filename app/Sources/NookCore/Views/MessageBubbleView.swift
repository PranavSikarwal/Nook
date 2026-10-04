import SwiftUI

public struct MessageBubbleView: View {
    let message: DisplayMessage
    var onRetry: (() -> Void)?

    public init(message: DisplayMessage, onRetry: (() -> Void)? = nil) {
        self.message = message
        self.onRetry = onRetry
    }

    public var body: some View {
        HStack(alignment: .top, spacing: 8) {
            if message.role == .user {
                Spacer(minLength: 40)
                userBubble
            } else {
                assistantBubble
                Spacer(minLength: 40)
            }
        }
    }

    private var userBubble: some View {
        VStack(alignment: .trailing, spacing: 6) {
            if !message.attachments.isEmpty {
                HStack(spacing: 6) {
                    ForEach(message.attachments, id: \.id) { att in
                        HStack(spacing: 4) {
                            Image(systemName: att.kind == .image ? "photo" : (att.kind == .pdf ? "doc.richtext" : "doc.text"))
                                .font(.system(size: 10))
                            Text(att.name)
                                .font(.system(size: 11))
                                .lineLimit(1)
                        }
                        .padding(.horizontal, 6)
                        .padding(.vertical, 3)
                        .background(Color.white.opacity(0.2))
                        .cornerRadius(4)
                    }
                }
            }

            Text(message.text)
                .font(.system(size: 14))
                .foregroundColor(.white)
                .textSelection(.enabled)
                .padding(.horizontal, 12)
                .padding(.vertical, 8)
                .background(Color.accentColor)
                .cornerRadius(12)
        }
    }

    private var assistantBubble: some View {
        VStack(alignment: .leading, spacing: 6) {
            if !message.text.isEmpty {
                MarkdownView(content: message.text)
                    .padding(.horizontal, 12)
                    .padding(.vertical, 8)
                    .background(Color(NSColor.controlBackgroundColor))
                    .cornerRadius(12)
            }

            if message.status == .cancelled {
                HStack(spacing: 4) {
                    Image(systemName: "stop.circle")
                    Text("Cancelled")
                }
                .font(.system(size: 11))
                .foregroundColor(.secondary)
                .padding(.horizontal, 8)
                .padding(.vertical, 4)
            }

            if let error = message.error {
                HStack(spacing: 8) {
                    Image(systemName: "exclamationmark.triangle.fill")
                        .foregroundColor(.red)
                    Text(error.message)
                        .font(.system(size: 12))
                        .foregroundColor(.red)

                    if error.retryable {
                        Spacer()
                        Button(action: {
                            onRetry?()
                        }) {
                            HStack(spacing: 4) {
                                Image(systemName: "arrow.clockwise")
                                Text("Retry")
                            }
                            .font(.system(size: 11, weight: .semibold))
                            .foregroundColor(.accentColor)
                        }
                        .buttonStyle(.plain)
                    }
                }
                .padding(8)
                .background(Color.red.opacity(0.1))
                .cornerRadius(8)
                .overlay(
                    RoundedRectangle(cornerRadius: 8)
                        .stroke(Color.red.opacity(0.3), lineWidth: 1)
                )
            }
        }
    }
}
