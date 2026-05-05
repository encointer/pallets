#!/usr/bin/env bash
# Print the polkadot-sdk version-bump table for encointer-pallets by reading the
# polkadot-fellows/runtimes workspace Cargo.toml. The runtimes workspace is always
# one PSDK release ahead, so its [workspace.dependencies] is the authoritative
# version source for unstable RCs (where `cargo psvm` doesn't help).
#
# Usage:
#   derive-sdk-versions.sh [<runtimes-path>]
# Default runtimes-path: ../runtimes (relative to encointer-pallets root).
#
# Output: a table of (crate, encointer-current, runtimes-current) for each SDK dep,
# plus a tail section for crates the runtimes manifest does NOT pin (apply a
# +1-minor heuristic to those).

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../../../.." && pwd)"
[ -f "$ROOT/Cargo.toml" ] || { echo "no workspace Cargo.toml at $ROOT" >&2; exit 1; }

RUNTIMES_PATH="${1:-$ROOT/../runtimes}"
[ -f "$RUNTIMES_PATH/Cargo.toml" ] || { echo "no Cargo.toml at $RUNTIMES_PATH" >&2; exit 1; }

ENCOINTER_TOML="$ROOT/Cargo.toml" RUNTIMES_TOML="$RUNTIMES_PATH/Cargo.toml" python3 <<'PY'
import os, re

def parse_workspace_deps(path):
    """Return {crate_name: version_string} for entries in [workspace.dependencies] that have a version=. Skip path-only deps."""
    out = {}
    in_section = False
    with open(path) as fh:
        for line in fh:
            stripped = line.strip()
            if stripped.startswith("[workspace.dependencies]"):
                in_section = True
                continue
            if stripped.startswith("[") and stripped.endswith("]"):
                in_section = False
                continue
            if not in_section or not stripped or stripped.startswith("#"):
                continue
            # match: crate-name = "X.Y.Z"  or  crate-name = { ..., version = "X.Y.Z", ... }
            m = re.match(r'^([A-Za-z0-9_-]+)\s*=\s*(.+)$', stripped)
            if not m:
                continue
            name, rhs = m.group(1), m.group(2)
            # bare-string form
            mb = re.match(r'^"([^"]+)"\s*$', rhs)
            if mb:
                out[name] = mb.group(1)
                continue
            # inline-table form: extract version="..."
            mv = re.search(r'version\s*=\s*"([^"]+)"', rhs)
            if mv:
                out[name] = mv.group(1)
    return out

def is_sdk_crate(name):
    # crates encointer-pallets pulls from the polkadot-sdk ecosystem
    if name.startswith("encointer-") or name.startswith("pallet-encointer-") or name == "ep-core":
        return False  # encointer-internal self-dep, not SDK
    prefixes = ("frame-", "pallet-", "sp-", "sc-", "cumulus-", "polkadot-", "xcm", "staging-xcm", "substrate-")
    return any(name.startswith(p) for p in prefixes)

enc = parse_workspace_deps(os.environ["ENCOINTER_TOML"])
run = parse_workspace_deps(os.environ["RUNTIMES_TOML"])

# Filter encointer entries to SDK-prefixed ones
sdk = {k: v for k, v in enc.items() if is_sdk_crate(k)}

# Encointer-internal (path) deps and patch deps appear in enc but we only want SDK ones; the filter handles that.

found, missing = [], []
for name in sorted(sdk):
    cur = sdk[name]
    if name in run:
        new = run[name]
        change = "==" if cur == new else "->"
        found.append((name, cur, new, change))
    else:
        missing.append((name, cur))

print(f"{'CRATE':<40} {'ENCOINTER':<12} {'RUNTIMES':<12} CHANGE")
print(f"{'-----':<40} {'---------':<12} {'--------':<12} ------")
for name, cur, new, change in found:
    print(f"{name:<40} {cur:<12} {new:<12} {change}")

if missing:
    print()
    print("Not pinned by /runtimes — apply +1-minor heuristic manually (verify on crates.io if cargo check fails):")
    print(f"{'CRATE':<40} {'ENCOINTER':<12}")
    print(f"{'-----':<40} {'---------':<12}")
    for name, cur in missing:
        print(f"{name:<40} {cur:<12}")
PY
