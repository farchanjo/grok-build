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

# Local deploy stays host-arch only (native, no cross build). The Intel slice is
# published by the GitHub release workflow; build it locally only on demand with
# `make build-x64`, or deploy a combined binary with `make deploy UNIVERSAL=1`.
MACOS_X64_TARGET ?= x86_64-apple-darwin
UNIVERSAL ?= 0

# Local builds are tuned for this workstation's CPU. `apple-m4` matches the M4
# host; the binary then needs M4 or newer. Override per Apple Silicon generation
# (`apple-a17` = M1/M2/M3, `native`, or empty to build the generic arm64 slice):
#   make deploy MACOS_TARGET_CPU=apple-a17
#
# RUSTFLAGS REPLACES the per-target list in .cargo/config.toml, so the arm64 base
# flags are repeated verbatim before the tuning flag. Build scripts and proc
# macros also see RUSTFLAGS (no `--target` locally), which is safe here because
# they execute on this same M4.
MACOS_TARGET_CPU ?= apple-m4
MACOS_ARM_BASE_FLAGS := -C link-arg=-undefined -C link-arg=dynamic_lookup -C force-unwind-tables=yes -C link-args=-ObjC
LOCAL_TUNING_FLAGS := $(if $(filter Darwin,$(shell uname -s)),$(if $(filter arm64,$(shell uname -m)),$(MACOS_ARM_BASE_FLAGS) $(if $(MACOS_TARGET_CPU),-C target-cpu=$(MACOS_TARGET_CPU),),),)
LIPO ?= /usr/bin/lipo
ARCH ?= /usr/bin/arch
X64_ARTIFACT := $(abspath $(CARGO_TARGET_DIR))/$(MACOS_X64_TARGET)/$(PROFILE)/$(BINARY_NAME)
UNIVERSAL_ARTIFACT := $(abspath $(CARGO_TARGET_DIR))/$(PROFILE)/$(BINARY_NAME)-universal
DEPLOY_SOURCE := $(if $(filter 1,$(UNIVERSAL)),$(UNIVERSAL_ARTIFACT),$(ARTIFACT))

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

.PHONY: build build-x64 build-universal deploy deploy-binary deploy-wrapper verify help

build:
	@set -eu; \
	if [ -n "$(LOCAL_TUNING_FLAGS)" ]; then export RUSTFLAGS="$(LOCAL_TUNING_FLAGS)"; echo "RUSTFLAGS=$$RUSTFLAGS"; fi; \
	$(CARGO) build --locked --jobs $(CARGO_BUILD_JOBS) --timings -p $(PACKAGE) --bin $(BINARY_NAME) --profile $(PROFILE) $(FEATURE_ARGS)

# Intel slice for older Macs. Cross-compiled from Apple Silicon; the macOS SDK
# and the `[target.x86_64-apple-darwin]` rustflags in .cargo/config.toml already
# support it.
build-x64:
	$(CARGO) build --locked --jobs $(CARGO_BUILD_JOBS) -p $(PACKAGE) --bin $(BINARY_NAME) --profile $(PROFILE) --target $(MACOS_X64_TARGET) $(FEATURE_ARGS)

# One artifact that runs on both Apple Silicon and Intel.
build-universal: build build-x64
	@set -eu; \
	$(LIPO) -create "$(ARTIFACT)" "$(X64_ARTIFACT)" -output "$(UNIVERSAL_ARTIFACT)"; \
	arches="$$($(LIPO) -archs "$(UNIVERSAL_ARTIFACT)")"; \
	case "$$arches" in *arm64*) ;; *) echo "error: universal artifact is missing the arm64 slice: $$arches" >&2; exit 1;; esac; \
	case "$$arches" in *x86_64*) ;; *) echo "error: universal artifact is missing the x86_64 slice: $$arches" >&2; exit 1;; esac; \
	echo "Universal artifact: $(UNIVERSAL_ARTIFACT) [$$arches]"

deploy: deploy-binary
	+$(MAKE) deploy-wrapper

ifeq ($(UNIVERSAL),1)
deploy-binary: build-universal
else
deploy-binary: build
endif
deploy-binary:
	@set -eu; \
	if [ "$$(uname -s)" != "Darwin" ]; then \
		echo "error: signed deployment is supported only on macOS" >&2; \
		exit 1; \
	fi; \
	if [ ! -x "$(DEPLOY_SOURCE)" ]; then \
		echo "error: release artifact not found: $(DEPLOY_SOURCE) (run 'make build-universal' or 'make build')" >&2; \
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
	/bin/cp "$(DEPLOY_SOURCE)" "$$staged"; \
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
	signed_arches="$$($(LIPO) -archs "$$staged")"; \
	if [ "$(UNIVERSAL)" = "1" ]; then \
	case "$$signed_arches" in *arm64*) ;; *) echo "error: signed artifact lost the arm64 slice: $$signed_arches" >&2; exit 1;; esac; \
	case "$$signed_arches" in *x86_64*) ;; *) echo "error: signed artifact lost the x86_64 slice: $$signed_arches" >&2; exit 1;; esac; \
	fi; \
	"$$staged" --version; \
	if [ "$(UNIVERSAL)" = "1" ]; then \
	if $(ARCH) -x86_64 /usr/bin/true 2>/dev/null; then \
	x64_version="$$($(ARCH) -x86_64 "$$staged" --version)"; \
	native_version="$$("$$staged" --version)"; \
	if [ "$$x64_version" != "$$native_version" ]; then \
	echo "error: x86_64 slice reports '$$x64_version' but the native slice reports '$$native_version'" >&2; \
	exit 1; \
	fi; \
	echo "x86_64 slice OK: $$x64_version"; \
	else \
	echo "note: Rosetta unavailable, x86_64 slice not executed"; \
	fi; \
	fi; \
	echo "Deployed slices: $$signed_arches"; \
	$(SUDO) $(INSTALL) -d -m 0755 -o root -g wheel "$(DEPLOY_DIR)"; \
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
	if [ "$(UNIVERSAL)" = "1" ]; then \
	deployed_arches="$$($(LIPO) -archs "$(DEPLOY_BINARY)")"; \
	case "$$deployed_arches" in *arm64*) ;; *) echo "error: deployed binary is missing the arm64 slice: $$deployed_arches" >&2; exit 1;; esac; \
	case "$$deployed_arches" in *x86_64*) ;; *) echo "error: deployed binary is missing the x86_64 slice (older Macs cannot run it): $$deployed_arches" >&2; exit 1;; esac; \
	fi; \
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
	grok_home_pin="$$(/usr/bin/sed -n 's|^export GROK_HOME="\$${GROK_HOME:-$${HOME}/\([^}]*\)}"|\1|p' "$(DEPLOY_WRAPPER)")"; \
	if [ "$$(/usr/bin/grep -Fc 'export GROK_HOME=' "$(DEPLOY_WRAPPER)")" -ne 1 ] || [ -z "$$grok_home_pin" ]; then \
	echo "error: wrapper must default GROK_HOME exactly once to a HOME-relative path" >&2; \
	exit 1; \
	fi; \
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
	expected_home="$${HOME}/$$grok_home_pin"; \
	test -d "$$expected_home"; \
	home_meta="$$("/usr/bin/stat" -f '%Lp' "$$expected_home")"; \
	if [ "$$home_meta" != "700" ]; then \
		echo "error: unexpected GROK_HOME mode: $$home_meta" >&2; \
		exit 1; \
	fi; \
	inspect_json="$$(mktemp "$${TMPDIR:-/tmp}/grok-custom-inspect.XXXXXX")"; \
	trap '/bin/unlink "$$inspect_json"' EXIT HUP INT TERM; \
	/usr/bin/env -u GROK_HOME -u GROK_LEADER_SOCKET "$(DEPLOY_WRAPPER)" inspect --json >"$$inspect_json"; \
	$(PYTHON3) -c 'import json, pathlib, sys; data = json.load(open(sys.argv[1], encoding="utf-8")); home = pathlib.Path(sys.argv[2]); surfaces = ("skills", "rules", "agents", "mcps", "hooks", "sessions"); expected = {(vendor, surface) for vendor in ("cursor", "claude") for surface in surfaces} | {("codex", "sessions")}; cells = data["externalCompat"]["cells"]; actual = {(cell["vendor"], cell["surface"]) for cell in cells}; actual == expected or sys.exit("unexpected runtime compatibility cells: " + repr(sorted(actual))); bad = [cell for cell in cells if cell.get("enabled") is not False or cell.get("source") != "env"]; not bad or sys.exit("compatibility cells not disabled by env: " + repr(bad)); user_roles = {"managed": "managed_config.toml", "user": "config.toml", "requirements": "requirements.toml"}; bad_paths = []; \
	[bad_paths.append(layer) for layer in data["configSources"]["layers"] if layer.get("role") in user_roles and (lambda p, role: (p.parent != home) if p.is_absolute() else (p.name != user_roles[role] or p.parent not in (pathlib.Path("."), pathlib.Path(""))))(pathlib.Path(layer["path"]), layer["role"])]; not bad_paths or sys.exit("user configuration escaped isolated GROK_HOME: " + repr(bad_paths)); legacy_socket = str(home.parent / ".grok" / "leader.sock"); legacy_socket not in json.dumps(data) or sys.exit("legacy leader socket leaked into inspect output")' "$$inspect_json" "$$expected_home"; \
	echo "$$wrapper_version"; \
	echo "GROK_HOME: $$expected_home ($$home_meta)"; \
	echo "Compatibility: 18 wrapper variables pinned; 13 runtime cells disabled by env"; \
	echo "Slices: $$($(LIPO) -archs "$(DEPLOY_BINARY)")"; \
	$(SHASUM) -a 256 "$(DEPLOY_BINARY)" "$(DEPLOY_WRAPPER)"

help:
	@echo "make                 Build the optimized release-dist artifact (host arch, tuned for $(MACOS_TARGET_CPU))"
	@echo "make FEATURES=name   Build with an explicit Cargo feature (for example claude-cli-runtime)"
	@echo "make deploy          Build, sign, and deploy the host-arch binary with the isolated wrapper"
	@echo "make deploy MACOS_TARGET_CPU=apple-a17   Tune for M1/M2/M3 instead of M4"
	@echo "make build-x64       Build the Intel slice ($(MACOS_X64_TARGET)) -- mirrors the Actions release asset"
	@echo "make build-universal Build host + Intel and lipo them into one universal artifact"
	@echo "make deploy UNIVERSAL=1  Deploy that universal artifact instead of the thin host one"
	@echo "make deploy-binary   Build, sign, back up, and deploy to $(DEPLOY_BINARY)"
	@echo "make deploy-wrapper  Back up and deploy the wrapper to $(DEPLOY_WRAPPER)"
	@echo "make verify          Verify the binary, wrapper, permissions, and isolation"
	@echo
	@echo "Signing defaults to Developer ID Application: Fabricio Fonseca (MYT54AW7PD)."
	@echo "For ad-hoc signing (local-only, no Gatekeeper):"
	@echo '  make deploy CODESIGN_IDENTITY="-"'
	@echo "For a different Developer ID:"
	@echo '  make deploy CODESIGN_IDENTITY="Developer ID Application: Name (TEAMID)"'
