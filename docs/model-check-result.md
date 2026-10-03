# Model check result

Output of running `check_model.py` against the real Model endpoint.

## Run output

```
PASS 1. raw tool call: get_weather({'city': 'Paris'})
PASS 2. streaming tool call: get_weather({'city': 'Paris'}) assembled from stream
PASS 3. deep agent: tool calls: ['write_todos', 'get_secret_code', 'write_todos']; todos: 2
PASS 4. image input: identified color: red
```

All four steps passed:
1. Raw tool calling via OpenAI client with `get_weather`.
2. Streaming tool calling with deltas assembled from the stream.
3. Deep agent execution with custom tool and `TodoListMiddleware`.
4. Image input via `image_url` on `/v1/chat/completions` identifying the color red.
