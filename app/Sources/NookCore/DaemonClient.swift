import Combine
import Foundation

@MainActor
public protocol DaemonClientDelegate: AnyObject {
    func daemonClient(_ client: DaemonClient, didChangeConnectionState isConnected: Bool)
    func daemonClient(_ client: DaemonClient, didReceiveEvent event: DaemonMessage)
}

public final class DaemonClient: @unchecked Sendable {
    public static let shared = DaemonClient()

    public static func defaultSocketPath() -> String {
        let appSupport = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask).first!
        return appSupport.appendingPathComponent("Nook/daemon.sock").path
    }

    public weak var delegate: DaemonClientDelegate?

    public let messages = PassthroughSubject<DaemonMessage, Never>()
    public let isConnectedPublisher = CurrentValueSubject<Bool, Never>(false)

    private let socketPath: String
    private var socketFD: Int32 = -1
    private var readSource: DispatchSourceRead?
    private let queue = DispatchQueue(label: "tech.nook.daemonclient", qos: .userInitiated)
    private var buffer = Data()
    private let decoder = JSONDecoder()
    private let encoder = JSONEncoder()

    private var reconnectTimer: DispatchSourceTimer?
    private var reconnectAttempts = 0
    private let maxReconnectAttempts = 10
    private var isExplicitlyDisconnected = false

    public private(set) var isConnected: Bool = false {
        didSet {
            if oldValue != isConnected {
                isConnectedPublisher.send(isConnected)
                DispatchQueue.main.async { [weak self] in
                    guard let self = self else { return }
                    self.delegate?.daemonClient(self, didChangeConnectionState: self.isConnected)
                }
            }
        }
    }

    public var onMessage: ((DaemonMessage) -> Void)?

    public init(socketPath: String = DaemonClient.defaultSocketPath()) {
        self.socketPath = socketPath
    }

    public func connect() {
        queue.async { [weak self] in
            self?.isExplicitlyDisconnected = false
            self?.reconnectAttempts = 0
            self?.attemptConnect()
        }
    }

    public func disconnect() {
        queue.async { [weak self] in
            self?.isExplicitlyDisconnected = true
            self?.stopReconnectTimer()
            self?.closeSocket()
        }
    }

    private func attemptConnect() {
        if isConnected { return }

        let fd = socket(AF_UNIX, SOCK_STREAM, 0)
        if fd < 0 {
            NSLog("Failed to create socket: %s", strerror(errno))
            scheduleReconnect()
            return
        }

        var opt: Int32 = 1
        setsockopt(fd, SOL_SOCKET, SO_NOSIGPIPE, &opt, socklen_t(MemoryLayout<Int32>.size))

        var addr = sockaddr_un()
        addr.sun_family = sa_family_t(AF_UNIX)

        let pathBytes = socketPath.utf8CString
        if pathBytes.count > MemoryLayout.size(ofValue: addr.sun_path) {
            NSLog("Socket path too long: %@", socketPath)
            close(fd)
            return
        }

        withUnsafeMutablePointer(to: &addr.sun_path) { ptr in
            let rawPtr = UnsafeMutableRawPointer(ptr)
            pathBytes.withUnsafeBufferPointer { bufPtr in
                rawPtr.copyMemory(from: bufPtr.baseAddress!, byteCount: bufPtr.count)
            }
        }

        let addrLen = socklen_t(MemoryLayout<sockaddr_un>.size)
        let connectRes = withUnsafePointer(to: &addr) { ptr in
            ptr.withMemoryRebound(to: sockaddr.self, capacity: 1) { saPtr in
                Darwin.connect(fd, saPtr, addrLen)
            }
        }

        if connectRes < 0 {
            close(fd)
            scheduleReconnect()
            return
        }

        self.socketFD = fd
        self.isConnected = true
        self.reconnectAttempts = 0
        stopReconnectTimer()

        setupReadSource(fd: fd)
    }

    private func setupReadSource(fd: Int32) {
        let source = DispatchSource.makeReadSource(fileDescriptor: fd, queue: queue)
        source.setEventHandler { [weak self] in
            self?.readFromSocket()
        }
        source.setCancelHandler {
            Darwin.close(fd)
        }
        self.readSource = source
        source.resume()
    }

    private func readFromSocket() {
        var tempBuffer = [UInt8](repeating: 0, count: 4096)
        let bytesRead = Darwin.read(socketFD, &tempBuffer, tempBuffer.count)

        if bytesRead > 0 {
            buffer.append(tempBuffer, count: bytesRead)
            processBuffer()
        } else if bytesRead == 0 {
            // EOF
            NSLog("Daemon closed connection")
            closeSocket()
            scheduleReconnect()
        } else if errno != EAGAIN && errno != EWOULDBLOCK {
            NSLog("Socket read error: %s", strerror(errno))
            closeSocket()
            scheduleReconnect()
        }
    }

    private func processBuffer() {
        let newline = UInt8(ascii: "\n")
        while let newlineIndex = buffer.firstIndex(of: newline) {
            let lineData = buffer.subdata(in: 0..<newlineIndex)
            buffer.removeSubrange(0...newlineIndex)

            guard !lineData.isEmpty else { continue }
            do {
                let message = try decoder.decode(DaemonMessage.self, from: lineData)
                self.messages.send(message)
                self.onMessage?(message)
                DispatchQueue.main.async { [weak self] in
                    guard let self = self else { return }
                    self.delegate?.daemonClient(self, didReceiveEvent: message)
                }
            } catch {
                NSLog("Failed to decode daemon message: %@", error.localizedDescription)
            }
        }
    }

    private func closeSocket() {
        isConnected = false
        if let source = readSource {
            source.cancel()
            readSource = nil
            socketFD = -1
        } else if socketFD >= 0 {
            Darwin.close(socketFD)
            socketFD = -1
        }
        buffer.removeAll()
    }

    private func scheduleReconnect() {
        isConnected = false
        if isExplicitlyDisconnected {
            return
        }
        if reconnectAttempts >= maxReconnectAttempts {
            NSLog("Exceeded max daemon reconnect attempts (%d)", maxReconnectAttempts)
            return
        }

        reconnectAttempts += 1
        if reconnectTimer == nil {
            let timer = DispatchSource.makeTimerSource(queue: queue)
            timer.schedule(deadline: .now() + 1.0, repeating: 1.0)
            timer.setEventHandler { [weak self] in
                guard let self = self else { return }
                if !self.isConnected {
                    self.attemptConnect()
                } else {
                    self.stopReconnectTimer()
                }
            }
            self.reconnectTimer = timer
            timer.resume()
        }
    }

    private func stopReconnectTimer() {
        if let timer = reconnectTimer {
            timer.cancel()
            reconnectTimer = nil
        }
    }

    public func send(message: ClientMessage) throws {
        var data = try encoder.encode(message)
        data.append(contentsOf: [UInt8(ascii: "\n")])

        queue.async { [weak self] in
            guard let self = self, self.socketFD >= 0, self.isConnected else {
                NSLog("Cannot send message: socket not connected")
                return
            }
            data.withUnsafeBytes { rawBuf in
                guard let base = rawBuf.baseAddress else { return }
                var totalWritten = 0
                while totalWritten < data.count {
                    let written = Darwin.write(self.socketFD, base.advanced(by: totalWritten), data.count - totalWritten)
                    if written <= 0 {
                        let err = errno
                        NSLog("Failed to write to daemon socket: %s", strerror(err))
                        if err == EPIPE || err == ECONNRESET {
                            self.closeSocket()
                            self.scheduleReconnect()
                        }
                        break
                    }
                    totalWritten += written
                }
            }
        }
    }
}
