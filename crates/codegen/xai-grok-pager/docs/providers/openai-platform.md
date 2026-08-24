# OpenAI Platform Client

Grok Build ships a typed client and CLI covering every endpoint in the pinned
OpenAI application and administration baseline inventories.

Grok Build's OpenAI Platform client is pinned to OpenAI revision
`5c044be3bf3a42854e99e34616564eeb2124a317`. The local baseline contains **181
paths** and **287 operations**. The implementation requires the complete
pinned surface rather than an undocumented subset. Application and
administration credentials remain separate.

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

Supported transports are HTTP JSON, SSE, multipart, and binary request bodies;
the realtime WebSocket client is separate. Mutating operations require
confirmation (`--yes`), and `--dry-run` computes the operation without
performing it.

### Multiple OpenAI Platform accounts

Multiple Platform API-key instances coexist under distinct instance IDs. The
placeholder CLI examples below use `<INSTANCE_ID>` and `<MODEL_ID>`:

```bash
grok openai --provider openai-platform-primary models list
grok openai --provider openai-platform-secondary chat create --input request.json
grok -m openai-platform-primary:<MODEL_ID> -p "…"
```

Configured-instance credential resolution is scoped to that instance and
fails closed rather than borrowing a sibling account or an xAI session
credential. See [Multi-Account Providers](../user-guide/29-multi-account-providers.md)
and [Configuration](../user-guide/05-configuration.md).

See also `xai-grok-inference/baselines/operation_bindings_report.json` for the
auditable operation inventory.
