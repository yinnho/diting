#!/usr/bin/env python3
"""Verify a diting-webplatform-truth checkout matches the source repo pin."""
import hashlib
import json
import subprocess
import sys
from pathlib import Path

root = Path(sys.argv[1] if len(sys.argv) > 1 else 'truth')
pin = json.loads(Path('tools/webplatform/font-truth-source.json').read_text())
got = subprocess.check_output(['git', '-C', str(root), 'rev-parse', 'HEAD'], text=True).strip()
assert got == pin['commit'], (got, pin['commit'])
mh = hashlib.sha256((root / pin['manifest']).read_bytes()).hexdigest()
assert mh == pin['manifest_sha256'], (mh, pin['manifest_sha256'])
print('truth pin ok', got)
