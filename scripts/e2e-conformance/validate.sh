#!/usr/bin/env bash
set -euo pipefail
script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
cd -- "$script_dir/../.."
for variable in FERRITE_CHROMIUM_PATH FERRITE_FIREFOX_PATH; do
    executable=${!variable:-}
    if [[ -z "$executable" || ! -x "$executable" ]]; then
        echo "Required browser unavailable: set $variable to an executable path" >&2
        exit 1
    fi
    echo "$variable=$executable"
    "$executable" --version
done
export FERRITE_E2E_REQUIRE_BOTH_BROWSERS=1
cargo test -p ferrite-e2e --no-fail-fast -- --test-threads=4
