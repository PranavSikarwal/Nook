# Handle rapid sends without stranding replies

Type: task
Status: resolved

## Observation

In the browser preview, submitting a second prompt while the first was active
left the first assistant bubble at "Thinking..." and left the second prompt in
the input. Neither response completed. This reproduced after a page reload.

A related run submitted another URL while a web-fetch approval was pending. The
preview showed two approval cards for the same host.

## Impact

The user may lose a reply, leave a draft unsent, or see multiple approval cards.
The browser mock may contribute to this behavior, so the real app needs separate
verification.

## Verification

Run the same actions through the native Tauri app, real Daemon, and Worker. Test
a second plain prompt during streaming and a second URL while approval is pending.
Capture message IDs, request IDs, terminal events, approval card count, and input
state.

## Native verification

The first native live run showed that the Panel accepted a prompt while `nookd`
was still starting. Issue 10 now has a native test for that startup window.

The isolated native live test drafted a second URL while a real web-search
request waited for approval. The input retained the URL while Stop replaced the
send control, and the transcript still contained only the first user message.
After the first response completed, the test submitted a separate plain prompt
and verified its completed assistant response. This confirms the UI does not
submit a second URL while a request is active. It does not test concurrent
Daemon requests because the Panel exposes no Send control during streaming.

## Done when

Every accepted request reaches one terminal state and renders one matching
response. If Nook disallows concurrent sends, it keeps the second prompt clearly
unsent and does not create a duplicate Worker request or approval.
