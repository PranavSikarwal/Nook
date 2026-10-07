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
  context.grant_scope == "once"
};
