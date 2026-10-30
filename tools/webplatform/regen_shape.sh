#!/bin/zsh
# regenerate shape_fix.js against an engine capture taken WITHOUT shape_fix applied, then rebuild with it
cd /Users/sophiehe/Documents/aginx/aginxbrowser
S=${SHAPE_SCRATCH:-/tmp/shape-regen}; mkdir -p $S
restart() { lsof -nP -iTCP:18199 -sTCP:LISTEN -t | xargs kill 2>/dev/null; sleep 1; (DITING_BIND=127.0.0.1:18199 ./target/debug/diting --allow-private-network >/dev/null 2>&1 &); for i in 1 2 3 4 5 6 7 8; do curl -s http://127.0.0.1:18199/health >/dev/null && break; sleep 1; done; }
sed -i '' 's|^    ("shape_fix.js"|    //("shape_fix.js"|' crates/diting/build.rs
cargo build --bin diting 2>&1 | grep -E "^error|Uncaught|panicked"
restart
python3 tools/webplatform/capture/probe_run.py engine tools/webplatform/chrome/shape-audit-probe.js $S/shape-nofix-x.json >/dev/null
python3 tools/webplatform/gen/gen_shape_fix.py $S/shape-nofix-x.json
sed -i '' 's|^    //("shape_fix.js"|    ("shape_fix.js"|' crates/diting/build.rs
cargo build --bin diting 2>&1 | grep -E "^error|Finished|Uncaught|panicked"
restart
python3 tools/webplatform/capture/probe_run.py engine tools/webplatform/chrome/shape-audit-probe.js $S/shape-fixed.json >/dev/null
python3 tools/webplatform/capture/probe_run.py engine tools/webplatform/chrome/instance-audit-probe.js $S/inst-e.json >/dev/null
