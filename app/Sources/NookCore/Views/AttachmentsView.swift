import SwiftUI

public struct AttachmentsView: View {
    @ObservedObject var state: PanelState

    public init(state: PanelState) {
        self.state = state
    }

    public var body: some View {
        if !state.pendingAttachments.isEmpty {
            ScrollView(.horizontal, showsIndicators: false) {
                HStack(spacing: 8) {
                    ForEach(state.pendingAttachments) { att in
                        AttachmentChip(attachment: att) {
                            state.removeAttachment(id: att.id)
                        }
                    }
                }
                .padding(.horizontal, 4)
                .padding(.vertical, 4)
            }
        }
    }
}

public struct AttachmentChip: View {
    let attachment: PendingAttachment
    let onRemove: () -> Void

    public var body: some View {
        HStack(spacing: 6) {
            Image(systemName: iconName(for: attachment.kind))
                .font(.system(size: 11))
                .foregroundColor(attachment.isValid ? .accentColor : .red)

            Text(attachment.name)
                .font(.system(size: 12))
                .lineLimit(1)
                .foregroundColor(attachment.isValid ? .primary : .red)

            if let error = attachment.errorMessage {
                Text(error)
                    .font(.system(size: 10))
                    .foregroundColor(.white)
                    .padding(.horizontal, 4)
                    .padding(.vertical, 2)
                    .background(Color.red)
                    .cornerRadius(4)
            }

            Button(action: onRemove) {
                Image(systemName: "xmark.circle.fill")
                    .font(.system(size: 12))
                    .foregroundColor(.secondary)
            }
            .buttonStyle(.plain)
        }
        .padding(.horizontal, 8)
        .padding(.vertical, 4)
        .background(
            RoundedRectangle(cornerRadius: 6)
                .fill(attachment.isValid ? Color.secondary.opacity(0.12) : Color.red.opacity(0.1))
        )
        .overlay(
            RoundedRectangle(cornerRadius: 6)
                .stroke(attachment.isValid ? Color.secondary.opacity(0.2) : Color.red, lineWidth: 1)
        )
    }

    private func iconName(for kind: AttachmentKind) -> String {
        switch kind {
        case .image: return "photo"
        case .pdf: return "doc.richtext"
        case .text: return "doc.text"
        }
    }
}
