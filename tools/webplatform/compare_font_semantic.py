#!/usr/bin/env python3
"""Compare a regenerated font-metrics index's semantic digest with the checked-in artifact."""
import json
import sys

got = json.load(open(sys.argv[1]))
want = json.load(open('crates/diting_js/js/font_metrics_index.json'))
assert got['semantic_sha256'] == want['semantic_sha256'], (got['semantic_sha256'], want['semantic_sha256'])
print('font semantic digest ok', want['semantic_sha256'])
