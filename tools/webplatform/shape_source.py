"""Which whole-surface Chrome shape audit is which.

chrome-target.toml says the product is Chrome 157.0.8103.38.
ACCEPTANCE is the capture we measure against (local Chrome 157.0.8103.52,
962 interfaces). shape_audit.py, gen_shape_fix.py, gen_sym_fix.py,
gen_lift_own.py, gen_svg_filters.py and the prototype-parent repair in
gen_globals_surface.py read it.
BUILT_FROM is the same 157 audit. stub_members reads it. A regeneration
must keep constants on interfaces that already have real members: mark
those interfaces non-stub in the engine capture so the generator skips
them. Font pixel tables stay the 145 measurement files; a 157 spot check
of four families matched, and fonts-faces is already at the size cap.
"""
import os

HERE = os.path.dirname(os.path.abspath(__file__))
CHROME = os.path.join(HERE, "chrome")

ACCEPTANCE_NAME = "shape-audit-truth-157.json"
BUILT_FROM_NAME = "shape-audit-truth-157.json"


def acceptance_path():
    return os.path.join(CHROME, ACCEPTANCE_NAME)


def built_from_path():
    return os.path.join(CHROME, BUILT_FROM_NAME)
