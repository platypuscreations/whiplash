#!/bin/bash
# Regenerates THIRD-PARTY-NOTICES.md: the hand-written part in
# packaging/notices-header.md, followed by every Rust crate Whiplash uses
# (on any platform) with its license. Run it after changing dependencies.
set -euo pipefail

cd "$(dirname "$0")/.."
{
    cat packaging/notices-header.md
    echo
    echo "| Crate | Version | License |"
    echo "|---|---|---|"
    cargo tree --edges normal --target all --prefix none --format '{p}|{l}' \
        | sed -e 's/ (\*)$//' -e 's/ (proc-macro)//' -e 's/ ([^)]*)|/|/' \
        | grep -v '^whiplash ' \
        | sort -u \
        | sed -E 's/^([^ ]+) v([^|]+)\|[[:space:]]*(.*)$/| \1 | \2 | \3 |/'
} > THIRD-PARTY-NOTICES.md
echo "Wrote THIRD-PARTY-NOTICES.md"
