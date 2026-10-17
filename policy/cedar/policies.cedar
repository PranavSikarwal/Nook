// Web search is permitted under allow tier or when granted for chat, persistent, or once
permit (
  principal,
  action == Nook::Action::"web_search",
  resource
) when {
  context.approval_tier == "allow" ||
  context.grant_scope == "chat" ||
  context.grant_scope == "persistent" ||
  context.grant_scope == "once"
};

// Web fetch is permitted only with a matching scoped grant
permit (
  principal,
  action == Nook::Action::"web_fetch",
  resource
) when {
  context.grant_scope == "chat_host" ||
  context.grant_scope == "persistent_host" ||
  context.grant_scope == "once"
};

// Reading and listing need a grant for this call, this path, or its folder
permit (
  principal,
  action in [Nook::Action::"read_local_file", Nook::Action::"list_local_directory"],
  resource
) when {
  context.grant_scope == "once" ||
  context.grant_scope == "chat_path" ||
  context.grant_scope == "chat_dir"
};

// Writing needs a read-write folder and a grant for this call or this path
permit (
  principal,
  action == Nook::Action::"write_local_file",
  resource
) when {
  context.access == "read_write" &&
  (context.grant_scope == "once" || context.grant_scope == "chat_path")
};

// Credential files stay closed even inside an allowed folder
forbid (
  principal,
  action in [
    Nook::Action::"read_local_file",
    Nook::Action::"list_local_directory",
    Nook::Action::"write_local_file"
  ],
  resource
) when {
  context.path like "*/.ssh" || context.path like "*/.ssh/*" ||
  context.path like "*/.gnupg" || context.path like "*/.gnupg/*" ||
  context.path like "*/.aws" || context.path like "*/.aws/*" ||
  context.path like "*/.kube" || context.path like "*/.kube/*" ||
  context.path like "*/.env" || context.path like "*/.env.*"
};

// Git hooks and config can run code, so writes to .git stay closed
forbid (
  principal,
  action == Nook::Action::"write_local_file",
  resource
) when {
  context.path like "*/.git" || context.path like "*/.git/*"
};

// A command runs only with a grant for this exact call
permit (
  principal,
  action == Nook::Action::"run_command",
  resource
) when {
  context.grant_scope == "once"
};

// Programs that raise privileges or drive other apps stay closed
forbid (
  principal,
  action == Nook::Action::"run_command",
  resource
) when {
  ["sudo", "su", "doas", "pkexec", "osascript"].contains(context.executable_name)
};

// An MCP call needs a grant for this exact call or for this tool in this chat
permit (
  principal,
  action == Nook::Action::"mcp_call",
  resource
) when {
  context.grant_scope == "once" ||
  context.grant_scope == "chat" ||
  context.grant_scope == "persistent"
};
