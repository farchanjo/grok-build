#!/usr/bin/env python3
"""Report registry settings that would hit the panic! fallback in
defaults_match_ui_config_default (i.e. entries with neither an explicit arm
nor a shell `CONTROLS` spec).

Run from the repository root: python3 script-test/find_missing_arms.py
Exit code 1 when anything is missing, so it can gate a shell chain."""
import re
import sys

DEFS = "crates/codegen/xai-grok-pager/src/settings/defs.rs"
REG = "crates/codegen/xai-grok-pager/src/settings/registry.rs"
CONTROL = "crates/codegen/xai-grok-shell/src/session/control.rs"

defs = open(DEFS, encoding="utf-8").read()
reg = open(REG, encoding="utf-8").read()
control = open(CONTROL, encoding="utf-8").read()

entries = []
for m in re.finditer(r"SettingMeta \{", defs):
    i = m.end()
    depth = 1
    while depth and i < len(defs):
        c = defs[i]
        if c == "{":
            depth += 1
        elif c == "}":
            depth -= 1
        i += 1
    body = defs[m.end():i]
    k = re.search(r'key: "([^"]+)"', body)
    o = re.search(r"owner: SettingOwner::(\w+)", body)
    kind = re.search(r"kind: SettingKind::(\w+)", body)
    if k and o and kind:
        entries.append((k.group(1), o.group(1), kind.group(1)))

# Shell control rows are reached through the generic arm, which resolves the
# spec at run time instead of naming the path.
control_paths = set(re.findall(r'^\s+(?:bool|percent|int|choice)_row\(\s*\n?\s*"([^"]+)"', control, re.M))
control_paths |= set(re.findall(r'^\s*"([a-z_]+(?:\.[a-z_]+)+)",\s*$', control, re.M))

start = reg.index("fn defaults_match_ui_config_default")
end = reg.index("fn defaults_match_pager_state")
arms = set(re.findall(r'\("([^"]+)", SettingKind::(\w+)', reg[start:end]))

missing = [
    e for e in entries
    if e[1] != "Pager"
    and e[2] != "Group"
    and (e[0], e[2]) not in arms
    and e[0] not in control_paths
]

print(f"{len(entries)} registry entries, {len(arms)} explicit arms, "
      f"{len(control_paths)} control specs, {len(missing)} uncovered")
for key, owner, kind in missing:
    print(f"  {owner:8} {kind:14} {key}")
sys.exit(1 if missing else 0)