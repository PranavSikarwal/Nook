import AppKit
import Foundation
import UniformTypeIdentifiers

public struct PendingAttachment: Identifiable, Equatable, Sendable {
    public let id: UUID
    public let name: String
    public let kind: AttachmentKind
    public let mime: String
    public let sizeBytes: Int64
    public let sourceURL: URL?
    public let data: Data?
    public let errorMessage: String?

    public var isValid: Bool {
        return errorMessage == nil
    }

    public init(id: UUID = UUID(), name: String, kind: AttachmentKind, mime: String, sizeBytes: Int64, sourceURL: URL?, data: Data? = nil, errorMessage: String? = nil) {
        self.id = id
        self.name = name
        self.kind = kind
        self.mime = mime
        self.sizeBytes = sizeBytes
        self.sourceURL = sourceURL
        self.data = data
        self.errorMessage = errorMessage
    }
}

public enum AttachmentValidator {
    public static let maxFileBytes: Int64 = 10 * 1024 * 1024 // 10 MB
    public static let maxAttachments = 5

    public static func validate(name _: String, sizeBytes: Int64, mime: String, kind: AttachmentKind) -> String? {
        if sizeBytes > maxFileBytes {
            return "File exceeds 10 MB limit (\(sizeBytes / (1024 * 1024)) MB)"
        }

        let lowerMime = mime.lowercased()
        switch kind {
        case .image:
            let allowed = ["image/png", "image/jpeg", "image/jpg", "image/webp", "image/gif"]
            if !allowed.contains(lowerMime) {
                return "Unsupported image type: \(mime)"
            }
        case .pdf:
            if lowerMime != "application/pdf" {
                return "Unsupported PDF type: \(mime)"
            }
        case .text:
            let allowedExact = [
                "application/json", "text/csv", "text/markdown", "text/plain",
                "application/javascript", "application/typescript", "application/xml",
                "application/x-yaml", "text/yaml"
            ]
            if !lowerMime.hasPrefix("text/") && !allowedExact.contains(lowerMime) {
                return "Unsupported text type: \(mime)"
            }
        }
        return nil
    }

    public static func detectKindAndMime(url: URL) -> (AttachmentKind, String) {
        let ext = url.pathExtension.lowercased()
        if let utType = UTType(filenameExtension: ext) {
            let mime = utType.preferredMIMEType ?? "application/octet-stream"
            if utType.conforms(to: .image) {
                return (.image, mime)
            } else if utType.conforms(to: .pdf) {
                return (.pdf, "application/pdf")
            } else {
                return (.text, mime)
            }
        }

        switch ext {
        case "png": return (.image, "image/png")
        case "jpg", "jpeg": return (.image, "image/jpeg")
        case "webp": return (.image, "image/webp")
        case "gif": return (.image, "image/gif")
        case "pdf": return (.pdf, "application/pdf")
        case "json": return (.text, "application/json")
        case "csv": return (.text, "text/csv")
        case "md", "markdown": return (.text, "text/markdown")
        default: return (.text, "text/plain")
        }
    }

    public static func copyToStorage(attachment: PendingAttachment, chatId: UUID) throws -> Attachment {
        let appSupport = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask).first!
        let chatDir = appSupport.appendingPathComponent("Nook/attachments/\(chatId.uuidString)")
        try FileManager.default.createDirectory(at: chatDir, withIntermediateDirectories: true)

        let sanitizedName = URL(fileURLWithPath: attachment.name).lastPathComponent
            .replacingOccurrences(of: "\0", with: "")
            .replacingOccurrences(of: "/", with: "")
            .trimmingCharacters(in: .whitespacesAndNewlines)
        let safeName = sanitizedName.isEmpty ? "attachment" : sanitizedName

        let destFileName = "\(attachment.id.uuidString)-\(safeName)"
        let destURL = chatDir.appendingPathComponent(destFileName)

        if let data = attachment.data {
            try data.write(to: destURL, options: .atomic)
        } else if let srcURL = attachment.sourceURL {
            if FileManager.default.fileExists(atPath: destURL.path) {
                try FileManager.default.removeItem(at: destURL)
            }
            try FileManager.default.copyItem(at: srcURL, to: destURL)
        }

        return Attachment(
            id: attachment.id,
            kind: attachment.kind,
            name: safeName,
            mime: attachment.mime,
            sizeBytes: attachment.sizeBytes,
            path: destURL.path
        )
    }

    public static func fromPasteboard() -> [PendingAttachment] {
        let pasteboard = NSPasteboard.general
        var results: [PendingAttachment] = []

        // 1. Check for file URLs
        if let urls = pasteboard.readObjects(forClasses: [NSURL.self], options: nil) as? [URL], !urls.isEmpty {
            for url in urls where url.isFileURL {
                let name = url.lastPathComponent
                let size = (try? FileManager.default.attributesOfItem(atPath: url.path)[.size] as? Int64) ?? 0
                let (kind, mime) = detectKindAndMime(url: url)
                let error = validate(name: name, sizeBytes: size, mime: mime, kind: kind)
                results.append(PendingAttachment(
                    name: name,
                    kind: kind,
                    mime: mime,
                    sizeBytes: size,
                    sourceURL: url,
                    errorMessage: error
                ))
            }
            return results
        }

        // 2. Check for image data
        if let image = NSImage(pasteboard: pasteboard),
           let tiffData = image.tiffRepresentation,
           let bitmap = NSBitmapImageRep(data: tiffData),
           let pngData = bitmap.representation(using: .png, properties: [:]) {
            let id = UUID()
            let size = Int64(pngData.count)
            let error = validate(name: "pasted_image.png", sizeBytes: size, mime: "image/png", kind: .image)
            results.append(PendingAttachment(
                id: id,
                name: "pasted_image.png",
                kind: .image,
                mime: "image/png",
                sizeBytes: size,
                sourceURL: nil,
                data: pngData,
                errorMessage: error
            ))
        }

        return results
    }

    public static func openFilePicker(completion: @escaping ([PendingAttachment]) -> Void) {
        let panel = NSOpenPanel()
        panel.allowsMultipleSelection = true
        panel.canChooseDirectories = false
        panel.canChooseFiles = true
        panel.allowedContentTypes = [
            .image, .pdf, .text, .plainText, .json, .commaSeparatedText
        ]

        panel.begin { response in
            guard response == .OK else { return }
            var results: [PendingAttachment] = []
            for url in panel.urls {
                let name = url.lastPathComponent
                let size = (try? FileManager.default.attributesOfItem(atPath: url.path)[.size] as? Int64) ?? 0
                let (kind, mime) = detectKindAndMime(url: url)
                let error = validate(name: name, sizeBytes: size, mime: mime, kind: kind)
                results.append(PendingAttachment(
                    name: name,
                    kind: kind,
                    mime: mime,
                    sizeBytes: size,
                    sourceURL: url,
                    errorMessage: error
                ))
            }
            completion(results)
        }
    }
}
