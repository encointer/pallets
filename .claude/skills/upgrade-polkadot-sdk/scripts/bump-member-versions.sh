#!/usr/bin/env bash
# Bump every encointer-pallets member crate's [package].version per a strategy,
# and update matching version strings in the root [workspace.dependencies] block.
#
# Usage:
#   bump-member-versions.sh [--strategy minor|patch|major] [--dry-run]
#
# Strategy semantics (input X.Y.Z):
#   minor (default): X.Y.Z -> X.(Y+1).0
#   patch:           X.Y.Z -> X.Y.(Z+1)
#   major:           X.Y.Z -> (X+1).0.0
#
# Safety: refuses to run if member crates don't all share the same major version.

set -euo pipefail

STRATEGY=minor
DRY_RUN=0
while [ $# -gt 0 ]; do
    case "$1" in
        --strategy) STRATEGY="$2"; shift 2 ;;
        --dry-run)  DRY_RUN=1; shift ;;
        -h|--help)  sed -n '2,15p' "$0"; exit 0 ;;
        *) echo "unknown arg: $1" >&2; exit 2 ;;
    esac
done

case "$STRATEGY" in minor|patch|major) ;; *) echo "bad --strategy: $STRATEGY" >&2; exit 2 ;; esac

# Run from the encointer-pallets repo root (where the workspace Cargo.toml lives).
ROOT="$(cd "$(dirname "$0")/../../../.." && pwd)"
[ -f "$ROOT/Cargo.toml" ] || { echo "no workspace Cargo.toml at $ROOT" >&2; exit 1; }
cd "$ROOT"

bump() {
    local v="$1" maj min pat
    IFS='.' read -r maj min pat <<<"$v"
    case "$STRATEGY" in
        minor) echo "$maj.$((min+1)).0" ;;
        patch) echo "$maj.$min.$((pat+1))" ;;
        major) echo "$((maj+1)).0.0" ;;
    esac
}

MAP_FILE=$(mktemp)
trap 'rm -f "$MAP_FILE"' EXIT

declare -a ROWS=()
MAJORS_SEEN=""

while IFS= read -r f; do
    grep -q '^\[package\]' "$f" || continue
    name=$(awk '/^\[package\]/{p=1;next} /^\[/{p=0} p && /^name *=/{gsub(/[" ]/,""); split($0,a,"="); print a[2]; exit}' "$f")
    old=$(awk '/^\[package\]/{p=1;next} /^\[/{p=0} p && /^version *=/{gsub(/[" ]/,""); split($0,a,"="); print a[2]; exit}' "$f")
    [ -z "$name" ] && continue
    [ -z "$old" ] && continue
    new=$(bump "$old")
    ROWS+=("$f|$name|$old|$new")
    printf '%s=%s\n' "$name" "$new" >> "$MAP_FILE"
    maj="${old%%.*}"
    case " $MAJORS_SEEN " in *" $maj "*) ;; *) MAJORS_SEEN="$MAJORS_SEEN $maj" ;; esac
done < <(find . -name Cargo.toml -not -path './target/*')

nmajors=$(printf '%s\n' $MAJORS_SEEN | grep -c .)
if [ "$nmajors" -ne 1 ]; then
    echo "REFUSING: member crates have inconsistent majors:$MAJORS_SEEN" >&2
    exit 3
fi

printf '%-50s %-10s %-10s\n' CRATE OLD NEW
printf '%-50s %-10s %-10s\n' "----" "---" "---"
for r in "${ROWS[@]}"; do
    IFS='|' read -r f name old new <<<"$r"
    printf '%-50s %-10s %-10s\n' "$name" "$old" "$new"
done

if [ "$DRY_RUN" -eq 1 ]; then
    echo "(dry-run; no files modified)"
    exit 0
fi

# 1) Bump [package].version in each member Cargo.toml.
for r in "${ROWS[@]}"; do
    IFS='|' read -r f name old new <<<"$r"
    awk -v old="$old" -v new="$new" '
        BEGIN{p=0; done=0}
        /^\[package\]/ {p=1; print; next}
        /^\[/ {p=0; print; next}
        p && !done && /^version *= *"/ { sub("\"" old "\"", "\"" new "\""); done=1 }
        {print}
    ' "$f" > "$f.tmp" && mv "$f.tmp" "$f"
done

# 2) Update root [workspace.dependencies] version strings keyed by crate name on the same line.
BUMP_MAP_FILE="$MAP_FILE" python3 - "$ROOT/Cargo.toml" <<'PY'
import sys, re, os
path = sys.argv[1]
mapping = {}
with open(os.environ["BUMP_MAP_FILE"]) as fh:
    for line in fh:
        line = line.strip()
        if not line:
            continue
        k, v = line.split("=", 1)
        mapping[k] = v

with open(path) as fh:
    src = fh.read()

def replace_line(line):
    m = re.match(r'^([A-Za-z0-9_-]+)\s*=\s*\{', line)
    if not m:
        return line
    name = m.group(1)
    if name not in mapping:
        return line
    return re.sub(r'version\s*=\s*"[^"]+"', f'version = "{mapping[name]}"', line, count=1)

new_src = "\n".join(replace_line(l) for l in src.split("\n"))
if new_src != src:
    with open(path, "w") as fh:
        fh.write(new_src)
    print(f"updated {path}")
else:
    print(f"no [workspace.dependencies] changes in {path}")
PY
