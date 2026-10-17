# Nook glossary

## Terms

**Panel**

The Tauri desktop overlay that displays chats, settings, shortcuts, attachments,
and approval cards.

**Daemon**

The Rust process that owns local IPC, chat transcripts, configuration, database
migrations, Worker supervision, and tool authorization.

**Worker**

The Python process that runs the agent, persists checkpoint memory, invokes the
model endpoint, and exposes registered tools.

**Chat**

A saved conversation identified by a UUID.

**Message**

A user request or an assistant reply in a Chat.

**Attachment**

An image, text file, or PDF sent with a Message.

**History**

The Panel view that lists saved Chats.

**Transcript**

The stored Messages for a Chat.

**Memory**

The Worker's checkpoint data for a Chat.

**Model endpoint**

The OpenAI-compatible service used by the Worker.

**Focused shortcut**

A key combination that the Panel handles only while it has focus.

**Tool registry**

The Worker catalog of tools that the agent can request.

**Approval grant**

A scoped record of a user decision that permits a tool request.

**Approval tier**

The policy metadata that determines whether Nook permits, asks about, or denies
a tool request.
