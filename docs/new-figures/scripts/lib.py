"""Shared pieces for the figures parity scripts (stdlib only).

The class weights live here and nowhere else: a capability's weight is its
class's weight, so the score cannot be tuned by editing numbers in a ledger.
"""

from __future__ import annotations

import hashlib
import json
import os
import subprocess
from pathlib import Path

CLASS_WEIGHTS = {
    "core": 5,
    "editor": 4,
    "control": 3,
    "version": 3,
    "interaction": 2,
    "output": 1,
}
UNCLASSIFIED_WEIGHT = 2
SCORED_CLASSES = set(CLASS_WEIGHTS)


def weight(cap: dict) -> float:
    return float(CLASS_WEIGHTS.get(cap.get("class", ""), UNCLASSIFIED_WEIGHT))


def sha(obj) -> str:
    if not isinstance(obj, (bytes, str)):
        obj = json.dumps(obj, sort_keys=True, ensure_ascii=False)
    if isinstance(obj, str):
        obj = obj.encode("utf-8")
    return hashlib.sha256(obj).hexdigest()


def page_text_hash(page: dict) -> str:
    """Hash of what the climb reads from a page (bullets + control text), not the raw HTML,
    so a deploy that only changes a script tag does not reset the score."""
    return sha({"can_set": page.get("can_set", []), "controls_text": page.get("controls_text", "")})


def ledger_shape(caps: list[dict]) -> str:
    """The denominator of one figure: which capabilities exist and their classes. Evidence is
    deliberately left out, so a round may add evidence but not move the denominator unseen."""
    return sha([(c.get("id"), c.get("class"), c.get("text")) for c in caps])


def read_json(path: Path, default=None):
    try:
        return json.loads(Path(path).read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return default


def write_json(path: Path, obj) -> None:
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(obj, indent=1, ensure_ascii=False) + "\n", encoding="utf-8")


def newest_src_mtime(repo: Path) -> float:
    """Newest mtime of anything that can change the catalog, handles or renders."""
    newest = 0.0
    for root in (repo / "src",):
        for p in root.rglob("*.rs"):
            try:
                newest = max(newest, p.stat().st_mtime)
            except OSError:
                pass
    return newest


def git_head(repo: Path) -> str:
    try:
        head = subprocess.run(["git", "rev-parse", "--short", "HEAD"], cwd=repo, text=True, capture_output=True).stdout.strip()
        dirty = subprocess.run(["git", "status", "--porcelain", "--", "src"], cwd=repo, text=True, capture_output=True).stdout.strip()
        return head + ("+dirty" if dirty else "")
    except OSError:
        return ""


def binary(repo: Path) -> Path:
    """Build the vershion binary once (debug) and return its path."""
    proc = subprocess.run(["cargo", "build", "--quiet", "--bin", "vershion"], cwd=repo, text=True, capture_output=True, timeout=1800)
    if proc.returncode != 0:
        raise SystemExit(f"cargo build failed in {repo}:\n{proc.stderr[-4000:]}")
    target = Path(os.environ.get("CARGO_TARGET_DIR", repo / "target"))
    if not target.is_absolute():
        target = repo / target
    exe = target / "debug" / "vershion"
    if not exe.exists():
        raise SystemExit(f"built, but {exe} is missing")
    return exe


def run_bin(exe: Path, repo: Path, env: dict, timeout: float = 300) -> subprocess.CompletedProcess:
    return subprocess.run([str(exe)], cwd=repo, env={**os.environ, **env}, text=True, capture_output=True, timeout=timeout)
