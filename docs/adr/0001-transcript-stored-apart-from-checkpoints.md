# Transcript stored apart from checkpoints

The Daemon stores every Message in its own Postgres tables as it streams. The Worker's LangGraph checkpoints hold the agent's Memory in a separate schema. We chose this because Memory gets summarized once a Chat nears 750k tokens, while the Transcript must stay complete for the user to read. The cost is that the same text lives in two places.
