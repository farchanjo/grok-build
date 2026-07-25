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

See also `xai-grok-inference/baselines/operation_bindings_report.json` for the
auditable operation inventory.
