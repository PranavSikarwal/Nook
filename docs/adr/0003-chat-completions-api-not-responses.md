# Chat Completions API, not the Responses API

The Worker builds its model with `ChatOpenAI(..., use_responses_api=False)` and talks to `/v1/chat/completions` on the Model endpoint. The Worker needs tool calling and streaming, and `model-check/check_model.py` tests exactly that path. Deep Agents issue #3973 is the reason to avoid assuming the Responses API on a self-hosted server. We revisit this if the endpoint's Responses support is confirmed and a feature needs it.
