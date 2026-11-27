#!/usr/bin/env bash
# Regenerate the font-metrics artifact from the independently pinned truth
# repository. Normal source checkouts do not need the large truth corpus; this
# script is for deliberate model/generator work and the font-truth CI lane.
set -euo pipefail
cd "$(dirname "$0")/../.."
TRUTH_ROOT=${DITING_FONT_TRUTH_ROOT:-}
if [ -z "$TRUTH_ROOT" ]; then
  echo "set DITING_FONT_TRUTH_ROOT to a diting-webplatform-truth checkout" >&2
  exit 2
fi
python3 tools/webplatform/gen/gen_font_metrics.py --truth-root "$TRUTH_ROOT"
python3 tools/webplatform/check_font_artifact.py
