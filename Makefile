SHELL := /bin/sh
.DEFAULT_GOAL := build

CARGO ?= cargo
CARGO_TARGET_DIR ?= target
CARGO_INCREMENTAL ?= 0
CARGO_BUILD_JOBS ?= 16
RUSTC_WRAPPER ?= sccache
export CARGO_TARGET_DIR
export CARGO_INCREMENTAL
export CARGO_BUILD_JOBS
export RUSTC_WRAPPER

PACKAGE ?= xai-grok-pager-bin
PROFILE ?= release-dist
FEATURES ?=
FEATURE_ARGS := $(if $(strip $(FEATURES)),--features $(FEATURES),)
BINARY_NAME ?= xai-grok-pager
ARTIFACT := $(abspath $(CARGO_TARGET_DIR))/$(PROFILE)/$(BINARY_NAME)

DEPLOY_DIR ?= /opt/grok-custom
DEPLOY_BINARY ?= $(DEPLOY_DIR)/grok
WRAPPER_SOURCE ?= $(abspath grok-custom)
DEPLOY_WRAPPER ?= $(DEPLOY_DIR)/grok-custom

CODESIGN ?= /usr/bin/codesign
CODESIGN_IDENTITY ?= Developer ID Application: Fabricio Fonseca (MYT54AW7PD)
CODESIGN_IDENTIFIER ?= grok-custom

SUDO ?= sudo
INSTALL ?= /usr/bin/install
SHASUM ?= /usr/bin/shasum
BASH ?= /bin/bash
PYTHON3 ?= /usr/bin/python3

.PHONY: build deploy deploy-binary deploy-wrapper verify help

build:
	$(CARGO) build --locked --jobs $(CARGO_BUILD_JOBS) --timings -p $(PACKAGE) --bin $(BINARY_NAME) --profile $(PROFILE) $(FEATURE_ARGS)

deploy: deploy-binary
	+$(MAKE) deploy-wrapper

deploy-binary: build
	@set -eu; \
	if [ "$$(uname -s)" != "Darwin" ]; then \
		echo "error: signed deployment is supported only on macOS" >&2; \
		exit 1; \
	fi; \
	if [ ! -x "$(ARTIFACT)" ]; then \
		echo "error: release artifact not found: $(ARTIFACT)" >&2; \
		exit 1; \
	fi; \
	staged="$$(mktemp "$${TMPDIR:-/tmp}/grok-custom-deploy.XXXXXX")"; \
	deploy_tmp=""; \
	cleanup() { \
		if [ -n "$$deploy_tmp" ] && $(SUDO) test -e "$$deploy_tmp"; then \
			$(SUDO) /bin/unlink "$$deploy_tmp"; \
		fi; \
		if [ -e "$$staged" ]; then \
			/bin/unlink "$$staged"; \
		fi; \
	}; \
	trap cleanup EXIT HUP INT TERM; \
	/bin/cp "$(ARTIFACT)" "$$staged"; \
	/bin/chmod 0755 "$$staged"; \
	identity="$(CODESIGN_IDENTITY)"; \
	if [ "$$identity" = "-" ]; then \
		$(CODESIGN) --force --sign - \
			--identifier "$(CODESIGN_IDENTIFIER)" \
			--options runtime \
			--timestamp=none \
			"$$staged"; \
	else \
		$(CODESIGN) --force --sign "$$identity" \
			--identifier "$(CODESIGN_IDENTIFIER)" \
			--options runtime \
			--timestamp \
			"$$staged"; \
	fi; \
	$(CODESIGN) --verify --strict --verbose=2 "$$staged"; \
	"$$staged" --version; \
	$(SUDO) $(INSTALL) -d -m 0755 -o root -g wheel "$(DEPLOY_DIR)"; \
	if $(SUDO) test -e "$(DEPLOY_BINARY)"; then \
		backup="$(DEPLOY_BINARY).backup.$$(date -u +%Y%m%d-%H%M%S).$$$$"; \
		$(SUDO) /bin/cp -p "$(DEPLOY_BINARY)" "$$backup"; \
		echo "Backup created: $$backup"; \
	fi; \
	deploy_tmp="$(DEPLOY_BINARY).new.$$$$"; \
	$(SUDO) $(INSTALL) -m 0755 -o root -g wheel "$$staged" "$$deploy_tmp"; \
	$(SUDO) /bin/mv -f "$$deploy_tmp" "$(DEPLOY_BINARY)"; \
	deploy_tmp=""; \
	/usr/bin/cmp -s "$$staged" "$(DEPLOY_BINARY)"; \
	$(CODESIGN) --verify --strict --verbose=2 "$(DEPLOY_BINARY)"; \
	"$(DEPLOY_BINARY)" --version; \
	$(SHASUM) -a 256 "$(DEPLOY_BINARY)"

deploy-wrapper:
	@set -eu; \
	if [ "$$(uname -s)" != "Darwin" ]; then \
		echo "error: wrapper deployment is supported only on macOS" >&2; \
		exit 1; \
	fi; \
	if [ ! -f "$(WRAPPER_SOURCE)" ]; then \
		echo "error: wrapper source not found: $(WRAPPER_SOURCE)" >&2; \
		exit 1; \
	fi; \
	$(BASH) -n "$(WRAPPER_SOURCE)"; \
	staged="$$(mktemp "$${TMPDIR:-/tmp}/grok-custom-wrapper.XXXXXX")"; \
	deploy_tmp=""; \
	cleanup() { \
		if [ -n "$$deploy_tmp" ] && $(SUDO) test -e "$$deploy_tmp"; then \
			$(SUDO) /bin/unlink "$$deploy_tmp"; \
		fi; \
		if [ -e "$$staged" ]; then \
			/bin/unlink "$$staged"; \
		fi; \
	}; \
	trap cleanup EXIT HUP INT TERM; \
	$(INSTALL) -m 0755 "$(WRAPPER_SOURCE)" "$$staged"; \
	$(BASH) -n "$$staged"; \
	$(SUDO) $(INSTALL) -d -m 0755 -o root -g wheel "$(DEPLOY_DIR)"; \
	if $(SUDO) test -e "$(DEPLOY_WRAPPER)"; then \
		backup="$(DEPLOY_WRAPPER).backup.$$(date -u +%Y%m%d-%H%M%S).$$$$"; \
		$(SUDO) /bin/cp -p "$(DEPLOY_WRAPPER)" "$$backup"; \
		echo "Backup created: $$backup"; \
	fi; \
	deploy_tmp="$(DEPLOY_WRAPPER).new.$$$$"; \
	$(SUDO) $(INSTALL) -m 0755 -o root -g wheel "$$staged" "$$deploy_tmp"; \
	$(SUDO) /bin/mv -f "$$deploy_tmp" "$(DEPLOY_WRAPPER)"; \
	deploy_tmp=""; \
	/usr/bin/cmp -s "$$staged" "$(DEPLOY_WRAPPER)"; \
	$(BASH) -n "$(DEPLOY_WRAPPER)"; \
	installed_meta="$$("/usr/bin/stat" -f '%Su:%Sg:%Lp' "$(DEPLOY_WRAPPER)")"; \
	if [ "$$installed_meta" != "root:wheel:755" ]; then \
		echo "error: unexpected wrapper ownership or mode: $$installed_meta" >&2; \
		exit 1; \
	fi; \
	echo "Wrapper deployed: $(DEPLOY_WRAPPER) ($$installed_meta)"

verify:
	@set -eu; \
	if [ "$$(uname -s)" != "Darwin" ]; then \
		echo "error: deployed verification is supported only on macOS" >&2; \
		exit 1; \
	fi; \
	test -f "$(WRAPPER_SOURCE)"; \
	$(BASH) -n "$(WRAPPER_SOURCE)"; \
	test -x "$(DEPLOY_BINARY)"; \
	test -f "$(DEPLOY_WRAPPER)"; \
	test -x "$(DEPLOY_WRAPPER)"; \
	$(BASH) -n "$(DEPLOY_WRAPPER)"; \
	/usr/bin/cmp -s "$(WRAPPER_SOURCE)" "$(DEPLOY_WRAPPER)"; \
	installed_meta="$$("/usr/bin/stat" -f '%Su:%Sg:%Lp' "$(DEPLOY_WRAPPER)")"; \
	if [ "$$installed_meta" != "root:wheel:755" ]; then \
		echo "error: unexpected wrapper ownership or mode: $$installed_meta" >&2; \
		exit 1; \
	fi; \
	for vendor in CURSOR CLAUDE CODEX; do \
		for surface in SKILLS RULES AGENTS MCPS HOOKS SESSIONS; do \
			assignment="export GROK_$${vendor}_$${surface}_ENABLED=0"; \
			count="$$(/usr/bin/grep -Fxc "$$assignment" "$(DEPLOY_WRAPPER)" || true)"; \
			if [ "$$count" -ne 1 ]; then \
				echo "error: wrapper must pin exactly once: $$assignment" >&2; \
				exit 1; \
			fi; \
		done; \
	done; \
	/usr/bin/grep -Fqx 'export GROK_HOME="$${HOME}/.grok-prod"' "$(DEPLOY_WRAPPER)"; \
	/usr/bin/grep -Fqx 'export GROK_LEADER_SOCKET="$${GROK_HOME}/leader.sock"' "$(DEPLOY_WRAPPER)"; \
	/usr/bin/grep -Fqx 'export GROK_CLAUDE_CLI_RUNTIME=1' "$(DEPLOY_WRAPPER)"; \
	/usr/bin/grep -Fqx 'export GROK_EXTERNAL_OTEL=1' "$(DEPLOY_WRAPPER)"; \
	/usr/bin/grep -Fqx 'export OTEL_METRICS_EXPORTER=otlp' "$(DEPLOY_WRAPPER)"; \
	/usr/bin/grep -Fqx 'export OTEL_LOGS_EXPORTER=otlp' "$(DEPLOY_WRAPPER)"; \
	/usr/bin/grep -Fqx 'export OTEL_EXPORTER_OTLP_PROTOCOL=http/protobuf' "$(DEPLOY_WRAPPER)"; \
	/usr/bin/grep -Fqx 'export OTEL_EXPORTER_OTLP_ENDPOINT=http://vm.services:24318' "$(DEPLOY_WRAPPER)"; \
	/usr/bin/grep -Fqx 'export OTEL_LOG_USER_PROMPTS=0' "$(DEPLOY_WRAPPER)"; \
	/usr/bin/grep -Fqx 'export OTEL_LOG_TOOL_DETAILS=0' "$(DEPLOY_WRAPPER)"; \
	/usr/bin/grep -Fq 'unset OTEL_EXPORTER_OTLP_LOGS_ENDPOINT OTEL_EXPORTER_OTLP_METRICS_ENDPOINT' "$(DEPLOY_WRAPPER)"; \
	/usr/bin/grep -Fq 'unset OTEL_EXPORTER_OTLP_HEADERS OTEL_EXPORTER_OTLP_LOGS_HEADERS' "$(DEPLOY_WRAPPER)"; \
	/usr/bin/grep -Fq 'unset OTEL_EXPORTER_OTLP_METRICS_HEADERS' "$(DEPLOY_WRAPPER)"; \
	/usr/bin/grep -Fqx 'exec "$${GROK_BINARY}" "$$@"' "$(DEPLOY_WRAPPER)"; \
	$(CODESIGN) --verify --strict --verbose=2 "$(DEPLOY_BINARY)"; \
	$(CODESIGN) -dvv "$(DEPLOY_BINARY)" 2>&1; \
	binary_version="$$("$(DEPLOY_BINARY)" --version)"; \
	wrapper_version="$$("$(DEPLOY_WRAPPER)" --version)"; \
	binary_build="$$(printf '%s\n' "$$binary_version" | /usr/bin/awk '{ print $$1, $$2, $$3 }')"; \
	wrapper_build="$$(printf '%s\n' "$$wrapper_version" | /usr/bin/awk '{ print $$1, $$2, $$3 }')"; \
	if [ "$$wrapper_build" != "$$binary_build" ]; then \
		echo "error: wrapper build does not match deployed binary" >&2; \
		echo "binary:  $$binary_version" >&2; \
		echo "wrapper: $$wrapper_version" >&2; \
		exit 1; \
	fi; \
	expected_home="$${HOME}/.grok-prod"; \
	test -d "$$expected_home"; \
	home_meta="$$("/usr/bin/stat" -f '%Lp' "$$expected_home")"; \
	if [ "$$home_meta" != "700" ]; then \
		echo "error: unexpected GROK_HOME mode: $$home_meta" >&2; \
		exit 1; \
	fi; \
	inspect_json="$$(mktemp "$${TMPDIR:-/tmp}/grok-custom-inspect.XXXXXX")"; \
	trap '/bin/unlink "$$inspect_json"' EXIT HUP INT TERM; \
	"$(DEPLOY_WRAPPER)" inspect --json >"$$inspect_json"; \
	$(PYTHON3) -c 'import json, pathlib, sys; data = json.load(open(sys.argv[1], encoding="utf-8")); home = pathlib.Path(sys.argv[2]); surfaces = ("skills", "rules", "agents", "mcps", "hooks", "sessions"); expected = {(vendor, surface) for vendor in ("cursor", "claude") for surface in surfaces} | {("codex", "sessions")}; cells = data["externalCompat"]["cells"]; actual = {(cell["vendor"], cell["surface"]) for cell in cells}; actual == expected or sys.exit("unexpected runtime compatibility cells: " + repr(sorted(actual))); bad = [cell for cell in cells if cell.get("enabled") is not False or cell.get("source") != "env"]; not bad or sys.exit("compatibility cells not disabled by env: " + repr(bad)); user_roles = {"managed", "user", "requirements"}; bad_paths = [layer for layer in data["configSources"]["layers"] if layer.get("role") in user_roles and pathlib.Path(layer["path"]).parent != home]; not bad_paths or sys.exit("user configuration escaped isolated GROK_HOME: " + repr(bad_paths)); legacy_socket = str(home.parent / ".grok" / "leader.sock"); legacy_socket not in json.dumps(data) or sys.exit("legacy leader socket leaked into inspect output")' "$$inspect_json" "$$expected_home"; \
	echo "$$wrapper_version"; \
	echo "GROK_HOME: $$expected_home ($$home_meta)"; \
	echo "Compatibility: 18 wrapper variables pinned; 13 runtime cells disabled by env"; \
	$(SHASUM) -a 256 "$(DEPLOY_BINARY)" "$(DEPLOY_WRAPPER)"

help:
	@echo "make                 Build the optimized release-dist artifact"
	@echo "make FEATURES=name   Build with an explicit Cargo feature (for example claude-cli-runtime)"
	@echo "make deploy          Build/sign the binary and deploy it with the isolated wrapper"
	@echo "make deploy-binary   Build, sign, back up, and deploy to $(DEPLOY_BINARY)"
	@echo "make deploy-wrapper  Back up and deploy the wrapper to $(DEPLOY_WRAPPER)"
	@echo "make verify          Verify the binary, wrapper, permissions, and isolation"
	@echo
	@echo "Signing defaults to Developer ID Application: Fabricio Fonseca (MYT54AW7PD)."
	@echo "For ad-hoc signing (local-only, no Gatekeeper):"
	@echo '  make deploy CODESIGN_IDENTITY="-"'
	@echo "For a different Developer ID:"
	@echo '  make deploy CODESIGN_IDENTITY="Developer ID Application: Name (TEAMID)"'
