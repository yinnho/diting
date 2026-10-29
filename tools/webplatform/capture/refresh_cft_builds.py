#!/usr/bin/env python3
"""Refresh chrome/cft-builds.json from Google's Chrome for Testing known-good list (network).
After refreshing: tools/webplatform/check_fresh.sh --update, review the diff, commit."""
import datetime, json, os, urllib.request
URL = "https://googlechromelabs.github.io/chrome-for-testing/known-good-versions.json"
out = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "chrome", "cft-builds.json")
d = json.load(urllib.request.urlopen(URL, timeout=60))
STABLE_URL = "https://googlechromelabs.github.io/chrome-for-testing/last-known-good-versions.json"
stable = json.load(urllib.request.urlopen(STABLE_URL, timeout=60))["channels"]["Stable"]["version"]
by = {}
for v in d["versions"]:
    parts = v["version"].split(".")
    by.setdefault(int(parts[0]), set()).add(".".join(parts[2:]))
key = lambda bp: tuple(int(x) for x in bp.split("."))
json.dump({"_meta": {"source": URL, "fetched": datetime.date.today().isoformat(), "stable": stable,
                     "note": "major -> every build.patch Google published as a Chrome for Testing known-good version. Used offline to prove that a claimed full version really exists."},
           "builds": {str(m): sorted(v, key=key) for m, v in sorted(by.items())}},
          open(out, "w"), indent=0, sort_keys=True)
print("refreshed:", sum(len(v) for v in by.values()), "versions,", len(by), "majors")
