# Nook

Nook is a macOS overlay that lets one user ask an AI agent a question with a keystroke, read the answer, and dismiss it. Version 1 is a general chatbot.

## Language

**Panel**:
The overlay window the user sees. It opens and closes with a global hotkey, starts as a compact input, and grows to show the current Chat.
_Avoid_: Window, popup, UI

**Daemon**:
The background process that sits between the Panel and the Worker and owns the Chat list.
_Avoid_: Server, backend, service

**Worker**:
The process that runs the agent and streams its reply to the Daemon.
_Avoid_: Agent process, runner

**Chat**:
One whole conversation, however many questions it holds. The user can reopen it later and keep asking.
_Avoid_: Thread, session, conversation

**Message**:
One user question or one agent reply inside a Chat.
_Avoid_: Turn, entry

**Attachment**:
An image or file the user pastes, drops, or picks to send with a Message.
_Avoid_: Upload, file, media

**History**:
The list of all Chats, shown in the Panel so the user can open a past Chat.
_Avoid_: Archive, sidebar

**Transcript**:
The full, unabridged record of a Chat's Messages, as the user reads it.
_Avoid_: Log, chat log

**Memory**:
What the agent carries between questions in a Chat. It can be shorter than the Transcript.
_Avoid_: Context, state

**Model endpoint**:
The self-hosted, OpenAI-compatible service that supplies the language model.
_Avoid_: LLM server, API
