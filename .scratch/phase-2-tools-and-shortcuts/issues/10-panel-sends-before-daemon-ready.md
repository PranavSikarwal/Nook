# Block sends until the Daemon is ready

Type: task
Status: resolved

## Observation

Native WebDriver runs showed the Panel accepting a prompt before `nookd` had
opened its Unix socket. The transcript then showed `Failed to connect to daemon:
No such file or directory (os error 2)`. The Panel's setup starts `nookd`
asynchronously, while `handleSend` immediately appends a user message and sends
an IPC command.

## Impact

A user who sends as the Panel starts can get a failed assistant message. They can
then retry while the Daemon is still starting, which can mix startup failures
with later approval and streaming state.

## Verification

Use the native Tauri WebDriver app with its real Daemon and Worker. Verify the
input stays disabled until the `ping_daemon` command succeeds. Submit one prompt
after readiness and confirm it creates one request and reaches one terminal
state.

## Native verification

The native smoke run launched `nookd` through a test-only wrapper that delayed
startup by eight seconds. The test confirmed that the input and send action were
disabled before Daemon readiness, then confirmed that the input became enabled
after `ping_daemon` succeeded. The isolated native test passed.

## Done when

The Panel does not accept or dispatch a prompt until the Daemon responds to ping.
The native test proves that startup does not create a failed assistant message,
and the live news flow reaches a terminal Worker response.
