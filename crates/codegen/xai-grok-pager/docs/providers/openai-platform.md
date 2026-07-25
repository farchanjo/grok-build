# OpenAI Platform Client

Grok Build ships a typed client and CLI covering every endpoint in the pinned
OpenAI application and administration baseline inventories.

```bash
grok openai --provider openai ops --json
grok openai --provider openai models list
grok openai --provider openai chat create --input request.json
grok openai --provider openai admin projects list --yes   # mutating ops require --yes
```

- Application and administration credentials are structurally isolated.
- Cross-origin redirects never forward Authorization.
- Complex `--input` bodies deserialize into the operation's typed request.
- Multipart operations preserve typed text fields and accept repeatable
  `--file field=/path/to/file` bindings.
- `OpenAiClient::connect_realtime` opens a bounded, cancellable WebSocket
  session with typed client/server event discriminators and additive unknown
  event support.
- Realtime call setup remains a separate typed HTTP operation: it sends the
  multipart SDP/session payload and returns the bounded SDP answer as text.

See also `xai-grok-inference/baselines/operation_bindings_report.json` for the
auditable operation inventory.
