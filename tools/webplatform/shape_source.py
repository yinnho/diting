"""Which whole-surface Chrome shape audit is which.

chrome-target.toml says the product is Chrome 157.0.8103.38.
ACCEPTANCE is the capture we measure against (local Chrome 157.0.8103.52,
962 interfaces). shape_audit.py, gen_shape_fix.py, gen_sym_fix.py,
gen_lift_own.py, gen_svg_filters.py and the prototype-parent repair in
gen_globals_surface.py read it.
BUILT_FROM is still the 145 capture. stub_members reads it: a full
regeneration drops constants on interfaces that already have some real
members. See docs/项目整理-20261115.md.
See docs/项目整理-20261115.md.
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
