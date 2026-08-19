#!/usr/bin/env bash
# Print the polkadot-sdk version-bump table for encointer-pallets by reading the
# crates.io sparse index, anchored on the `polkadot-sdk` umbrella crate.
#
# Why not read the polkadot-sdk git tree? The version numbers checked into the
# repo at a release tag are rewritten at publish time and do NOT match crates.io
# (at polkadot-stable2606-1 sp-arithmetic reads 23.0.0 in-tree but is published
# as 28.0.1). The index is the only authority.
#
# Why the umbrella crate? `polkadot-sdk` vYYMM.P.0 depends on ~390 SDK crates
# with exact requirements, so every version is read off a dependency edge with
# no guessing. It also settles which cohort actually exists on crates.io: SDK
# patch releases (the `-1` in polkadot-stable2606-1) only republish the crates
# they changed, and sometimes publish nothing at all.
#
# Unlike derive-sdk-versions.sh this needs neither `cargo psvm` nor a runtimes
# checkout that has already moved to the target release.
#
# Usage:
#   derive-sdk-versions-from-index.sh [<umbrella-version>]   # e.g. 2606.0.0
# Default: the newest published umbrella version.

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../../../.." && pwd)"
[ -f "$ROOT/Cargo.toml" ] || { echo "no workspace Cargo.toml at $ROOT" >&2; exit 1; }

ENCOINTER_TOML="$ROOT/Cargo.toml" UMBRELLA_VERSION="${1:-}" python3 <<'PY'
import json, os, re, subprocess

UMBRELLA = "polkadot-sdk"


def fetch(name):
    if len(name) <= 2:
        path = f"{len(name)}/{name}"
    elif len(name) == 3:
        path = f"3/{name[0]}/{name}"
    else:
        path = f"{name[:2]}/{name[2:4]}/{name}"
    raw = subprocess.run(
        ["curl", "-sf", f"https://index.crates.io/{path}"], capture_output=True, text=True
    ).stdout
    return [json.loads(l) for l in raw.splitlines() if l.strip()]


def parse_workspace_deps(path):
    """{crate: version} for [workspace.dependencies] entries carrying a version."""
    out, in_section = {}, False
    with open(path) as fh:
        for line in fh:
            s = line.strip()
            if s.startswith("[workspace.dependencies]"):
                in_section = True
                continue
            if s.startswith("[") and s.endswith("]"):
                in_section = False
                continue
            if not in_section or not s or s.startswith("#"):
                continue
            m = re.match(r"^([A-Za-z0-9_-]+)\s*=\s*(.+)$", s)
            if not m:
                continue
            name, rhs = m.group(1), m.group(2)
            mb = re.match(r'^"([^"]+)"\s*$', rhs)
            if mb:
                out[name] = mb.group(1)
                continue
            mv = re.search(r'version\s*=\s*"([^"]+)"', rhs)
            if mv:
                out[name] = mv.group(1)
    return out


def is_sdk_crate(name):
    if name.startswith(("encointer-", "pallet-encointer-")) or name == "ep-core":
        return False
    return name.startswith(
        ("frame-", "pallet-", "sp-", "sc-", "cumulus-", "polkadot-", "xcm", "staging-xcm", "substrate-")
    )


releases = fetch(UMBRELLA)
if not releases:
    raise SystemExit(f"could not read {UMBRELLA} from the crates.io index")

wanted = os.environ.get("UMBRELLA_VERSION") or ""
if wanted:
    match = [v for v in releases if v["vers"] == wanted]
    if not match:
        print(f"{UMBRELLA} {wanted} is not on crates.io. Recently published cohorts:")
        for v in releases[-8:]:
            print(f"  {v['vers']:<12} {v.get('pubtime', '')[:10]}")
        print()
        print("An SDK patch release (e.g. stable2606-1) has no cohort of its own unless")
        print("it appears here -- in that case target the base release and confirm with")
        print("`git diff <base-tag>..<patch-tag>` that nothing encointer depends on moved.")
        raise SystemExit(1)
    release = match[0]
else:
    release = releases[-1]

deps = {d["name"]: d["req"].lstrip("^~=") for d in release["deps"] if d["kind"] == "normal"}
print(f"cohort: {UMBRELLA} {release['vers']} published {release.get('pubtime', '')[:10]} ({len(deps)} crates)")
print()

enc = parse_workspace_deps(os.environ["ENCOINTER_TOML"])
sdk = sorted(k for k in enc if is_sdk_crate(k))

rows, missing = [], []
for name in sdk:
    if name in deps:
        rows.append((name, enc[name], deps[name]))
    else:
        missing.append((name, enc[name]))

print(f"{'CRATE':<34} {'CURRENT':<11} {'TARGET':<11} CHANGE")
print(f"{'-----':<34} {'-------':<11} {'------':<11} ------")
for name, current, target in rows:
    print(f"{name:<34} {current:<11} {target:<11} {'==' if current == target else '->'}")

if missing:
    print()
    print("Not carried by the umbrella crate -- check these by hand on crates.io:")
    for name, current in missing:
        print(f"  {name:<34} {current}")
PY
