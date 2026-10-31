#!/usr/bin/env python3
"""Build ABI 2 Loop packages/installers with the official Canopus CLI."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile
import tomllib

ROOT = Path(__file__).resolve().parents[1]


def sdk_revision(root):
    def git(*args):
        return subprocess.check_output(["git", "-C", str(root), *args], text=True).strip()

    return {
        "path": str(root),
        "branch": git("branch", "--show-current"),
        "commit": git("rev-parse", "HEAD"),
        "dirty": bool(git("status", "--porcelain", "--untracked-files=no")),
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("selection", nargs="*", help="device family or exact target")
    args = parser.parse_args()
    if sys.platform == "win32":
        parser.error("run this script inside WSL (see docs/BUILD.md)")
    manifest_path = ROOT / "Canopus.toml"
    text = manifest_path.read_text(encoding="utf-8")
    manifest = tomllib.loads(text)
    all_targets = manifest["targets"]["include"]
    selection = args.selection or os.environ.get("CANOPUS_DEVICE", os.environ.get("CANOPUS_TARGET", "")).split()
    selected = []
    for value in selection:
        matches = [target for target in all_targets if target == value or target.rsplit("-", 1)[0] == value]
        if not matches:
            parser.error(f"unsupported device/target: {value}")
        selected.extend(target for target in matches if target not in selected)
    selected = selected or all_targets
    canopus_root = Path(os.environ.get("CANOPUS_ROOT", ROOT.parent / "Canopus")).resolve()
    cli = os.environ.get("CANOPUS_CLI") or shutil.which("canopus") or str(canopus_root / "target/debug/canopus")
    cli = str(Path(shutil.which(cli) or cli).resolve())
    env = dict(os.environ, CANOPUS_ROOT=str(canopus_root))
    sdk = sdk_revision(canopus_root)
    suffix = "" if selected == all_targets else "-" + "_".join(value.rsplit("-", 1)[1] for value in selected)
    output_dir = ROOT / "dist"
    if suffix:
        output_dir = output_dir / "selected" / suffix.removeprefix("-")
    output = output_dir / f"loop-{manifest['module']['version']}{suffix}.canopus"
    temporary = None
    try:
        if selected != all_targets:
            # Keep the manifest in the project root so official CLI paths and
            # freshness checks refer to the real sources. Write BEFORE build.
            text, count = re.subn(r"(?ms)(^\[targets\]\s*include\s*=\s*)\[[^]]*\]", lambda m: m[1] + json.dumps(selected), text)
            if count != 1:
                raise ValueError("expected exactly one target include list")
            with tempfile.NamedTemporaryFile(mode="w", encoding="utf-8", prefix=".Canopus.build-", suffix=".toml", dir=ROOT, delete=False) as file:
                file.write(text)
                temporary = Path(file.name)
            manifest_path = temporary
        subprocess.run([cli, "build", "--manifest", str(manifest_path)], cwd=ROOT, env=env, check=True)
        subprocess.run([cli, "package", "--manifest", str(manifest_path), "--output", str(output), "--installer"], cwd=ROOT, env=env, check=True)
        subprocess.run([cli, "package", "verify", str(output)], cwd=ROOT, env=env, check=True)
        for family in dict.fromkeys(target.rsplit("-", 1)[0] for target in selected):
            lua = output.parent / "installer" / family / "main.lua"
            subprocess.run(["luac", "-p", str(lua)], check=True)
        provenance = {
            "module": manifest["module"]["id"],
            "version": manifest["module"]["version"],
            "version_code": manifest["module"]["version_code"],
            "sdk": sdk,
            "cli": str(Path(cli).resolve()),
            "cli_sha256": hashlib.sha256(Path(cli).read_bytes()).hexdigest(),
            "package": output.name,
            "package_sha256": hashlib.sha256(output.read_bytes()).hexdigest(),
            "artifacts": {
                target: hashlib.sha256((ROOT / "build" / target / "loop.elf").read_bytes()).hexdigest()
                for target in selected
            },
        }
        info = output.with_suffix(".build-info.json")
        info.write_text(json.dumps(provenance, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
        print(f"Package: {output}")
        print(f"Build info: {info}")
        print(f"Installer directories: {output.parent / 'installer'}")
    finally:
        if temporary is not None:
            temporary.unlink()


if __name__ == "__main__":
    main()
