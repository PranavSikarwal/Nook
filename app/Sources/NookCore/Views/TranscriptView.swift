import SwiftUI

public struct TranscriptView: View {
    @ObservedObject var state: PanelState

    public init(state: PanelState) {
        self.state = state
    }

    public var body: some View {
        ScrollViewReader { proxy in
            ScrollView {
                LazyVStack(spacing: 12) {
                    ForEach(state.messages) { msg in
                        MessageBubbleView(message: msg) {
                            state.retryLast()
                        }
                        .id(msg.id)
                    }
                }
                .padding(.horizontal, 12)
                .padding(.vertical, 8)
            }
            .onChange(of: state.messages.count) {
                if let lastId = state.messages.last?.id {
                    withAnimation {
                        proxy.scrollTo(lastId, anchor: .bottom)
                    }
                }
            }
            .onChange(of: state.messages.last?.text) {
                if let lastId = state.messages.last?.id {
                    proxy.scrollTo(lastId, anchor: .bottom)
                }
            }
        }
    }
}
