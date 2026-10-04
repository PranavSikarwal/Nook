import AppKit
import Foundation
import SwiftUI

public enum ViewMode: Equatable {
    case compact
    case expanded
    case history
    case settings
}

public struct DisplayMessage: Identifiable, Equatable {
    public let id: UUID
    public let role: MessageRole
    public var text: String
    public var status: MessageStatus
    public var error: ErrorInfo?
    public let attachments: [Attachment]
    public let createdAt: String

    public init(id: UUID = UUID(), role: MessageRole, text: String, status: MessageStatus = .complete, error: ErrorInfo? = nil, attachments: [Attachment] = [], createdAt: String = ISO8601DateFormatter().string(from: Date())) {
        self.id = id
        self.role = role
        self.text = text
        self.status = status
        self.error = error
        self.attachments = attachments
        self.createdAt = createdAt
    }
}

@MainActor
public final class PanelState: ObservableObject {
    public static let shared = PanelState()

    @Published public var chatId: UUID = UUID()
    @Published public var messages: [DisplayMessage] = []
    @Published public var isStreaming: Bool = false
    @Published public var isConnected: Bool = false
    @Published public var viewMode: ViewMode = .compact
    @Published public var pendingAttachments: [PendingAttachment] = []
    @Published public var historyChats: [ChatSummary] = []
    @Published public var currentTitle: String = "New Chat"

    // Settings fields
    @Published public var baseUrl: String = "http://127.0.0.1:8000/v1"
    @Published public var model: String = "gpt-4o"
    @Published public var hasApiKey: Bool = false
    @Published public var apiKeyInput: String = ""
    @Published public var hotkeyDisplay: String = "⌥Space"

    private var activeRequestId: UUID?
    private var client: DaemonClient { DaemonClient.shared }

    private init() {
        self.client.onMessage = { [weak self] message in
            Task { @MainActor in
                self?.handleDaemonMessage(message)
            }
        }
    }

    public func startNewChat() {
        chatId = UUID()
        messages = []
        isStreaming = false
        pendingAttachments = []
        currentTitle = "New Chat"
        viewMode = .compact
        activeRequestId = nil
    }

    public func sendMessage(text: String) {
        let validAttachments = pendingAttachments.filter { $0.isValid }
        guard !text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || !validAttachments.isEmpty else {
            return
        }

        var savedAttachments: [Attachment] = []
        for att in validAttachments {
            if let saved = try? AttachmentValidator.copyToStorage(attachment: att, chatId: chatId) {
                savedAttachments.append(saved)
            }
        }
        pendingAttachments.removeAll()

        let userMsg = DisplayMessage(
            role: .user,
            text: text,
            status: .complete,
            attachments: savedAttachments
        )
        messages.append(userMsg)

        // Expand view on first message
        if viewMode == .compact {
            viewMode = .expanded
        }

        let reqId = UUID()
        activeRequestId = reqId
        isStreaming = true

        let clientMsg = ClientMessage.sendMessage(
            id: reqId,
            chatId: chatId,
            text: text,
            attachments: savedAttachments
        )

        do {
            try client.send(message: clientMsg)
        } catch {
            isStreaming = false
            messages.append(DisplayMessage(
                role: .assistant,
                text: "",
                status: .error,
                error: ErrorInfo(code: .internalError, message: error.localizedDescription, retryable: true)
            ))
        }
    }

    public func stopStreaming() {
        guard let reqId = activeRequestId else { return }
        let cancelMsg = ClientMessage.cancel(id: UUID(), targetId: reqId)
        try? client.send(message: cancelMsg)
        isStreaming = false
    }

    public func retryLast() {
        guard let lastUserMsg = messages.last(where: { $0.role == .user }) else { return }
        sendMessage(text: lastUserMsg.text)
    }

    public func toggleHistory() {
        if viewMode == .history {
            viewMode = messages.isEmpty ? .compact : .expanded
        } else {
            viewMode = .history
            loadHistory()
        }
    }

    public func loadHistory() {
        let reqId = UUID()
        let msg = ClientMessage.listChats(id: reqId)
        try? client.send(message: msg)
    }

    public func openPastChat(selectedChatId: UUID) {
        let reqId = UUID()
        let msg = ClientMessage.getChat(id: reqId, chatId: selectedChatId)
        try? client.send(message: msg)
    }

    public func deletePastChat(chatIdToDelete: UUID) {
        let reqId = UUID()
        let msg = ClientMessage.deleteChat(id: reqId, chatId: chatIdToDelete)
        try? client.send(message: msg)
        historyChats.removeAll { $0.chatId == chatIdToDelete }
        if chatId == chatIdToDelete {
            startNewChat()
        }
    }

    public func toggleSettings() {
        if viewMode == .settings {
            viewMode = messages.isEmpty ? .compact : .expanded
        } else {
            viewMode = .settings
            loadSettings()
        }
    }

    public func loadSettings() {
        let reqId = UUID()
        let msg = ClientMessage.getSettings(id: reqId)
        try? client.send(message: msg)
    }

    public func saveSettings() {
        let reqId = UUID()
        let key = apiKeyInput.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ? nil : apiKeyInput
        let msg = ClientMessage.setSettings(id: reqId, baseUrl: baseUrl, model: model, apiKey: key)
        try? client.send(message: msg)
        apiKeyInput = ""
        viewMode = messages.isEmpty ? .compact : .expanded
    }

    public func addAttachments(_ attachments: [PendingAttachment]) {
        for att in attachments {
            if pendingAttachments.count >= AttachmentValidator.maxAttachments {
                break
            }
            pendingAttachments.append(att)
        }
    }

    public func removeAttachment(id: UUID) {
        pendingAttachments.removeAll { $0.id == id }
    }

    private func handleDaemonMessage(_ message: DaemonMessage) {
        switch message {
        case .messageStarted(_, _, let messageId):
            messages.append(DisplayMessage(
                id: messageId,
                role: .assistant,
                text: "",
                status: .complete
            ))

        case .textDelta(_, _, let messageId, let delta):
            if let index = messages.firstIndex(where: { $0.id == messageId }) {
                messages[index].text += delta
            } else if let lastIndex = messages.indices.last, messages[lastIndex].role == .assistant {
                messages[lastIndex].text += delta
            }

        case .messageFinished(_, _, let messageId, let status):
            isStreaming = false
            activeRequestId = nil
            if let index = messages.firstIndex(where: { $0.id == messageId }) {
                messages[index].status = (status == .cancelled) ? .cancelled : .complete
            }

        case .error(let id, let errorInfo):
            if id == activeRequestId {
                isStreaming = false
                activeRequestId = nil
            }
            if let lastIndex = messages.indices.last, messages[lastIndex].role == .assistant {
                messages[lastIndex].status = .error
                messages[lastIndex].error = errorInfo
            } else {
                messages.append(DisplayMessage(
                    role: .assistant,
                    text: "",
                    status: .error,
                    error: errorInfo
                ))
            }

        case .chatTitled(let titledChatId, let title):
            if chatId == titledChatId {
                currentTitle = title
            }
            if let idx = historyChats.firstIndex(where: { $0.chatId == titledChatId }) {
                historyChats[idx] = ChatSummary(chatId: titledChatId, title: title, updatedAt: historyChats[idx].updatedAt)
            }

        case .chats(_, let chats):
            historyChats = chats

        case .chat(_, let openChatId, let title, let chatMessages):
            chatId = openChatId
            currentTitle = title
            messages = chatMessages.map { m in
                DisplayMessage(
                    id: m.messageId,
                    role: m.role,
                    text: m.text,
                    status: m.status,
                    error: m.error,
                    attachments: m.attachments,
                    createdAt: m.createdAt
                )
            }
            viewMode = .expanded

        case .settings(_, let base, let mod, let hasKey):
            baseUrl = base
            model = mod
            hasApiKey = hasKey

        default:
            break
        }
    }
}
