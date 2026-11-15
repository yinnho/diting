"""Which whole-surface Chrome shape audit is which.

chrome-target.toml says the product is Chrome 157.0.8103.38.
ACCEPTANCE is the capture we measure against (local Chrome 157.0.8103.52,
962 interfaces). BUILT_FROM is the capture the checked-in generated JS was
produced from. They stay different until shape_fix, stub_members and
sym_fix are regenerated from an engine capture taken with those scripts
disabled. See docs/项目整理-20261115.md.
"""
import os

HERE = os.path.dirname(os.path.abspath(__file__))
CHROME = os.path.join(HERE, "chrome")

ACCEPTANCE_NAME = "shape-audit-truth-157.json"
BUILT_FROM_NAME = "shape-audit-truth-145.json"


def acceptance_path():
    return os.path.join(CHROME, ACCEPTANCE_NAME)


def built_from_path():
    return os.path.join(CHROME, BUILT_FROM_NAME)
