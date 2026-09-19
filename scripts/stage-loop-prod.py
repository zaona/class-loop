#!/usr/bin/env python3
"""Stage Loop prod watchfaces using the Canopus shared installer generator."""
import importlib.util
import os
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CANOPUS = Path(os.environ.get("CANOPUS_ROOT", ROOT / "../Canopus")).resolve()
SCRIPT = CANOPUS / "scripts" / "build_module_installer_prod.py"
spec = importlib.util.spec_from_file_location("module_prod_builder", SCRIPT)
builder = importlib.util.module_from_spec(spec)
spec.loader.exec_module(builder)
builder.PRODUCTS["loop"] = ("loop", "Loop", 393216, ["appicon_loop.bin"])
sys.argv = [str(SCRIPT), *sys.argv[1:]]
builder.main()
