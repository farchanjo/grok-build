# Contributing

This repository does **not** accept external pull requests or unsolicited
patches.

SpaceXAI develops this software internally. The public tree is published for
source transparency and local builds under the terms of the Apache License,
Version 2.0 (see [`LICENSE`](LICENSE)).

## Security reports

Please report security issues through the process described in
[`SECURITY.md`](SECURITY.md). Do not open a public issue for vulnerabilities.

## Licensing of this source

By downloading or using this source, you agree that your use is governed by
the Apache License, Version 2.0. No contributor license agreement is offered
because external contributions are not accepted.

## Workbench terminal backend (downstream)

Selectable external ACP backend for Workbench orchestration is documented in
[`docs/workbench-backend.md`](docs/workbench-backend.md). Default GrokShell
behavior is unchanged unless `WORKBENCH_TERMINAL_BACKEND=1` (or
`GROK_AGENT_BACKEND=workbench`) and an absolute workbench executable path are
set.
