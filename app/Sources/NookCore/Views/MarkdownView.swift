import AppKit
import SwiftUI

public struct MarkdownView: View {
    public let content: String

    public init(content: String) {
        self.content = content
    }

    public var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            ForEach(parseBlocks(content), id: \.id) { block in
                switch block.kind {
                case .text(let text):
                    Text(LocalizedStringKey(text))
                        .textSelection(.enabled)
                        .font(.system(size: 14))
                        .foregroundColor(.primary)

                case .code(let code, let lang):
                    CodeBlockView(code: code, language: lang)
                }
            }
        }
    }

    private struct ParsedBlock: Identifiable {
        let id = UUID()
        enum Kind {
            case text(String)
            case code(String, String?)
        }
        let kind: Kind
    }

    private func parseBlocks(_ markdown: String) -> [ParsedBlock] {
        var blocks: [ParsedBlock] = []
        let lines = markdown.components(separatedBy: "\n")
        var currentText: [String] = []
        var inCodeBlock = false
        var codeLines: [String] = []
        var codeLang: String?

        for line in lines {
            if line.hasPrefix("```") {
                if inCodeBlock {
                    // End code block
                    blocks.append(ParsedBlock(kind: .code(codeLines.joined(separator: "\n"), codeLang)))
                    codeLines.removeAll()
                    inCodeBlock = false
                    codeLang = nil
                } else {
                    // Start code block
                    if !currentText.isEmpty {
                        blocks.append(ParsedBlock(kind: .text(currentText.joined(separator: "\n"))))
                        currentText.removeAll()
                    }
                    let lang = String(line.dropFirst(3)).trimmingCharacters(in: .whitespaces)
                    codeLang = lang.isEmpty ? nil : lang
                    inCodeBlock = true
                }
            } else if inCodeBlock {
                codeLines.append(line)
            } else {
                currentText.append(line)
            }
        }

        if inCodeBlock && !codeLines.isEmpty {
            blocks.append(ParsedBlock(kind: .code(codeLines.joined(separator: "\n"), codeLang)))
        } else if !currentText.isEmpty {
            blocks.append(ParsedBlock(kind: .text(currentText.joined(separator: "\n"))))
        }

        return blocks
    }
}

public struct CodeBlockView: View {
    let code: String
    let language: String?
    @State private var copied: Bool = false

    public init(code: String, language: String? = nil) {
        self.code = code
        self.language = language
    }

    public var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack {
                if let lang = language, !lang.isEmpty {
                    Text(lang.uppercased())
                        .font(.system(size: 10, weight: .bold, design: .monospaced))
                        .foregroundColor(.secondary)
                }
                Spacer()
                Button(action: {
                    let pasteboard = NSPasteboard.general
                    pasteboard.clearContents()
                    pasteboard.setString(code, forType: .string)
                    copied = true
                    DispatchQueue.main.asyncAfter(deadline: .now() + 1.5) {
                        copied = false
                    }
                }) {
                    HStack(spacing: 4) {
                        Image(systemName: copied ? "checkmark" : "doc.on.doc")
                        Text(copied ? "Copied" : "Copy")
                    }
                    .font(.system(size: 11, weight: .medium))
                    .foregroundColor(copied ? .green : .secondary)
                }
                .buttonStyle(.plain)
            }
            .padding(.horizontal, 10)
            .padding(.top, 8)
            .padding(.bottom, 4)

            ScrollView(.horizontal, showsIndicators: false) {
                Text(code)
                    .font(.system(size: 12, design: .monospaced))
                    .textSelection(.enabled)
                    .padding(10)
            }
        }
        .background(Color(NSColor.textBackgroundColor).opacity(0.8))
        .cornerRadius(8)
        .overlay(
            RoundedRectangle(cornerRadius: 8)
                .stroke(Color.secondary.opacity(0.2), lineWidth: 1)
        )
    }
}
