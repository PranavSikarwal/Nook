import Foundation

// MARK: - Enums

public enum AttachmentKind: String, Codable, Sendable, Equatable {
    case image
    case text
    case pdf
}

public enum ErrorCode: String, Codable, Sendable, Equatable {
    case endpointUnreachable = "endpoint_unreachable"
    case endpointError = "endpoint_error"
    case workerCrashed = "worker_crashed"
    case databaseUnavailable = "database_unavailable"
    case attachmentInvalid = "attachment_invalid"
    case invalidRequest = "invalid_request"
    case internalError = "internal"
}

public enum MessageRole: String, Codable, Sendable, Equatable {
    case user
    case assistant
}

public enum MessageStatus: String, Codable, Sendable, Equatable {
    case complete
    case cancelled
    case error
}

public enum FinishStatus: String, Codable, Sendable, Equatable {
    case complete
    case cancelled
}

// MARK: - Shared Models

public struct Attachment: Codable, Sendable, Equatable {
    public let id: UUID
    public let kind: AttachmentKind
    public let name: String
    public let mime: String
    public let sizeBytes: Int64
    public let path: String

    public enum CodingKeys: String, CodingKey {
        case id
        case kind
        case name
        case mime
        case sizeBytes = "size_bytes"
        case path
    }

    public init(id: UUID, kind: AttachmentKind, name: String, mime: String, sizeBytes: Int64, path: String) {
        self.id = id
        self.kind = kind
        self.name = name
        self.mime = mime
        self.sizeBytes = sizeBytes
        self.path = path
    }
}

public struct ErrorInfo: Codable, Sendable, Equatable {
    public let code: ErrorCode
    public let message: String
    public let retryable: Bool

    public init(code: ErrorCode, message: String, retryable: Bool) {
        self.code = code
        self.message = message
        self.retryable = retryable
    }
}

public struct ChatMessage: Codable, Sendable, Equatable {
    public let messageId: UUID
    public let role: MessageRole
    public let text: String
    public let status: MessageStatus
    public let error: ErrorInfo?
    public let attachments: [Attachment]
    public let createdAt: String

    public enum CodingKeys: String, CodingKey {
        case messageId = "message_id"
        case role
        case text
        case status
        case error
        case attachments
        case createdAt = "created_at"
    }

    public init(messageId: UUID, role: MessageRole, text: String, status: MessageStatus, error: ErrorInfo?, attachments: [Attachment], createdAt: String) {
        self.messageId = messageId
        self.role = role
        self.text = text
        self.status = status
        self.error = error
        self.attachments = attachments
        self.createdAt = createdAt
    }
}

public struct ChatSummary: Codable, Sendable, Equatable {
    public let chatId: UUID
    public let title: String
    public let updatedAt: String

    public enum CodingKeys: String, CodingKey {
        case chatId = "chat_id"
        case title
        case updatedAt = "updated_at"
    }

    public init(chatId: UUID, title: String, updatedAt: String) {
        self.chatId = chatId
        self.title = title
        self.updatedAt = updatedAt
    }
}

// MARK: - Client Messages (Panel -> Daemon)

public enum ClientMessage: Codable, Sendable, Equatable {
    case sendMessage(id: UUID, chatId: UUID, text: String, attachments: [Attachment])
    case cancel(id: UUID, targetId: UUID)
    case listChats(id: UUID)
    case getChat(id: UUID, chatId: UUID)
    case deleteChat(id: UUID, chatId: UUID)
    case getSettings(id: UUID)
    case setSettings(id: UUID, baseUrl: String, model: String, apiKey: String?)
    case ping(id: UUID)

    private enum CodingKeys: String, CodingKey {
        case type
        case id
        case chatId = "chat_id"
        case text
        case attachments
        case targetId = "target_id"
        case baseUrl = "base_url"
        case model
        case apiKey = "api_key"
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let type = try container.decode(String.self, forKey: .type)

        switch type {
        case "send_message":
            let id = try container.decode(UUID.self, forKey: .id)
            let chatId = try container.decode(UUID.self, forKey: .chatId)
            let text = try container.decode(String.self, forKey: .text)
            let attachments = try container.decode([Attachment].self, forKey: .attachments)
            self = .sendMessage(id: id, chatId: chatId, text: text, attachments: attachments)

        case "cancel":
            let id = try container.decode(UUID.self, forKey: .id)
            let targetId = try container.decode(UUID.self, forKey: .targetId)
            self = .cancel(id: id, targetId: targetId)

        case "list_chats":
            let id = try container.decode(UUID.self, forKey: .id)
            self = .listChats(id: id)

        case "get_chat":
            let id = try container.decode(UUID.self, forKey: .id)
            let chatId = try container.decode(UUID.self, forKey: .chatId)
            self = .getChat(id: id, chatId: chatId)

        case "delete_chat":
            let id = try container.decode(UUID.self, forKey: .id)
            let chatId = try container.decode(UUID.self, forKey: .chatId)
            self = .deleteChat(id: id, chatId: chatId)

        case "get_settings":
            let id = try container.decode(UUID.self, forKey: .id)
            self = .getSettings(id: id)

        case "set_settings":
            let id = try container.decode(UUID.self, forKey: .id)
            let baseUrl = try container.decode(String.self, forKey: .baseUrl)
            let model = try container.decode(String.self, forKey: .model)
            let apiKey = try container.decodeIfPresent(String.self, forKey: .apiKey)
            self = .setSettings(id: id, baseUrl: baseUrl, model: model, apiKey: apiKey)

        case "ping":
            let id = try container.decode(UUID.self, forKey: .id)
            self = .ping(id: id)

        default:
            throw DecodingError.dataCorruptedError(forKey: .type, in: container, debugDescription: "Unknown client message type: \(type)")
        }
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        switch self {
        case .sendMessage(let id, let chatId, let text, let attachments):
            try container.encode("send_message", forKey: .type)
            try container.encode(id, forKey: .id)
            try container.encode(chatId, forKey: .chatId)
            try container.encode(text, forKey: .text)
            try container.encode(attachments, forKey: .attachments)

        case .cancel(let id, let targetId):
            try container.encode("cancel", forKey: .type)
            try container.encode(id, forKey: .id)
            try container.encode(targetId, forKey: .targetId)

        case .listChats(let id):
            try container.encode("list_chats", forKey: .type)
            try container.encode(id, forKey: .id)

        case .getChat(let id, let chatId):
            try container.encode("get_chat", forKey: .type)
            try container.encode(id, forKey: .id)
            try container.encode(chatId, forKey: .chatId)

        case .deleteChat(let id, let chatId):
            try container.encode("delete_chat", forKey: .type)
            try container.encode(id, forKey: .id)
            try container.encode(chatId, forKey: .chatId)

        case .getSettings(let id):
            try container.encode("get_settings", forKey: .type)
            try container.encode(id, forKey: .id)

        case .setSettings(let id, let baseUrl, let model, let apiKey):
            try container.encode("set_settings", forKey: .type)
            try container.encode(id, forKey: .id)
            try container.encode(baseUrl, forKey: .baseUrl)
            try container.encode(model, forKey: .model)
            try container.encodeIfPresent(apiKey, forKey: .apiKey)

        case .ping(let id):
            try container.encode("ping", forKey: .type)
            try container.encode(id, forKey: .id)
        }
    }
}

// MARK: - Daemon Messages (Daemon -> Panel)

public enum DaemonMessage: Codable, Sendable, Equatable {
    case messageStarted(id: UUID, chatId: UUID, messageId: UUID)
    case textDelta(id: UUID, chatId: UUID, messageId: UUID, text: String)
    case toolCallStarted(id: UUID, chatId: UUID, messageId: UUID, callId: String, name: String, arguments: String)
    case toolCallFinished(id: UUID, chatId: UUID, messageId: UUID, callId: String, result: String)
    case messageFinished(id: UUID, chatId: UUID, messageId: UUID, status: FinishStatus)
    case chatTitled(chatId: UUID, title: String)
    case chats(id: UUID, chats: [ChatSummary])
    case chat(id: UUID, chatId: UUID, title: String, messages: [ChatMessage])
    case deleted(id: UUID, chatId: UUID)
    case settings(id: UUID, baseUrl: String, model: String, hasApiKey: Bool)
    case cancelled(id: UUID, targetId: UUID)
    case pong(id: UUID)
    case error(id: UUID, error: ErrorInfo)

    private enum CodingKeys: String, CodingKey {
        case type
        case id
        case chatId = "chat_id"
        case messageId = "message_id"
        case text
        case callId = "call_id"
        case name
        case arguments
        case result
        case status
        case title
        case chats
        case messages
        case baseUrl = "base_url"
        case model
        case hasApiKey = "has_api_key"
        case targetId = "target_id"
        case error
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let type = try container.decode(String.self, forKey: .type)

        switch type {
        case "message_started":
            let id = try container.decode(UUID.self, forKey: .id)
            let chatId = try container.decode(UUID.self, forKey: .chatId)
            let messageId = try container.decode(UUID.self, forKey: .messageId)
            self = .messageStarted(id: id, chatId: chatId, messageId: messageId)

        case "text_delta":
            let id = try container.decode(UUID.self, forKey: .id)
            let chatId = try container.decode(UUID.self, forKey: .chatId)
            let messageId = try container.decode(UUID.self, forKey: .messageId)
            let text = try container.decode(String.self, forKey: .text)
            self = .textDelta(id: id, chatId: chatId, messageId: messageId, text: text)

        case "tool_call_started":
            let id = try container.decode(UUID.self, forKey: .id)
            let chatId = try container.decode(UUID.self, forKey: .chatId)
            let messageId = try container.decode(UUID.self, forKey: .messageId)
            let callId = try container.decode(String.self, forKey: .callId)
            let name = try container.decode(String.self, forKey: .name)
            let arguments = try container.decode(String.self, forKey: .arguments)
            self = .toolCallStarted(id: id, chatId: chatId, messageId: messageId, callId: callId, name: name, arguments: arguments)

        case "tool_call_finished":
            let id = try container.decode(UUID.self, forKey: .id)
            let chatId = try container.decode(UUID.self, forKey: .chatId)
            let messageId = try container.decode(UUID.self, forKey: .messageId)
            let callId = try container.decode(String.self, forKey: .callId)
            let result = try container.decode(String.self, forKey: .result)
            self = .toolCallFinished(id: id, chatId: chatId, messageId: messageId, callId: callId, result: result)

        case "message_finished":
            let id = try container.decode(UUID.self, forKey: .id)
            let chatId = try container.decode(UUID.self, forKey: .chatId)
            let messageId = try container.decode(UUID.self, forKey: .messageId)
            let status = try container.decode(FinishStatus.self, forKey: .status)
            self = .messageFinished(id: id, chatId: chatId, messageId: messageId, status: status)

        case "chat_titled":
            let chatId = try container.decode(UUID.self, forKey: .chatId)
            let title = try container.decode(String.self, forKey: .title)
            self = .chatTitled(chatId: chatId, title: title)

        case "chats":
            let id = try container.decode(UUID.self, forKey: .id)
            let chats = try container.decode([ChatSummary].self, forKey: .chats)
            self = .chats(id: id, chats: chats)

        case "chat":
            let id = try container.decode(UUID.self, forKey: .id)
            let chatId = try container.decode(UUID.self, forKey: .chatId)
            let title = try container.decode(String.self, forKey: .title)
            let messages = try container.decode([ChatMessage].self, forKey: .messages)
            self = .chat(id: id, chatId: chatId, title: title, messages: messages)

        case "deleted":
            let id = try container.decode(UUID.self, forKey: .id)
            let chatId = try container.decode(UUID.self, forKey: .chatId)
            self = .deleted(id: id, chatId: chatId)

        case "settings":
            let id = try container.decode(UUID.self, forKey: .id)
            let baseUrl = try container.decode(String.self, forKey: .baseUrl)
            let model = try container.decode(String.self, forKey: .model)
            let hasApiKey = try container.decode(Bool.self, forKey: .hasApiKey)
            self = .settings(id: id, baseUrl: baseUrl, model: model, hasApiKey: hasApiKey)

        case "cancelled":
            let id = try container.decode(UUID.self, forKey: .id)
            let targetId = try container.decode(UUID.self, forKey: .targetId)
            self = .cancelled(id: id, targetId: targetId)

        case "pong":
            let id = try container.decode(UUID.self, forKey: .id)
            self = .pong(id: id)

        case "error":
            let id = try container.decode(UUID.self, forKey: .id)
            let error = try container.decode(ErrorInfo.self, forKey: .error)
            self = .error(id: id, error: error)

        default:
            throw DecodingError.dataCorruptedError(forKey: .type, in: container, debugDescription: "Unknown daemon message type: \(type)")
        }
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        switch self {
        case .messageStarted(let id, let chatId, let messageId):
            try container.encode("message_started", forKey: .type)
            try container.encode(id, forKey: .id)
            try container.encode(chatId, forKey: .chatId)
            try container.encode(messageId, forKey: .messageId)

        case .textDelta(let id, let chatId, let messageId, let text):
            try container.encode("text_delta", forKey: .type)
            try container.encode(id, forKey: .id)
            try container.encode(chatId, forKey: .chatId)
            try container.encode(messageId, forKey: .messageId)
            try container.encode(text, forKey: .text)

        case .toolCallStarted(let id, let chatId, let messageId, let callId, let name, let arguments):
            try container.encode("tool_call_started", forKey: .type)
            try container.encode(id, forKey: .id)
            try container.encode(chatId, forKey: .chatId)
            try container.encode(messageId, forKey: .messageId)
            try container.encode(callId, forKey: .callId)
            try container.encode(name, forKey: .name)
            try container.encode(arguments, forKey: .arguments)

        case .toolCallFinished(let id, let chatId, let messageId, let callId, let result):
            try container.encode("tool_call_finished", forKey: .type)
            try container.encode(id, forKey: .id)
            try container.encode(chatId, forKey: .chatId)
            try container.encode(messageId, forKey: .messageId)
            try container.encode(callId, forKey: .callId)
            try container.encode(result, forKey: .result)

        case .messageFinished(let id, let chatId, let messageId, let status):
            try container.encode("message_finished", forKey: .type)
            try container.encode(id, forKey: .id)
            try container.encode(chatId, forKey: .chatId)
            try container.encode(messageId, forKey: .messageId)
            try container.encode(status, forKey: .status)

        case .chatTitled(let chatId, let title):
            try container.encode("chat_titled", forKey: .type)
            try container.encode(chatId, forKey: .chatId)
            try container.encode(title, forKey: .title)

        case .chats(let id, let chats):
            try container.encode("chats", forKey: .type)
            try container.encode(id, forKey: .id)
            try container.encode(chats, forKey: .chats)

        case .chat(let id, let chatId, let title, let messages):
            try container.encode("chat", forKey: .type)
            try container.encode(id, forKey: .id)
            try container.encode(chatId, forKey: .chatId)
            try container.encode(title, forKey: .title)
            try container.encode(messages, forKey: .messages)

        case .deleted(let id, let chatId):
            try container.encode("deleted", forKey: .type)
            try container.encode(id, forKey: .id)
            try container.encode(chatId, forKey: .chatId)

        case .settings(let id, let baseUrl, let model, let hasApiKey):
            try container.encode("settings", forKey: .type)
            try container.encode(id, forKey: .id)
            try container.encode(baseUrl, forKey: .baseUrl)
            try container.encode(model, forKey: .model)
            try container.encode(hasApiKey, forKey: .hasApiKey)

        case .cancelled(let id, let targetId):
            try container.encode("cancelled", forKey: .type)
            try container.encode(id, forKey: .id)
            try container.encode(targetId, forKey: .targetId)

        case .pong(let id):
            try container.encode("pong", forKey: .type)
            try container.encode(id, forKey: .id)

        case .error(let id, let error):
            try container.encode("error", forKey: .type)
            try container.encode(id, forKey: .id)
            try container.encode(error, forKey: .error)
        }
    }
}
