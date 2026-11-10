# Clear pending approval when changing Chats

Type: task
Status: resolved

## Observation

The browser preview showed a web-fetch approval card, then retained that card
after Start new chat switched to an empty Chat. The card remained after a page
reload and a second test run.

## Impact

A card from one Chat may be mistaken for a request in another Chat. The preview
result is not conclusive for the real Daemon because the browser fallback uses
canned approval handling.

## Verification

Reproduce with the native Tauri app and real Daemon. Check the approval card,
Chat ID, message ID, and active request before and after switching Chats.

## Native verification

The isolated native test triggered a real web-fetch approval. While it was
pending, the test confirmed that Start new chat was disabled, the originating
user message remained visible, and the approval card stayed attached to its
assistant message. The Worker then completed after the test allowed the request.
The live native scenario passed.

## Done when

The native regression test proves that switching Chats either cancels the
pending request or keeps the approval attached only to its originating Chat.
The UI must never show the old approval as belonging to the new Chat.
