# Browser automation design

Status: proposed. No code exists. Implementation starts only after approval.

## Why

Some pages need JavaScript, a click, or a form before they show their text.
`web_fetch` cannot read them. A browser tool lets the agent open a page, read
what a person sees, and act on it. A browser is also the largest attack surface
in this plan. It runs code from the web, holds cookies, and can send data.

## Scope

In scope: open a page, read its text, click, and type into a field, in a
sandboxed browser that the Daemon starts for one chat.

Not in scope: the user's own browser, the user's profile or cookies, saved
passwords, logging in, file upload, file download, extensions, and payments.

## Platforms

| Platform | Support | Reason |
| --- | --- | --- |
| macOS | Yes | The command sandbox exists (Seatbelt). |
| Linux | Yes | The command sandbox exists (bubblewrap). |
| Windows | No | No sandbox exists. The tools are not offered. |

## Design

1. **Engine.** The Daemon starts a headless Chromium through the command
   sandbox. It uses the Chrome DevTools protocol on a pipe, not a network port,
   so no other process can attach. Nook does not bundle the browser. The user
   sets `[tools.browser] executable` in `config.toml`. If the browser is missing,
   the tools are not offered and the Daemon logs why.
2. **Profile.** Each chat gets a new empty profile in a private temporary
   folder. The Daemon deletes it when the chat ends, when the chat is deleted,
   and when the Daemon exits. No cookie, cache, or storage survives. The
   sandbox gives the browser no access to the user's folders.
3. **Network.** The browser has no direct network. It talks only to a proxy that
   the Daemon runs on a socket in the profile folder. The proxy applies the same
   address rules as `web_fetch`: it resolves the name once, refuses private,
   loopback, link-local, and carrier-grade NAT addresses, and connects to the
   address it checked. It also refuses a host that the user did not approve for
   this chat.
4. **Permissions.** Camera, microphone, location, notifications, clipboard,
   popups, and downloads are denied. A JavaScript dialog is dismissed. A
   navigation to a new window is refused.
5. **Tools.** `browser_open(url)`, `browser_read()`, `browser_click(ref)`, and
   `browser_type(ref, text)`. `browser_read` returns a text snapshot of the page
   with a numbered reference for each link, button, and field. The model uses a
   reference, never a pixel position or a script.
6. **Cedar actions.** `browser_open` has the context `host` and `url`.
   `browser_click` has `host` and `target_kind`. `browser_type` has `host`,
   `field_type`, and `field_label`. Policy forbids `browser_type` when
   `field_type` is `password`, and forbids any call on a host that fails the
   address rules.

## Approval

- Opening a page asks once for each host in a chat, with the same actions as
  `web_fetch`: allow once, or allow the host for the chat. A permanent host grant
  is not offered, because a page can change.
- A navigation to another host, including a redirect and a link click, asks
  again for that host.
- A click on a button inside a form, and every `browser_type`, always ask. The
  prompt shows the host, the field label, and the exact text. No grant covers
  them.

## Credentials and cookies

Nook never fills in, reads, or stores a credential. Typing into a password field
is forbidden by policy. A site that needs a login is out of scope. A cookie set
by a page lives in the chat profile and dies with it. Two chats never share a
cookie.

## Prompt injection

Page text is untrusted. The snapshot is cut to a fixed size, shown as data, and
tagged as page content in the tool description. A page cannot cause a navigation
or an action by itself. Only a model call that passes Cedar and the user can.

## Failure behavior

| Case | Result |
| --- | --- |
| Browser missing or fails its self-test | Tools are not offered. The Daemon logs the reason. |
| Page load takes over 30 seconds | `Error: the page took too long`. The page stays open. |
| Browser crashes | `Error: the browser stopped`. The next call starts a new profile. |
| Address rule refuses a host | A fixed denial reason. No request leaves the proxy. |
| Chat is cancelled | The Daemon kills the browser group and deletes the profile. |

## Tests

- Proxy: private, loopback, link-local, and carrier-grade NAT addresses are
  refused. A name that resolves to a private address is refused. A name that
  changes its answer between two lookups cannot reach a private address.
- Isolation: a cookie from chat A is absent in chat B. The profile folder is
  gone after the chat ends. The browser cannot read a file outside its profile.
- Policy: typing into a password field is denied. A host that the user did not
  approve cannot load. A redirect to a new host asks again.
- Permissions: a download, a popup, and a camera request do nothing.
- Native flow: a local fixture site, on an address that only the test allows,
  covers open, read, click, type, deny, and cancel.
- Contracts: examples for each tool message and each denial reason.

## Open questions for the approval

1. Chromium through the DevTools pipe, or another engine?
2. Does the model receive screenshots? Many models do not accept images, so the
   first version returns text only.
3. Who installs the browser, and how does Nook check its version?
