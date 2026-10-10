# Replace nested buttons in chat history rows

Type: task
Status: resolved

## Observation

The browser preview's History drawer renders each chat row as a button that
contains a delete button. React logged a warning that a button cannot be nested
inside another button and that hydration may fail. The DOM contained two
instances for the two saved chats.

## Impact

Nested interactive controls have invalid HTML semantics. Selecting a row and
deleting a Chat may trigger inconsistent click behavior or accessibility issues.

## Verification

Inspect the History drawer in the native Tauri WebView. Check the rendered
accessibility tree and console output. Select a row and activate its delete
control independently.

## Native verification

The native WebdriverIO test opened History after the isolated live query, found no
nested buttons, selected the saved test Chat, reopened History, and deleted that
Chat through its separate delete control. The test passed against the real Tauri
WebView and Daemon. The isolated run cleaned up its owned database after success.

## Done when

Chat selection and deletion use separate, valid interactive elements. The
native regression test activates each control and reports no nested-button
React warning.
