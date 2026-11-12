#!/usr/bin/env python3
"""Diff the scraped reference spec against the local figure registry, and score it.

Reference side: `spec.json` from scrape.py. Local side: the app's own catalog
(`VERSHION_FIGURE_CATALOG=1`), its handle list (`VERSHION_FIGURE_HANDLES=1`) and
`labels::FIGURES`.

Outputs (in --out):
  ours.json        local snapshot: params, defaults, handles, plus the source mtime it was built from
  diff.json/.md    one record per reference page: mapped / missing (map says null) / unmapped (not in map)
  capabilities/    seed ledgers (--init), one per figure, with suggested params per bullet
  frozen.json      (--freeze) the round-0 denominator: figure set, ledger shapes, page text hashes
  score.json/.md   weighted score from capabilities/ + probe.json, and whether it is comparable

Nothing here decides a capability is done; probe.py does, from evidence. Weights come
from the capability's class (lib.CLASS_WEIGHTS), never from the ledger.
"""

from __future__ import annotations

import argparse
import re
import sys
from datetime import datetime, timezone
from pathlib import Path

from lib import (CLASS_WEIGHTS, binary, git_head, ledger_shape, newest_src_mtime, page_text_hash, read_json,
                 run_bin, sha, weight, write_json)

# new-figures copy: the map and not-doing list are this climb's own (docs/new-figures/), not the skill's assets.
SKILL = Path(__file__).resolve().parent.parent
DEFAULT_MAP = SKILL / "map.json"
NOT_DOING = SKILL / "not-doing.md"
STOP = set("a an and or the of to for in on at by with as is it its each any one two three their your you can be "
           "from into than then this that these those up down which what where when how not no".split())


def now() -> str:
    return datetime.now(timezone.utc).isoformat(timespec="seconds")


# ---------------------------------------------------------------- local side

def parse_catalog(text: str) -> dict[str, dict]:
    figs: dict[str, dict] = {}
    cur: dict | None = None
    for line in text.splitlines():
        m = re.match(r"^([a-z0-9][a-z0-9-]*) - (.+)$", line)
        if m and not line.startswith(" "):
            cur = {"id": m.group(1), "title": m.group(2).strip(), "blurb": "", "params": []}
            figs[cur["id"]] = cur
            continue
        if cur is None:
            continue
        if not cur["blurb"] and line and not line.startswith(" ") and line != "params:":
            cur["blurb"] = line.strip()
            continue
        m = re.match(r"^\s{2}([a-z0-9_]+) \((.*)\): (.*)$", line)
        if not m:
            continue
        key, label, rest = m.group(1), m.group(2), m.group(3)
        kind, options, default = "text", [], ""

        def dflt(s: str) -> str:
            return s.split("default ")[-1].split(" - ")[0].strip().strip('"') if "default " in s else ""

        if rest.startswith("choice "):
            kind = "choice"
            before, _, after = rest[len("choice "):].partition(", default ")
            options = [x.strip() for x in before.split("|")]
            default = after.split(" - ")[0].strip().strip('"')
        elif rest.startswith("integer "):
            kind, default = "int", dflt(rest)
        elif rest.startswith("number "):
            kind, default = "float", dflt(rest)
        elif rest.startswith("bool "):
            kind, default = "bool", dflt(rest)
        elif rest.startswith("color "):
            kind = "color"
            before, _, after = rest.partition(", default ")
            options = [x.strip() for x in before[len("color "):].split("|")]
            default = after.split(" - ")[0].strip().strip('"')
        elif rest.startswith("multi-line text"):
            kind, default = "long", dflt(rest)
        elif rest.startswith("numbers text"):
            kind, default = "data", dflt(rest)
        elif rest.startswith("list "):
            kind, default = "list", dflt(rest)
        else:
            default = dflt(rest)
        cur["params"].append({"key": key, "label": label, "kind": kind, "options": options, "default": default})
    return figs


def parse_handles(text: str) -> dict[str, list[dict]]:
    out: dict[str, list[dict]] = {}
    for line in text.splitlines():
        parts = line.split("\t")
        if len(parts) != 4:
            continue
        fid, acts, edit, tip = parts
        out.setdefault(fid, []).append({"actions": [a for a in acts.split(",") if a], "edit": None if edit == "-" else edit, "tip": tip})
    return out


def list_from_rust(path: Path, const: str) -> list[str]:
    if not path.exists():
        return []
    m = re.search(rf"const {const}\s*:[^=]*=\s*&?\[(.*?)\];", path.read_text(encoding="utf-8", errors="replace"), re.S)
    return re.findall(r'"([^"]+)"', m.group(1)) if m else []


def load_ours(repo: Path, cache: Path, refresh: bool) -> dict:
    snap = read_json(cache)
    if snap and "figures" in snap and not refresh:
        if newest_src_mtime(repo) > snap.get("meta", {}).get("src_mtime", 0):
            print("note: src/ changed since ours.json was built; pass --refresh before probing", file=sys.stderr)
        return snap
    exe = binary(repo)
    cat = run_bin(exe, repo, {"VERSHION_FIGURE_CATALOG": "1"})
    if cat.returncode != 0:
        raise SystemExit(f"catalog failed:\n{cat.stderr[-4000:]}")
    hnd = run_bin(exe, repo, {"VERSHION_FIGURE_HANDLES": "1"})
    if hnd.returncode != 0 or not hnd.stdout.strip():
        raise SystemExit("VERSHION_FIGURE_HANDLES printed nothing; the repo needs the handle dump in src/main.rs "
                         f"(see references/vershion-map.md).\n{hnd.stderr[-2000:]}")
    figs = parse_catalog(cat.stdout)
    handles = parse_handles(hnd.stdout)
    label_drag = set(list_from_rust(repo / "src/figures/labels.rs", "FIGURES"))
    keys = lambda f: {p["key"] for p in f["params"]}
    for fid, f in figs.items():
        f["handle_list"] = handles.get(fid, [])
        f["handles"] = bool(f["handle_list"])
        f["label_drag"] = fid in label_drag
        f["show_answers"] = "show_answers" in keys(f)
        f["style"] = "style" in keys(f)
        f["label_size"] = "label_size" in keys(f)
    snap = {"meta": {"built_at": now(), "src_mtime": newest_src_mtime(repo), "git": git_head(repo), "figures": len(figs)},
            "figures": figs}
    write_json(cache, snap)
    return snap


# ---------------------------------------------------------------- diff

def interaction_gaps(rec: dict, ours: dict) -> list[str]:
    """First-pass keyword hints only. The task run and the ledger are the authority."""
    f = rec.get("flags", {})
    kinds = {p["kind"] for p in ours.get("params", [])}
    gaps: list[str] = []
    if (f.get("click") or f.get("drag")) and not (ours.get("handles") or ours.get("label_drag")):
        gaps.append("reference mentions click/drag; ours has no handles")
    if f.get("paste") and not ({"data", "list", "long"} & kinds):
        gaps.append("reference pastes a table; ours has no data/list control")
    if f.get("rows") and not ({"list", "data", "long"} & kinds):
        gaps.append("reference adds/removes rows; ours has no list/data/text-table control")
    if f.get("student") and not ours.get("show_answers"):
        gaps.append("reference has a student version; ours has no show_answers")
    if f.get("black_and_white") and not ours.get("style"):
        gaps.append("reference has black-and-white; ours has no style switch")
    return gaps


def words(s: str) -> set[str]:
    return {w for w in re.findall(r"[a-z]+", s.lower()) if len(w) > 2 and w not in STOP}


def suggest(text: str, fig: dict, n: int = 4) -> list[str]:
    """Params whose key/label share words with a reference bullet: a starting point for evidence."""
    want = words(text)
    scored = []
    for p in fig.get("params", []):
        have = words(p["label"] + " " + p["key"].replace("_", " "))
        hit = len(want & have)
        if hit:
            scored.append((-hit, p["key"]))
    return [k for _, k in sorted(scored)[:n]]


def load_map(path: Path) -> dict:
    data = read_json(path, {})
    return data.get("figures", data)


def build_records(spec: dict, mapping: dict, ours: dict) -> list[dict]:
    records = []
    for site, page_set in spec["sites"].items():
        for slug, page in page_set.get("pages", {}).items():
            key = f"{site}/{slug}"
            if "error" in page:
                records.append({"site": site, "slug": slug, "status": "error", "error": page["error"]})
                continue
            if key not in mapping:
                status, ours_id, cross = "unmapped", None, False
            else:
                entry = mapping[key]
                ours_id = entry.get("ours") if isinstance(entry, dict) else entry
                cross = bool(isinstance(entry, dict) and entry.get("cross_listed"))
                status = "mapped" if ours_id else "missing"
            # new-figures copy: an unbuilt figure is ledgered, frozen and scored (at 0) under its slug
            fid = ours_id or slug
            rec = {"site": site, "slug": slug, "status": status, "title": page.get("title", ""), "ours_id": ours_id, "fid": fid,
                   "cross_listed": cross, "can_set": page.get("can_set", []), "flags": page.get("flags", {}),
                   "text_sha256": page.get("text_sha256") or page_text_hash(page)}
            if status == "mapped" and ours_id not in ours:
                rec["status"] = "map-stale"
            if rec["status"] == "mapped":
                o = ours[ours_id]
                rec.update({"params": [p["key"] for p in o["params"]], "param_count": len(o["params"]),
                            "handles": len(o["handle_list"]), "label_drag": o["label_drag"],
                            "show_answers": o["show_answers"], "style": o["style"],
                            "interaction_gaps": interaction_gaps(rec, o)})
            records.append(rec)
    return records


def init_capabilities(records: list[dict], ours: dict, out: Path, include_cross: bool) -> int:
    capdir = out / "capabilities"
    capdir.mkdir(parents=True, exist_ok=True)
    made = 0
    for rec in records:
        fid = rec.get("fid")
        if rec["status"] not in ("mapped", "missing") or not rec.get("can_set") or (rec["cross_listed"] and not include_cross):
            continue
        path = capdir / f"{fid}.json"
        if path.exists():
            continue
        caps = [{"id": f"cap-{i + 1:02d}", "text": t, "class": "unclassified", "evidence": "", "note": "",
                 "suggest": suggest(t, ours[fid]) if fid in ours else []} for i, t in enumerate(rec["can_set"])]
        write_json(path, {"figure": fid, "site": rec["site"], "slug": rec["slug"], "capabilities": caps})
        made += 1
    return made


def load_capabilities(out: Path) -> dict[str, dict]:
    got: dict[str, dict] = {}
    for p in sorted((out / "capabilities").glob("*.json")):
        data = read_json(p)
        if isinstance(data, dict) and "capabilities" in data:
            got[data.get("figure") or p.stem] = data
    return got


def ledger_problems(fid: str, caps: list[dict]) -> list[str]:
    bad = []
    seen = set()
    for c in caps:
        cid = c.get("id", "")
        if cid in seen:
            bad.append(f"{fid}/{cid}: duplicate id")
        seen.add(cid)
        cls = c.get("class")
        if cls == "unclassified" or (cls not in CLASS_WEIGHTS and cls != "out-of-scope"):
            bad.append(f"{fid}/{cid}: class {cls!r}")
        if cls == "out-of-scope" and not (c.get("note") or "").strip():
            bad.append(f"{fid}/{cid}: out-of-scope with no reason in note")
    return bad


# ---------------------------------------------------------------- freeze and score

def map_sha(path: Path) -> str:
    """new-figures copy: hash of the map with `ours` set back to null where it is the page's own slug,
    so marking a figure built (ours null -> its planned id) does not read as the map moving."""
    data = read_json(path, {})
    figs = data.get("figures", data)
    norm = {k: ({**v, "ours": None} if isinstance(v, dict) and v.get("ours") == k.split("/", 1)[-1] else v)
            for k, v in figs.items()}
    return sha(norm)


def freeze(records: list[dict], caps: dict[str, dict], map_path: Path, out: Path) -> None:
    problems: list[str] = []
    scope: dict[str, dict] = {}
    for rec in records:
        if rec["status"] in ("unmapped", "map-stale", "error"):
            problems.append(f"{rec['site']}/{rec['slug']}: status {rec['status']} (fix map.json or re-scrape)")
            continue
        if rec["status"] not in ("mapped", "missing") or rec["cross_listed"]:
            continue
        fid = rec["fid"]
        if fid not in caps:
            problems.append(f"{fid}: no capabilities/{fid}.json")
            continue
        problems += ledger_problems(fid, caps[fid]["capabilities"])
        scope[fid] = {"page": f"{rec['site']}/{rec['slug']}", "text_sha256": rec["text_sha256"],
                      "shape": ledger_shape(caps[fid]["capabilities"])}
    if problems:
        raise SystemExit("cannot freeze:\n  " + "\n  ".join(problems))
    missing = sorted(f"{r['site']}/{r['slug']}" for r in records if r["status"] == "missing")
    write_json(out / "frozen.json", {"frozen_at": now(), "map_sha256": map_sha(map_path),
                                     "not_doing_sha256": sha(NOT_DOING.read_bytes()) if NOT_DOING.exists() else "",
                                     "missing_figures": missing, "figures": scope})
    print(f"froze {len(scope)} figures ({len(missing)} reference figures not built)", file=sys.stderr)


def score_all(records: list[dict], caps: dict[str, dict], probes: dict, out: Path, map_path: Path) -> dict:
    frozen = read_json(out / "frozen.json")
    drift: list[str] = []
    probe_path = out / "probe.json"
    stale_inputs = [p for p in [out / "ours.json", *sorted((out / "capabilities").glob("*.json"))]
                    if probe_path.exists() and p.exists() and p.stat().st_mtime > probe_path.stat().st_mtime]
    if not probe_path.exists():
        drift.append("no probe.json: every capability scores 0")
    elif stale_inputs:
        drift.append(f"probe.json is older than {len(stale_inputs)} input(s) (e.g. {stale_inputs[0].name}); re-run probe.py")
    pages = {r["fid"]: r for r in records if r.get("fid")}
    if frozen:
        fids = sorted(frozen["figures"])
        if map_sha(map_path) != frozen.get("map_sha256"):
            drift.append("map.json changed since freeze")
        if NOT_DOING.exists() and sha(NOT_DOING.read_bytes()) != frozen.get("not_doing_sha256"):
            drift.append("not-doing.md changed since freeze")
        for fid in fids:
            fz = frozen["figures"][fid]
            if fid not in caps:
                drift.append(f"{fid}: ledger deleted")
            elif ledger_shape(caps[fid]["capabilities"]) != fz["shape"]:
                drift.append(f"{fid}: capability list or classes changed since freeze (denominator moved)")
            rec = pages.get(fid)
            if rec and rec.get("text_sha256") != fz["text_sha256"]:
                drift.append(f"{fid}: reference page {fz['page']} changed since freeze")
        extra = sorted(set(caps) - set(fids))
        if extra:
            drift.append(f"ledgers outside the frozen set are not scored: {', '.join(extra)}")
    else:
        fids = sorted(caps)
        drift.append("not frozen: run diff.py --freeze after round 0; until then the figure set can move")

    figures: dict[str, dict] = {}
    got_all = tot_all = 0.0
    for fid in fids:
        cl = caps.get(fid, {}).get("capabilities", [])
        pr = probes.get(fid, {}).get("caps", {})
        got = tot = 0.0
        unclassified = 0
        gaps = []
        for c in cl:
            if c.get("class") == "out-of-scope":
                continue
            if c.get("class") not in CLASS_WEIGHTS:
                unclassified += 1
            w = weight(c)
            tot += w
            if pr.get(c.get("id", ""), {}).get("pass"):
                got += w
            else:
                gaps.append({"id": c.get("id"), "class": c.get("class"), "weight": w, "text": c.get("text", "")[:120],
                             "why": pr.get(c.get("id", ""), {}).get("why", "not probed")})
        gaps.sort(key=lambda g: -g["weight"])
        figures[fid] = {"score": round(100 * got / tot, 1) if tot else 0.0, "got": got, "total": tot,
                        "unclassified": unclassified, "gaps": gaps}
        got_all += got
        tot_all += tot
    if any(f["unclassified"] for f in figures.values()):
        drift.append("some capabilities are unclassified (counted at weight 2)")
    scores = [f["score"] for f in figures.values()]
    built = sum(1 for r in records if r["status"] == "mapped" and not r["cross_listed"])
    missing = [f"{r['site']}/{r['slug']}" for r in records if r["status"] == "missing"]
    return {"generated_at": now(), "probe_checked_at": (read_json(probe_path) or {}).get("checked_at", ""),
            "comparable": bool(frozen) and not drift, "drift": drift,
            "pooled": round(100 * got_all / tot_all, 1) if tot_all else 0.0,
            "mean": round(sum(scores) / len(scores), 1) if scores else 0.0,
            "figures_scored": len(figures), "figures_built": built, "figures_missing": missing,
            "figures": figures}


def score_md(s: dict) -> list[str]:
    lines = ["# Figures parity score", "", f"Generated: {s['generated_at']} · probe: {s['probe_checked_at'] or 'none'}", "",
             f"**Pooled {s['pooled']:.1f}** · mean {s['mean']:.1f} over {s['figures_scored']} figures · "
             f"built {s['figures_built']} of {s['figures_built'] + len(s['figures_missing'])} reference figures"
             + (f" (missing: {', '.join(s['figures_missing'])})" if s["figures_missing"] else ""), "",
             f"Comparable with the frozen baseline: **{'yes' if s['comparable'] else 'no'}**", ""]
    lines += [f"- {d}" for d in s["drift"]] + ([""] if s["drift"] else [])
    for fid, v in sorted(s["figures"].items(), key=lambda kv: kv[1]["score"]):
        lines.append(f"- `{fid}`: {v['score']:.0f} ({v['got']:g}/{v['total']:g})")
        for g in v["gaps"][:6]:
            lines.append(f"  - {g['id']} [{g['class']} {g['weight']:g}] {g['text']} — {g['why']}")
    return lines + [""]


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--spec", default=".figures-parity/scrape/spec.json")
    ap.add_argument("--repo", default=".")
    ap.add_argument("--out", default="docs/figures-parity")
    ap.add_argument("--map", default=str(DEFAULT_MAP))
    ap.add_argument("--refresh", action="store_true", help="rebuild ours.json from the app (needed after any src/ change)")
    ap.add_argument("--init", action="store_true", help="seed capabilities/*.json from the reference bullets")
    ap.add_argument("--include-cross", action="store_true", help="also seed figures cross-listed on another site")
    ap.add_argument("--freeze", action="store_true", help="freeze the classified ledgers as the round-0 denominator")
    args = ap.parse_args()

    repo = Path(args.repo).resolve()
    out = Path(args.out)
    out.mkdir(parents=True, exist_ok=True)
    map_path = Path(args.map)
    spec = read_json(Path(args.spec))
    if not spec:
        raise SystemExit(f"cannot read {args.spec}; run scrape.py first")
    ours = load_ours(repo, out / "ours.json", args.refresh)["figures"]
    records = build_records(spec, load_map(map_path), ours)
    write_json(out / "diff.json", {"generated_at": now(), "spec_generated_at": spec.get("generated_at"), "figures": records})

    if args.init:
        print(f"seeded {init_capabilities(records, ours, out, args.include_cross)} ledgers", file=sys.stderr)
    caps = load_capabilities(out)
    if args.freeze:
        freeze(records, caps, map_path, out)

    probes = (read_json(out / "probe.json") or {}).get("figures", {})
    s = score_all(records, caps, probes, out, map_path)
    write_json(out / "score.json", s)
    sm = score_md(s)
    (out / "score.md").write_text("\n".join(sm), encoding="utf-8")

    lines = ["# Figures parity diff", "", f"Reference scraped {spec.get('generated_at', '?')} · local `{out / 'ours.json'}`", ""]
    unmapped = [r for r in records if r["status"] in ("unmapped", "map-stale")]
    if unmapped:
        lines += ["## Needs a decision: pages not in map.json (a new reference figure, or a nav page)", ""]
        lines += [f"- `{r['site']}/{r['slug']}` — {r.get('title', '')} ({r['status']})" for r in unmapped] + [""]
    lines += ["The click/drag column is a keyword hint. The ledger and the round's task run are the authority.", ""]
    by_site: dict[str, list[dict]] = {}
    for r in records:
        by_site.setdefault(r["site"], []).append(r)
    for site, rs in by_site.items():
        lines += [f"## {site}", "", "| figure | ours | params | handles | bullets | hints | score |", "|---|---|---:|---:|---:|---|---:|"]
        for r in sorted(rs, key=lambda x: x["slug"]):
            if r["status"] != "mapped":
                lines.append(f"| {r['slug']} | **{r['status']}** | | | {len(r.get('can_set', []))} | | |")
                continue
            sc = s["figures"].get(r["ours_id"], {}).get("score")
            lines.append(f"| {r['slug']}{' (cross)' if r['cross_listed'] else ''} | `{r['ours_id']}` | {r['param_count']} | "
                         f"{r['handles']} | {len(r['can_set'])} | {'; '.join(r['interaction_gaps']) or '—'} | "
                         f"{'' if sc is None else f'{sc:.0f}'} |")
        lines.append("")
    missing = [f"`{r['site']}/{r['slug']}`" for r in records if r["status"] == "missing"]
    lines += ["## Reference figures we do not have", "", ", ".join(missing) or "(none)", ""]
    used = {r.get("ours_id") for r in records}
    lines += [f"Our figures with no reference page: {len(set(ours) - used)}", ""]
    lines += sm[2:]
    (out / "diff.md").write_text("\n".join(lines), encoding="utf-8")
    print("\n".join(sm[:12]))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
