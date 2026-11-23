#!/usr/bin/env python3
"""Check the evidence on every capability ledger entry, against the real app.

A capability passes only when every token in its evidence passes AND the
evidence is behavioural for its class. A parameter that exists is not proof:
the render has to change, or a named handle has to exist.

Evidence tokens, joined with " + " (all must pass):
  changes:<key>=<value>        rendering with key=value differs from the opening state (PNG bytes)
  changes:{"k":v,"k2":v2}      same, several params at once (JSON object)
  handle:<action>:<tip text>   a handle at the opening state whose tip contains <tip text>
                               (case-insensitive); <action> is drag|click|scroll|label|any
  handle-edit:<key>            a handle that edits <key> inline (double-click)
  label-drag                   the figure is in labels::FIGURES
  ui-test:<rust test path>     the named test (e.g. figuresui::tests::composite_canvas_moves_a_piece) exists in a
                               file that names the figure id, and passes when run (cargo test --exact). For figures
                               the workspace edits with its own canvas instead of figure handles; the round's
                               checker still judges that the test drives the action the capability names.
  param:<key>                  the figure has this parameter            (not behavioural)
  param-choice:<key>=<option>  the parameter offers this option         (not behavioural)
  show-answers | style | label-size   the standard switch exists       (not behavioural)
  symbol:<path>:<text>         <text> occurs in <path>, a file that names the figure id (not behavioural)

Class rules:
  core, control, version, editor   need at least one changes:/handle:/handle-edit:/label-drag/ui-test: token
  interaction                      needs a handle:/handle-edit:/label-drag/ui-test: token
  output                           any token
  out-of-scope                     not checked

The opening-state and override PNGs are kept in --renders so the round's checker
can look at what the probe compared.
"""

from __future__ import annotations

import argparse
import concurrent.futures
import json
import os
import sys
import threading
from datetime import datetime
from pathlib import Path

from lib import CLASS_WEIGHTS, binary, newest_src_mtime, read_json, run_bin, sha, write_json

BEHAVIOURAL = ("changes:", "handle:", "handle-edit:", "label-drag", "ui-test:")
HANDLE_ONLY = ("handle:", "handle-edit:", "label-drag", "ui-test:")
LIST_FILES = {"faq.rs", "interact.rs", "labels.rs", "start.rs", "census.rs", "mod.rs"}


TOKEN_STARTS = ("changes:", "handle:", "handle-edit:", "label-drag", "ui-test:", "param:", "param-choice:", "show-answers", "style", "label-size", "symbol:")


def tokens(ev: str) -> list[str]:
    """Split on " + " only where a new token starts, so an option value like "Solid + liquid" stays whole."""
    out: list[str] = []
    for part in ev.split(" + "):
        if out and not part.strip().startswith(TOKEN_STARTS):
            out[-1] += " + " + part
        else:
            out.append(part)
    return [t.strip() for t in out if t.strip()]


def coerce(fig: dict, key: str, raw):
    p = next((x for x in fig.get("params", []) if x["key"] == key), None)
    if p is None:
        raise KeyError(key)
    if not isinstance(raw, str):
        val = raw
    elif p["kind"] == "bool":
        val = raw.strip().lower() in ("true", "1", "yes", "on")
    elif p["kind"] == "int":
        val = int(float(raw))
    elif p["kind"] == "float":
        val = float(raw)
    else:
        val = raw
    if p["kind"] == "choice" and p.get("options") and val not in p["options"]:
        raise ValueError(f"{key}={val!r} is not one of its options {p['options']}")  # else it reads as a dead control
    if p["kind"] == "float":
        val = float(val)  # a JSON 5.0, so the app's loader sets a float, not an int
    if p["kind"] == "int":
        val = int(val)
    return val


def overrides(fig: dict, body: str) -> dict:
    if body.startswith("{"):
        obj = json.loads(body)
        return {k: coerce(fig, k, v) for k, v in obj.items()}
    key, sep, val = body.partition("=")
    if not sep:
        raise ValueError("changes: needs key=value")
    return {key.strip(): coerce(fig, key.strip(), val)}


class Renderer:
    def __init__(self, repo: Path, renders: Path, src_mtime: float):
        self.repo, self.renders, self.src_mtime = repo, renders, src_mtime
        self.exe: Path | None = None

    def png(self, fid: str, over: dict) -> Path:
        tag = sha({"fid": fid, "over": over, "src": self.src_mtime})[:16]
        path = self.renders / fid / f"{'opening' if not over else tag}.png"
        if path.exists() and path.stat().st_mtime >= self.src_mtime:
            return path
        if self.exe is None:
            self.exe = binary(self.repo)
        path.parent.mkdir(parents=True, exist_ok=True)
        tmp = path.with_name(f"{path.stem}.{threading.get_ident()}.tmp.png")
        r = run_bin(self.exe, self.repo, {"VERSHION_RENDER_FIG": fid, "VERSHION_FIG_PARAMS": json.dumps(over), "VERSHION_OUT": str(tmp)}, timeout=120)
        if r.returncode != 0 or not tmp.exists():
            raise RuntimeError(f"render failed: {r.stderr.strip()[-300:]}")
        os.replace(tmp, path)  # atomic, so a thread comparing the opening PNG never reads half a file
        return path


_UI_TESTS: dict[str, tuple[bool, str]] = {}
_UI_LOCK = threading.Lock()


def ui_test(name: str, fid: str, repo: Path) -> tuple[bool, str]:
    """Run one named Rust test (once per probe run) and say whether it exists for this figure and passes."""
    import re
    import subprocess
    leaf = name.rsplit("::", 1)[-1]
    if not re.fullmatch(r"[A-Za-z0-9_:]+", name) or not leaf:
        return False, "use ui-test:<module::path::test_name>"
    files = [f for f in (repo / "src").rglob("*.rs") if re.search(rf"\bfn {re.escape(leaf)}\s*\(", f.read_text(encoding="utf-8", errors="replace"))]
    if not files:
        return False, f"no test fn {leaf} in src/"
    if not any(f'"{fid}"' in f.read_text(encoding="utf-8", errors="replace") for f in files):
        return False, f"the file holding {leaf} does not name the figure id \"{fid}\""
    with _UI_LOCK:
        if name not in _UI_TESTS:
            r = subprocess.run(["cargo", "test", "--profile", "figtest", "--lib", "--", "--exact", name],
                               cwd=repo, capture_output=True, text=True, timeout=3600)
            out = r.stdout + r.stderr
            if f"test {name} ... ok" in out:
                _UI_TESTS[name] = (True, "ok")
            elif f"test {name} ... FAILED" in out:
                _UI_TESTS[name] = (False, f"test {name} fails")
            else:
                _UI_TESTS[name] = (False, f"test {name} did not run (check the full path): {out.strip()[-200:]}")
        return _UI_TESTS[name]


def check_token(token: str, fid: str, fig: dict, repo: Path, rnd: Renderer, shown: list) -> tuple[bool, str]:
    params = {p["key"]: p for p in fig.get("params", [])}
    if token.startswith("changes:"):
        try:
            over = overrides(fig, token[len("changes:"):].strip())
        except KeyError as e:
            return False, f"parameter not found: {e.args[0]}"
        except ValueError as e:
            return False, f"bad changes: token ({e})"
        try:
            base, alt = rnd.png(fid, {}), rnd.png(fid, over)
        except RuntimeError as e:
            return False, str(e)
        shown.append({"token": token, "opening": str(base), "override": str(alt)})
        if base.read_bytes() == alt.read_bytes():
            return False, f"dead control: {json.dumps(over)} renders the same as the opening state"
        return True, "ok"
    if token.startswith("handle:"):
        action, _, tip = token[len("handle:"):].partition(":")
        action, tip = action.strip(), tip.strip().lower()
        if action not in ("drag", "click", "scroll", "label", "any") or not tip:
            return False, "use handle:<drag|click|scroll|label|any>:<tip text>"
        for h in fig.get("handle_list", []):
            if tip in h["tip"].lower() and (action == "any" or action in h["actions"]):
                return True, "ok"
        tips = sorted({h["tip"] for h in fig.get("handle_list", [])})[:8]
        return False, f"no {action} handle with tip ~ {tip!r} (have: {tips or 'no handles'})"
    if token.startswith("handle-edit:"):
        key = token[len("handle-edit:"):].strip()
        if any(h.get("edit") == key for h in fig.get("handle_list", [])):
            return True, "ok"
        return False, f"no handle edits {key} inline"
    if token == "label-drag":
        return (True, "ok") if fig.get("label_drag") else (False, "figure is not in labels::FIGURES")
    if token.startswith("ui-test:"):
        return ui_test(token[len("ui-test:"):].strip(), fid, repo)
    if token.startswith("param-choice:"):
        key, _, val = token[len("param-choice:"):].partition("=")
        p = params.get(key.strip())
        return (True, "ok") if p and val.strip() in p.get("options", []) else (False, f"option not found: {token}")
    if token.startswith("param:"):
        key = token[len("param:"):].strip()
        return (True, "ok") if key in params else (False, f"parameter not found: {key}")
    if token in ("show-answers", "style", "label-size"):
        return (True, "ok") if fig.get(token.replace("-", "_")) else (False, f"{token} missing")
    if token.startswith("symbol:"):
        path, sep, text = token[len("symbol:"):].partition(":")
        f = repo / path.strip()
        if not sep or not text.strip():
            return False, "use symbol:<path>:<text>"
        if not f.is_file():
            return False, f"no file {path}"
        if f.name in LIST_FILES:
            return False, f"{f.name} lists every figure; name the figure's own module"
        body = f.read_text(encoding="utf-8", errors="replace")
        if f'"{fid}"' not in body:
            return False, f"{path} does not name the figure id \"{fid}\""
        return (True, "ok") if text.strip() in body else (False, f"text not found in {path}")
    return False, f"unknown evidence token: {token}"


def check(cap: dict, fid: str, fig: dict, repo: Path, rnd: Renderer) -> dict:
    cls = cap.get("class", "")
    ev = (cap.get("evidence") or "").strip()
    out = {"pass": False, "why": "", "class": cls, "renders": []}
    if cls == "out-of-scope":
        out["why"] = "out of scope"
        return out
    if cls not in CLASS_WEIGHTS:
        out["why"] = f"unclassified ({cls!r})"
        return out
    if not ev:
        out["why"] = "no evidence"
        return out
    toks = tokens(ev)
    if cls == "interaction" and not any(t.startswith(HANDLE_ONLY) for t in toks):
        out["why"] = "weak evidence: an interaction needs handle:/handle-edit:/label-drag/ui-test:"
        return out
    if cls in ("core", "control", "version", "editor") and not any(t.startswith(BEHAVIOURAL) for t in toks):
        out["why"] = f"weak evidence: a {cls} capability needs changes:/handle:/handle-edit:/label-drag/ui-test:"
        return out
    for t in toks:
        ok, why = check_token(t, fid, fig, repo, rnd, out["renders"])
        if not ok:
            out["why"] = why
            return out
    out.update({"pass": True, "why": "ok"})
    return out


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--repo", default=".")
    ap.add_argument("--out", default="docs/figures-parity")
    ap.add_argument("--renders", default=".figures-parity/renders", help="where the compared PNGs go (not committed)")
    ap.add_argument("--only", nargs="*", help="probe only these figure ids (the rest keep their previous result)")
    ap.add_argument("--strict", action="store_true", help="exit 1 when any capability with evidence fails")
    ap.add_argument("--allow-stale", action="store_true", help="probe even if src/ is newer than ours.json")
    ap.add_argument("--workers", type=int, default=6)
    args = ap.parse_args()
    repo = Path(args.repo).resolve()
    out = Path(args.out)
    snap = read_json(out / "ours.json")
    if not snap or "figures" not in snap:
        raise SystemExit(f"no {out / 'ours.json'} in the current format; run diff.py --refresh")
    if newest_src_mtime(repo) > snap["meta"]["src_mtime"] and not args.allow_stale:
        raise SystemExit("ours.json is older than src/: run diff.py --refresh first (or --allow-stale)")
    ours = snap["figures"]
    capdir = out / "capabilities"
    if not capdir.exists():
        raise SystemExit(f"no {capdir}; run diff.py --init and write the ledger first")
    rnd = Renderer(repo, Path(args.renders).resolve(), snap["meta"]["src_mtime"])
    prev = (read_json(out / "probe.json") or {}).get("figures", {}) if args.only else {}

    jobs = []
    for path in sorted(capdir.glob("*.json")):
        data = read_json(path) or {}
        fid = data.get("figure") or path.stem
        if args.only and fid not in args.only:
            continue
        fig = ours.get(fid)
        for cap in data.get("capabilities", []):
            jobs.append((fid, fig, cap))
    if any("changes:" in (c.get("evidence") or "") for _, _, c in jobs):
        rnd.exe = binary(repo)  # build once, before the threads

    def one(job):
        fid, fig, cap = job
        if fig is None:
            # new-figures copy: a figure not built yet scores 0 without counting as a failed piece of evidence
            return fid, cap.get("id", ""), {"pass": False, "why": "not built", "class": cap.get("class"), "renders": []}
        return fid, cap.get("id", ""), check(cap, fid, fig, repo, rnd)

    result: dict[str, dict] = dict(prev)
    for fid in {j[0] for j in jobs}:
        result[fid] = {"caps": {}}
    bad = 0
    with concurrent.futures.ThreadPoolExecutor(max_workers=args.workers) as pool:
        for fid, cid, res in pool.map(one, jobs):
            result[fid]["caps"][cid] = res
    for fid in sorted(result):
        for cid, res in result[fid]["caps"].items():
            if not res["pass"] and res["why"] not in ("no evidence", "out of scope", "not built") and not res["why"].startswith("unclassified"):
                bad += 1
                print(f"FAIL {fid} / {cid}: {res['why']}", file=sys.stderr)
    write_json(out / "probe.json", {"checked_at": datetime.now().isoformat(timespec="seconds"), "ours_built_at": snap["meta"]["built_at"],
                                    "git": snap["meta"].get("git", ""), "figures": result})
    passed = sum(1 for f in result.values() for c in f["caps"].values() if c["pass"])
    print(f"wrote {out / 'probe.json'}: {passed} capabilities pass, {bad} with evidence fail")
    return 1 if (bad and args.strict) else 0


if __name__ == "__main__":
    raise SystemExit(main())
