#!/usr/bin/env bash
# One-off manual publish for uzor 1.3.2.  CI was removed in 12d6136.
# Token comes from ../nemo/env/crates-io.env (path relative to nemo).
#
# Run from uzor repo root:
#   bash scripts/publish-1.3.2.sh

set -e

if [ -z "$CARGO_REGISTRY_TOKEN" ]; then
    if [ -f "../../env/crates-io.env" ]; then
        # uzor sits at nemo/uzor/, so ../../env points to nemo/env/.
        export CARGO_REGISTRY_TOKEN=$(grep CRATES_IO_TOKEN ../../env/crates-io.env | cut -d= -f2 | tr -d '"')
    elif [ -f "../env/crates-io.env" ]; then
        export CARGO_REGISTRY_TOKEN=$(grep CRATES_IO_TOKEN ../env/crates-io.env | cut -d= -f2 | tr -d '"')
    fi
fi

if [ -z "$CARGO_REGISTRY_TOKEN" ]; then
    echo "ERROR: CARGO_REGISTRY_TOKEN not set and no env/crates-io.env found"
    exit 1
fi

VERSION="1.3.2"

is_published() {
    local pkg="$1"
    local http_code
    http_code=$(curl -s -o /dev/null -w "%{http_code}" "https://crates.io/api/v1/crates/${pkg}/${VERSION}")
    [ "$http_code" = "200" ]
}

pub() {
    local pkg="$1"
    local wait_secs="${2:-30}"
    echo "=== ${pkg} ==="
    if is_published "$pkg"; then
        echo "  already on crates.io, skipping"
        return
    fi
    if cargo publish --package "$pkg"; then
        echo "  ok, waiting ${wait_secs}s for crates.io index..."
        sleep "$wait_secs"
    else
        echo "  FAILED — aborting"
        exit 1
    fi
}

# Tier 0 — leaf crates (no workspace deps)
pub uzor-fonts
pub uzor-icon
pub uzor-framework-macros

# Tier 1 — core (depends on uzor-fonts)
pub uzor 60

# Tier 2 — direct dependents of uzor
pub uzor-agent-api
pub uzor-render-canvas2d
pub uzor-render-tiny-skia
pub uzor-render-vello-cpu
pub uzor-render-vello-gpu
pub uzor-render-vello-hybrid
pub uzor-render-wgpu-instanced
pub uzor-window-desktop
pub uzor-window-mobile
pub uzor-window-web
pub uzor-tui

# Tier 3 — render-hub aggregates backends + window-*
pub uzor-render-hub 60

# Tier 4 — desktop bundle
pub uzor-desktop 60

# Tier 5 — examples (depends on the whole stack)
pub uzor-examples

echo
echo "All uzor 1.3.2 crates published."
