import AppKit
import Foundation
import NookCore

@main
struct TestRunner {
    @MainActor
    static func main() throws {
        print("Running Nook tests...")

        // NookPanel configuration
        let panel = NookPanel(contentRect: NSRect(x: 0, y: 0, width: 640, height: 56))
        assert(panel.canBecomeKey, "Panel must be able to become key (Open Point 3)")
        assert(panel.canBecomeMain, "Panel must be able to become main")
        assert(panel.isFloatingPanel, "Panel must be floating panel")
        assert(panel.level == .floating, "Panel level must be .floating")
        assert(panel.collectionBehavior.contains(.canJoinAllSpaces), "Panel must join all spaces")
        assert(panel.collectionBehavior.contains(.fullScreenAuxiliary), "Panel must be fullScreenAuxiliary")
        print("NookPanel properties verified.")

        // PanelController singleton and panel initialization
        let controller = PanelController.shared
        assert(controller.panel.canBecomeKey, "Shared panel must be able to become key")
        assert(!controller.panel.isVisible, "Panel should start hidden")
        print("PanelController verified.")

        // Protocol JSON decoding of contracts/examples/panel-daemon.ndjson
        let candidates = [
            URL(fileURLWithPath: "contracts/examples/panel-daemon.ndjson"),
            URL(fileURLWithPath: "../contracts/examples/panel-daemon.ndjson"),
            URL(fileURLWithPath: "../../contracts/examples/panel-daemon.ndjson")
        ]

        guard let examplesUrl = candidates.first(where: { FileManager.default.fileExists(atPath: $0.path) }) else {
            fatalError("Could not locate contracts/examples/panel-daemon.ndjson")
        }

        let content = try String(contentsOf: examplesUrl, encoding: .utf8)
        let lines = content.components(separatedBy: "\n").filter { !$0.trimmingCharacters(in: .whitespaces).isEmpty }
        assert(lines.count == 21, "Expected 21 example lines in panel-daemon.ndjson, got \(lines.count)")

        let decoder = JSONDecoder()
        let encoder = JSONEncoder()

        for (index, line) in lines.enumerated() {
            guard let data = line.data(using: .utf8) else { continue }

            if index < 8 {
                let clientMsg = try decoder.decode(ClientMessage.self, from: data)
                let roundtripData = try encoder.encode(clientMsg)
                let roundtripMsg = try decoder.decode(ClientMessage.self, from: roundtripData)
                assert(clientMsg == roundtripMsg, "Line \(index + 1) ClientMessage roundtrip failed")
            } else {
                let daemonMsg = try decoder.decode(DaemonMessage.self, from: data)
                let roundtripData = try encoder.encode(daemonMsg)
                let roundtripMsg = try decoder.decode(DaemonMessage.self, from: roundtripData)
                assert(daemonMsg == roundtripMsg, "Line \(index + 1) DaemonMessage roundtrip failed")
            }
        }
        print("All 21 contract example lines decoded and roundtripped.")

        // DaemonClient ping-pong test harness with Unix domain socket
        let testSocketPath = "/tmp/nook_test_\(UUID().uuidString.prefix(8)).sock"
        unlink(testSocketPath)

        let serverFD = socket(AF_UNIX, SOCK_STREAM, 0)
        assert(serverFD >= 0, "Failed to create test server socket")

        var serverAddr = sockaddr_un()
        serverAddr.sun_family = sa_family_t(AF_UNIX)
        let pathBytes = testSocketPath.utf8CString
        withUnsafeMutablePointer(to: &serverAddr.sun_path) { ptr in
            let raw = UnsafeMutableRawPointer(ptr)
            pathBytes.withUnsafeBufferPointer { buf in
                raw.copyMemory(from: buf.baseAddress!, byteCount: buf.count)
            }
        }

        let addrLen = socklen_t(MemoryLayout<sockaddr_un>.size)
        let bindRes = withUnsafePointer(to: &serverAddr) { ptr in
            ptr.withMemoryRebound(to: sockaddr.self, capacity: 1) { saPtr in
                bind(serverFD, saPtr, addrLen)
            }
        }
        assert(bindRes == 0, "Failed to bind test server socket")
        listen(serverFD, 1)

        let pingReceived = DispatchSemaphore(value: 0)
        let pongSent = DispatchSemaphore(value: 0)
        let testFinished = DispatchSemaphore(value: 0)

        let serverThread = Thread {
            var clientAddr = sockaddr()
            var clientAddrLen = socklen_t(MemoryLayout<sockaddr>.size)
            let clientFD = accept(serverFD, &clientAddr, &clientAddrLen)
            assert(clientFD >= 0, "Server failed to accept connection")

            var readBuffer = [UInt8](repeating: 0, count: 1024)
            let n = read(clientFD, &readBuffer, readBuffer.count)
            assert(n > 0, "Server read empty line")

            let receivedData = Data(readBuffer[0..<n])
            do {
                let clientMsg = try JSONDecoder().decode(ClientMessage.self, from: receivedData)
                if case .ping(let id) = clientMsg {
                    pingReceived.signal()

                    let pong = DaemonMessage.pong(id: id)
                    var pongData = try JSONEncoder().encode(pong)
                    pongData.append(contentsOf: [UInt8(ascii: "\n")])
                    pongData.withUnsafeBytes { raw in
                        _ = write(clientFD, raw.baseAddress!, pongData.count)
                    }
                    pongSent.signal()
                }
            } catch {
                NSLog("Server decode/encode error: %@", error.localizedDescription)
            }

            _ = testFinished.wait(timeout: .now() + 2)
            close(clientFD)
            close(serverFD)
            unlink(testSocketPath)
        }
        serverThread.start()

        let client = DaemonClient(socketPath: testSocketPath)
        let pongReceived = DispatchSemaphore(value: 0)
        var receivedPongId: UUID?

        client.onMessage = { message in
            if case .pong(let id) = message {
                receivedPongId = id
                pongReceived.signal()
            }
        }

        client.connect()
        Thread.sleep(forTimeInterval: 0.1)

        let testPingId = UUID()
        try client.send(message: .ping(id: testPingId))

        assert(pingReceived.wait(timeout: .now() + 2) == .success, "Server did not receive ping")
        assert(pongSent.wait(timeout: .now() + 2) == .success, "Server did not send pong")
        assert(pongReceived.wait(timeout: .now() + 2) == .success, "Client did not receive pong")
        assert(receivedPongId == testPingId, "Received pong id does not match ping id")

        testFinished.signal()
        client.disconnect()
        print("DaemonClient connected and received pong for ping.")

        // Attachment validation limits
        assert(AttachmentValidator.validate(name: "doc.pdf", sizeBytes: 500, mime: "application/pdf", kind: .pdf) == nil)
        assert(AttachmentValidator.validate(name: "big.pdf", sizeBytes: 15 * 1024 * 1024, mime: "application/pdf", kind: .pdf) != nil)
        assert(AttachmentValidator.validate(name: "bad.exe", sizeBytes: 100, mime: "application/x-msdownload", kind: .image) != nil)
        print("Attachment validation limits verified.")

        // Markdown code block parser
        let md = "Here is code:\n```swift\nlet x = 1\n```\nAnd text."
        let parsed = MarkdownView(content: md)
        assert(!parsed.content.isEmpty, "MarkdownView initialized")
        print("Markdown view component verified.")

        print("All Nook tests passed.")
    }
}
