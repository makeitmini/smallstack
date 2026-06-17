#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

DRY_RUN=false
if [ "${1:-}" = "--dry-run" ]; then
  DRY_RUN=true
  shift
fi

CRATES=(
  "mini-err"
  "mini-log"
  "mini-search"
  "mini-serve"
  "mini-static"
  "mini-unified"
)

if [ $# -gt 0 ]; then
  CRATES=("$@")
fi

echo "→ Resolving workspace crate versions..."
CRATE_NAMES=()
CRATE_VERS=()
while IFS='|' read -r name ver; do
  CRATE_NAMES+=("$name")
  CRATE_VERS+=("$ver")
done < <(cargo metadata --format-version 1 --no-deps |
  jq -r '.packages[] | select(.source == null) | "\(.name)|\(.version)"')

for crate in "${CRATES[@]}"; do
  version=""
  for ((i = 0; i < ${#CRATE_NAMES[@]}; i++)); do
    if [ "${CRATE_NAMES[$i]}" = "$crate" ]; then
      version="${CRATE_VERS[$i]}"
      break
    fi
  done

  if [ -z "$version" ]; then
    echo "✗ Unknown crate: $crate"
    exit 1
  fi

  if cargo search --registry mini "$crate" 2>/dev/null |
     grep -q "^${crate} = \"${version}\""; then
    echo "→ $crate v$version already published, skipping"
    continue
  fi

  echo "→ Publishing $crate v$version to mini registry..."

  manifest="$SCRIPT_DIR/crates/$crate/Cargo.toml"
  backup="$manifest.publishbak"

  cp "$manifest" "$backup"

  section=""
  re_section='^\[[^]]*\]'
  while IFS= read -r line; do
    if [[ "$line" =~ $re_section ]]; then
      section="$line"
      echo "$line"
      continue
    fi
    case "$section" in
      "[dependencies]"|"[dev-dependencies]"|"[build-dependencies]")
        ;;
      *)
        echo "$line"
        continue
        ;;
    esac
    [[ -z "${line// }" ]] && { echo "$line"; continue; }
    [[ "$line" =~ ^[[:space:]]*# ]] && { echo "$line"; continue; }

    matched=false
    for ((idx = 0; idx < ${#CRATE_NAMES[@]}; idx++)); do
      dep="${CRATE_NAMES[$idx]}"
      ver="${CRATE_VERS[$idx]}"

      re_dep='^[[:space:]]*'"$dep"'[[:space:]]*=[[:space:]]*\{.*path[[:space:]]*=[[:space:]]*"\.\./'"$dep"'"'
      if [[ "$line" =~ $re_dep ]]; then
        line=$(echo "$line" | sed -E \
          -e 's/ path = "\.\.\/'"$dep"'",?//' \
          -e 's/,?[[:space:]]*version = "[^"]*"//' \
          -e 's/\{[[:space:]]*,[[:space:]]*/{ /')
        if [[ "$line" =~ \{[[:space:]]*\} ]]; then
          line=$(echo "$line" | sed -E 's/\{[[:space:]]*\}/\{ version = "'"$ver"'", registry = "mini" \}/')
        else
          line="${line%\}}"
          line="$line, version = \"$ver\", registry = \"mini\"}"
        fi
        matched=true
        break
      fi
    done
    echo "$line"
  done < "$backup" > "$manifest"

  if [ "$DRY_RUN" = true ]; then
    cargo publish --dry-run --no-verify --allow-dirty --registry mini -p "$crate" || true
  else
    cargo publish --no-verify --allow-dirty --registry mini -p "$crate"
  fi

  mv "$backup" "$manifest"
  sleep 2
  echo
done

echo "Done!"
