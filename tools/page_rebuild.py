#!/usr/bin/env python3
"""Rebuild and audit every scan using local documentaSHion/WordCraft builders.

No model requests, account settings or keys are used. A local layout is input
evidence, not an independent transcription oracle. Reports keep build integrity,
source matching and human review separate.
"""
from __future__ import annotations

import argparse
from collections import Counter
from concurrent.futures import ThreadPoolExecutor, as_completed
import hashlib
import html
import inspect
import json
import math
import os
from pathlib import Path
import re
import shutil
import signal
import subprocess
import tempfile
import time
import unicodedata
import xml.etree.ElementTree as ET
import zipfile

from PIL import Image, ImageOps
from pypdf import PdfReader, PdfWriter

ROOT = Path(__file__).resolve().parents[1]
NS = {"w": "http://schemas.openxmlformats.org/wordprocessingml/2006/main",
      "m": "http://schemas.openxmlformats.org/officeDocument/2006/math"}
COMMANDS = {"frac": "", "dfrac": "", "tfrac": "", "left": "", "right": "",
            "sqrt": "", "text": "", "mathrm": "", "mathbf": "", "mathit": "",
            "cdot": "*", "times": "*", "div": "/", "pm": "±",
            "pi": "π", "theta": "θ", "alpha": "α", "beta": "β",
            "le": "≤", "leq": "≤", "ge": "≥", "geq": "≥", "ne": "≠"}


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def json_bytes(value) -> bytes:
    return json.dumps(value, sort_keys=True, ensure_ascii=False).encode()


def write_json(path: Path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile(dir=path.parent, delete=False) as f:
        tmp = Path(f.name)
        f.write(json.dumps(value, indent=2, ensure_ascii=False).encode() + b"\n")
    os.replace(tmp, path)


def contained(root: Path, relative: str) -> Path:
    """A manifest may select files within its root, never escape it."""
    path = (root / relative).resolve()
    if not path.is_relative_to(root.resolve()):
        raise ValueError(f"Path escapes input root: {relative}")
    return path


def tokens(text: str) -> list[str]:
    text = unicodedata.normalize("NFKC", text).lower().replace("−", "-")
    return re.findall(r"[^\W_]+|[+\-=/^<>≤≥≠]", text, re.UNICODE)


def walk(value):
    if isinstance(value, dict):
        yield value
        for v in value.values():
            yield from walk(v)
    elif isinstance(value, list):
        for v in value:
            yield from walk(v)


def math_signature(source: str) -> dict:
    unknown = []
    def command(match):
        name = match[1]
        if name not in COMMANDS:
            unknown.append(name)
        return COMMANDS.get(name, name)
    flat = re.sub(r"\\([A-Za-z]+)", command, source)
    flat = re.sub(r"\\[,;! ]", "", flat)
    return {"glyphs": re.sub(r"[\s{}_^$]", "", flat).replace("−", "-").replace("·", "*").replace("×", "*"),
            "fractions": len(re.findall(r"\\(?:d|t)?frac\b", source)),
            "powers": source.count("^"), "roots": len(re.findall(r"\\sqrt\b", source)),
            "unknown_commands": unknown}


def normalized_sheet(sheet: dict) -> tuple[dict, list]:
    """Repair only doubled, recognized LaTeX commands, recording every repair."""
    sheet = json.loads(json.dumps(sheet))
    repairs = []
    for obj in walk(sheet):
        if isinstance(obj.get("math"), str):
            before = obj["math"]
            after = re.sub(r"\\\\(?=(" + "|".join(COMMANDS) + r")\b)", r"\\", before)
            if after != before:
                obj["math"] = after
                repairs.append({"from": before, "to": after, "reason": "doubled LaTeX command escape"})
        if obj.get("type") == "figure" and obj.get("vershion") == "coordinate-grid":
            equations = obj.get("params", {}).get("equations", "")
            if "\n" in equations:
                fixed = "; ".join(row.strip() for row in equations.splitlines() if row.strip())
                obj["params"]["equations"] = fixed
                repairs.append({"from": equations, "to": fixed, "reason": "coordinate-grid equation rows use semicolons"})
    return sheet, repairs


def apply_layout_fixes(sheet: dict, patches: list) -> tuple[dict, list]:
    sheet = json.loads(json.dumps(sheet))
    applied = []
    for patch in patches:
        if "figure" in patch:
            figures = [obj for obj in walk(sheet) if obj.get("type") == "figure"]
            parent = figures[patch["figure"]]["params"]
            key = patch["parameter"]
        else:
            parent = sheet
            for part in patch["path"][:-1]:
                parent = parent[part]
            key = patch["path"][-1]
        before = parent.get(key) if isinstance(parent, dict) else parent[key]
        if before == patch["to"]:
            continue
        if before != patch["from"]:
            raise ValueError(f"Reviewed layout correction no longer matches: {patch}")
        parent[key] = patch["to"]
        applied.append(patch)
    return sheet, applied


class Runner:
    """Drain output, bound execution time, and kill the whole process group on timeout."""
    def __init__(self, timeout=120):
        self.timeout = timeout

    def run(self, argv, log: Path, input_data=None) -> str:
        allowed = {"documentashion", "wordcraft-cli", "vershion-figure", "pdftoppm", "pdftotext", "tesseract"}
        if Path(argv[0]).name not in allowed or (Path(argv[0]).name == "documentashion" and argv[1:3] != ["ws", "build"]):
            raise ValueError(f"Non-local builder command refused: {argv[:3]}")
        env = {k: v for k, v in os.environ.items()
               if not k.endswith("API_KEY") and k not in {"VERSHION_RELAY_KEY", "DOCUMENTASHION_AI_KEY"}}
        env["WORDCRAFT_REGION"] = "en-US"
        env["DOCUMENTASHION_VERSHION_FIGURE"] = str(TOOLS["figure"]["path"])
        log.parent.mkdir(parents=True, exist_ok=True)
        started = time.monotonic()
        p = subprocess.Popen([str(a) for a in argv], env=env, stdin=subprocess.PIPE if input_data is not None else subprocess.DEVNULL, stdout=subprocess.PIPE,
                             stderr=subprocess.PIPE, start_new_session=True)
        try:
            stdout, stderr = p.communicate(input=input_data.encode() if input_data is not None else None, timeout=self.timeout)
        except subprocess.TimeoutExpired:
            os.killpg(p.pid, signal.SIGKILL)
            stdout, stderr = p.communicate()
            log.with_suffix(".stdout.log").write_bytes(stdout)
            log.with_suffix(".stderr.log").write_bytes(stderr)
            raise RuntimeError(f"Timed out after {self.timeout}s: {Path(argv[0]).name}")
        log.with_suffix(".stdout.log").write_bytes(stdout)
        log.with_suffix(".stderr.log").write_bytes(stderr)
        if p.returncode:
            raise RuntimeError(f"{Path(argv[0]).name} exited {p.returncode}: {stderr.decode(errors='replace')[-600:]}")
        return stdout.decode(errors="replace")


def tool(path) -> dict:
    path = Path(shutil.which(str(path)) or path).resolve()
    if not path.is_file():
        raise ValueError(f"Local tool is missing: {path}")
    return {"path": str(path), "sha256": digest(path.read_bytes())}


def build_identity(sheet: dict, tools: dict) -> str:
    return digest(json_bytes({"builder_version": 1, "sheet": sheet, "tools": tools, "region": "en-US"}))


def cached_build(out: Path):
    names = ["page.docx", "page.pdf", "page.png"]
    try:
        saved = json.loads((out / "validation.json").read_text())
        hashes = saved["hashes"]
        if not all(name in hashes and digest((out / name).read_bytes()) == hashes[name] for name in names):
            return None
        if not (out / "sheet.json").is_file():
            return None
        return saved
    except (OSError, ValueError, KeyError, TypeError):
        return None


def word_math_glyphs(node) -> str:
    """Word stores stretchable delimiters as properties rather than text runs."""
    if node.tag == "{" + NS["m"] + "}t":
        return node.text or ""
    if node.tag == "{" + NS["m"] + "}d":
        def delimiter(name, default):
            prop = node.find("m:dPr/m:" + name, NS)
            return prop.get("{" + NS["m"] + "}val", default) if prop is not None else default
        return delimiter("begChr", "(") + delimiter("sepChr", "|").join(
            word_math_glyphs(child) for child in node.findall("m:e", NS)) + delimiter("endChr", ")")
    return "".join(word_math_glyphs(child) for child in node)


def inspect_docx(path: Path, sheet: dict) -> dict:
    with zipfile.ZipFile(path) as z:
        names = ["word/document.xml"] + sorted(n for n in z.namelist() if re.fullmatch(r"word/header\d*\.xml", n)) + sorted(n for n in z.namelist() if re.fullmatch(r"word/footer\d*\.xml", n))
        roots = [ET.fromstring(z.read(name)) for name in names]
    printed = [obj for section in ["blocks", "header", "footer"] for obj in walk(sheet.get(section, []))]
    expected_tokens = tokens(" ".join(obj["text"] for obj in printed if isinstance(obj.get("text"), str)))
    actual_tokens = tokens(" ".join(n.text or "" for root in roots for n in root.findall(".//w:t", NS)))
    expected_text, actual_text = Counter(expected_tokens), Counter(actual_tokens)
    missing = list((expected_text - actual_text).elements())
    expected_math = [obj["math"] for obj in printed if isinstance(obj.get("math"), str)]
    actual_math = [n for root in roots for n in root.findall(".//m:oMath", NS)]
    math_errors = []
    if len(expected_math) != len(actual_math):
        math_errors.append(f"{len(expected_math)} expected equations, {len(actual_math)} native Word equations")
    for i, (formula, node) in enumerate(zip(expected_math, actual_math), 1):
        expected = math_signature(formula)
        glyphs = word_math_glyphs(node)
        actual = {"glyphs": re.sub(r"\s", "", glyphs).replace("−", "-").replace("·", "*").replace("×", "*"),
                  "fractions": len(node.findall(".//m:f", NS)),
                  "powers": len(node.findall(".//m:sSup", NS)) + len(node.findall(".//m:sSubSup", NS)),
                  "roots": len(node.findall(".//m:rad", NS))}
        if expected["unknown_commands"] or any(expected[k] != actual[k] for k in actual):
            math_errors.append({"equation": i, "source": formula, "expected": expected, "actual": actual})
    expected_tables = sum(obj.get("type") == "table" for obj in walk(sheet))
    actual_tables = sum(len(root.findall(".//w:tbl", NS)) for root in roots)
    expected_figures = sum(obj.get("type") == "figure" for obj in walk(sheet)) + bool(sheet.get("page", {}).get("background"))
    actual_figures = sum(len(root.findall(".//w:drawing", NS)) for root in roots)
    return {"missing_text_tokens": missing, "text_order_ok": expected_tokens == actual_tokens, "math_errors": math_errors,
            "expected_equations": len(expected_math), "native_equations": len(actual_math),
            "expected_tables": expected_tables, "tables": actual_tables,
            "expected_figures": expected_figures, "figures": actual_figures,
            "ok": expected_tokens == actual_tokens and not missing and not math_errors and expected_tables == actual_tables and expected_figures == actual_figures}


def inspect_pdf(path: Path, bbox_xml: str) -> dict:
    pages = PdfReader(path).pages
    words = []
    errors = []
    overlaps = []
    rules = []
    for pdf_page in pages:
        points, rectangles = [], []
        height = float(pdf_page.mediabox.height)
        def visit(op, args, cm, tm):
            def pt(x, y):
                return (float(x) * cm[0] + float(y) * cm[2] + cm[4],
                        height - (float(x) * cm[1] + float(y) * cm[3] + cm[5]))
            if op == b"m":
                points.clear()
                points.append(pt(*args))
            elif op == b"l":
                points.append(pt(*args))
            elif op == b"re":
                x, y, w, h = map(float, args)
                if abs(h) <= 1.5 and abs(w) >= 3:
                    rectangles.append((pt(x, y + h / 2), pt(x + w, y + h / 2)))
            elif op in (b"S", b"s"):
                for a, b in zip(points, points[1:]):
                    if abs(a[1] - b[1]) <= .2 and abs(a[0] - b[0]) >= 3:
                        rules.append((min(a[0], b[0]), max(a[0], b[0]), a[1]))
                points.clear()
            elif op in (b"f", b"f*", b"B"):
                for a, b in rectangles:
                    rules.append((min(a[0], b[0]), max(a[0], b[0]), a[1]))
                rectangles.clear()
        pdf_page.extract_text(visitor_operand_before=visit)
    for page in ET.fromstring(bbox_xml).iter():
        if page.tag.rsplit("}", 1)[-1] != "page":
            continue
        width, height = float(page.attrib["width"]), float(page.attrib["height"])
        for n in page.iter():
            if n.tag.rsplit("}", 1)[-1] != "word":
                continue
            box = [float(n.attrib[k]) for k in ["xMin", "yMin", "xMax", "yMax"]]
            if box[0] < -0.5 or box[1] < -0.5 or box[2] > width + 0.5 or box[3] > height + 0.5:
                errors.append({"text": n.text, "box": box})
            for x0, x1, y in rules:
                if x1 - x0 < 40:
                    continue
                if (box[1] + .18 * (box[3] - box[1]) < y < box[3] - .18 * (box[3] - box[1])
                    and min(x1, box[2]) - max(x0, box[0]) > max(2, .25 * (box[2] - box[0]))):
                    overlaps.append({"text": n.text, "box": box, "rule": [x0, x1, y]})
                    break
            words.append({"text": n.text or "", "box": box, "x": (box[0] + box[2]) / (2 * width),
                          "y": (box[1] + box[3]) / (2 * height)})
    return {"page_count": len(pages), "out_of_page_words": errors, "text_rule_overlaps": overlaps, "horizontal_rules": rules,
            "ok": len(pages) == 1 and not errors and not overlaps, "words": words}


def inspect_answer_leaders(sheet: dict, pdf: dict) -> dict:
    """Find visible tab-leader answer lines near unique paragraph text anchors."""
    locations = {}
    for word in pdf["words"]:
        for token in tokens(word["text"]):
            if token.isalpha() and len(token) >= 4:
                locations.setdefault(token, []).append(word["box"])
    checked, unable = [], 0
    for obj in walk(sheet.get("blocks", [])):
        if obj.get("type") != "p":
            continue
        stops = obj.get("tabs", [])
        count = sum(bool(run.get("tab")) for run in obj.get("runs", []))
        if not count or len(stops) < count or not all(t.get("leader") == "underscore" for t in stops[:count]):
            continue
        plain = " ".join(run.get("text", "") for run in obj.get("runs", []))
        anchors = [locations[t][0] for t in set(tokens(plain)) if len(locations.get(t, [])) == 1]
        if len(anchors) < 2:
            unable += 1
            continue
        top, bottom = min(a[1] for a in anchors), max(a[3] for a in anchors)
        if bottom - top > 3 * sheet.get("size", 11):
            unable += 1
            continue
        lines = sorted((x0, x1, y) for x0, x1, y in pdf["horizontal_rules"]
                       if top - 1 <= y <= bottom + 3 and x1 - x0 >= 15)
        # WordCraft can render underscore leaders as text rather than paths.
        for word in pdf["words"]:
            x0, y0, x1, y1 = word["box"]
            if re.fullmatch(r"_{2,}", word["text"]) and top - 1 <= (y0+y1)/2 <= bottom + 3:
                lines.append((x0, x1, (y0+y1)/2))
        # Filled rectangles can expose the same rule's two edges.
        distinct = []
        for line in sorted(lines):
            if not any(abs(line[0]-p[0]) < 1 and abs(line[1]-p[1]) < 1 and abs(line[2]-p[2]) < 1 for p in distinct):
                distinct.append(line)
        checked.append({"paragraph": plain, "expected_answer_lines": count,
                        "visible_answer_lines": len(distinct), "ok": len(distinct) >= count})
    return {"paragraphs": checked, "uncheckable_paragraphs": unable, "ok": all(c["ok"] for c in checked)}


def inspect_figures(sheet: dict, out: Path, runner: Runner) -> dict:
    checks = []
    for i, obj in enumerate(obj for obj in walk(sheet) if obj.get("type") == "figure"):
        if obj.get("vershion") != "coordinate-grid":
            continue
        rows = [s.strip() for s in obj.get("params", {}).get("equations", "").split(";") if s.strip()]
        expected = len(rows)
        if re.search(r"\btwo\b.*\blines\b", obj.get("alt", ""), re.IGNORECASE):
            expected = max(expected, 2)
        svg = runner.run([TOOLS["figure"]["path"]], out / f"figure-check-{i}",
                         json.dumps({"id": obj["vershion"], "params": obj.get("params", {})}))
        actual = sum(n.tag.rsplit("}", 1)[-1] == "polyline" for n in ET.fromstring(svg).iter())
        checks.append({"figure": i, "required_curves": expected, "drawn_curves": actual, "ok": actual >= expected})
    return {"figures": checks, "ok": all(c["ok"] for c in checks)}


def build(sheet: dict, cache: Path, runner: Runner) -> dict:
    key = build_identity(sheet, TOOLS)
    out = cache / key
    expected_files = ["page.docx", "page.pdf", "page.png", "validation.json"]
    saved = cached_build(out)
    if saved is not None:
        # Re-score cached artifacts with the current validators; a new check
        # must not inherit a previous pass (or fail) merely from saved JSON.
        saved["docx"] = inspect_docx(out / "page.docx", sheet)
        bbox = runner.run([TOOLS["pdftotext"]["path"], "-bbox", str(out / "page.pdf"), "-"], out / "bbox")
        saved["pdf"] = inspect_pdf(out / "page.pdf", bbox)
        saved["pdf"]["answer_lines"] = inspect_answer_leaders(sheet, saved["pdf"])
        saved["pdf"]["ok"] &= saved["pdf"]["answer_lines"]["ok"]
        saved["figures"] = inspect_figures(sheet, out, runner)
        saved["ok"] = saved["docx"]["ok"] and saved["pdf"]["ok"] and saved["figures"]["ok"]
        write_json(out / "validation.json", saved)
        return {"directory": str(out), "cache_hit": True, **saved}
    out.mkdir(parents=True, exist_ok=True)
    write_json(out / "sheet.json", sheet)
    runner.run([TOOLS["documentashion"]["path"], "ws", "build", str(out / "sheet.json"), str(out / "page.docx")], out / "build")
    runner.run([TOOLS["wordcraft"]["path"], "convert", str(out / "page.docx"), str(out / "page.pdf"), "--strict"], out / "pdf")
    bbox = runner.run([TOOLS["pdftotext"]["path"], "-bbox", str(out / "page.pdf"), "-"], out / "bbox")
    pdf = inspect_pdf(out / "page.pdf", bbox)
    pdf["answer_lines"] = inspect_answer_leaders(sheet, pdf)
    pdf["ok"] &= pdf["answer_lines"]["ok"]
    runner.run([TOOLS["pdftoppm"]["path"], "-f", "1", "-singlefile", "-scale-to", "1600", "-png", str(out / "page.pdf"), str(out / "page")], out / "render")
    result = {"docx": inspect_docx(out / "page.docx", sheet), "pdf": pdf, "figures": inspect_figures(sheet, out, runner),
              "hashes": {name: digest((out / name).read_bytes()) for name in expected_files[:-1]}}
    result["ok"] = result["docx"]["ok"] and result["pdf"]["ok"] and result["figures"]["ok"]
    write_json(out / "validation.json", result)
    return {"directory": str(out), "cache_hit": False, **result}


def source_audit(source: Path, rotation: int, dest: Path, rebuilt: dict, runner: Runner) -> dict:
    identity = digest(json_bytes({"source": digest(source.read_bytes()), "rotation": rotation,
        "pdf": rebuilt["hashes"]["page.pdf"], "ocr": TOOLS["tesseract"]["sha256"],
        "validator": digest((inspect.getsource(source_audit) + inspect.getsource(tokens)).encode())}))
    cached = dest / "source-validation.json"
    if cached.exists() and (dest / "source.png").exists():
        saved = json.loads(cached.read_text())
        if saved["identity"] == identity and saved["preview_sha256"] == digest((dest / "source.png").read_bytes()):
            return {**saved["result"], "cache_hit": True}
    with Image.open(source) as raw:
        image = ImageOps.exif_transpose(raw).convert("L").rotate(rotation, expand=True)
        image.thumbnail((1800, 1800))
        image.save(dest / "source.png")
        width, height = image.size
    tsv = runner.run([TOOLS["tesseract"]["path"], str(dest / "source.png"), "stdout", "--psm", "11", "tsv"], dest / "ocr")
    source_words = []
    for line in tsv.splitlines()[1:]:
        cols = line.split("\t", 11)
        if len(cols) == 12 and cols[0] == "5" and float(cols[10]) >= 60 and cols[11].strip():
            source_words.append({"text": cols[11], "x": (int(cols[6]) + int(cols[8]) / 2) / width,
                                 "y": (int(cols[7]) + int(cols[9]) / 2) / height})
    def unique(words, numeric=False):
        counts = Counter("".join(tokens(w["text"])) for w in words)
        return {"".join(tokens(w["text"])): w for w in words
                if (len("".join(tokens(w["text"]))) >= 4 or
                    (numeric and len("".join(tokens(w["text"])).lstrip("+-")) >= 2 and "".join(tokens(w["text"])).lstrip("+-").isdigit()))
                and counts["".join(tokens(w["text"]))] == 1}
    a, b = unique(source_words), unique(rebuilt["pdf"]["words"])
    common = sorted(a.keys() & b.keys())
    numeric = len(common) < 3 or len(common) / max(1, len(b)) < .45
    if numeric:
        # Answer-key pages have few long words. Use unique printed numbers only
        # as a fallback, so handwritten numbers cannot displace good text anchors.
        a, b = unique(source_words, True), unique(rebuilt["pdf"]["words"], True)
        common = sorted(a.keys() & b.keys())
    distances = sorted(math.hypot(a[w]["x"] - b[w]["x"], a[w]["y"] - b[w]["y"]) for w in common)
    support = len(common) / max(1, len(b))
    p90 = distances[min(len(distances) - 1, math.floor(len(distances) * .9))] if distances else None
    result = {"printed_anchor_support": round(support, 3), "matching_unique_anchors": len(common), "numeric_anchor_fallback": numeric,
            "anchor_position_error_p90": p90, "ocr_words": len(source_words),
            "ok": len(common) >= 3 and support >= .45 and p90 is not None and p90 <= .10,
            "limitation": "OCR anchors check identity and placement, not exact math or handwritten solutions."}
    write_json(cached, {"identity": identity, "preview_sha256": digest((dest / "source.png").read_bytes()), "result": result})
    return {**result, "cache_hit": False}


def gallery(out: Path, report: dict):
    rows = []
    for record in report["pages"]:
        folder = record.get("output_directory")
        images = ""
        if folder:
            rel = Path(folder).relative_to(out).as_posix()
            images = f'<p><a href="{rel}/page.pdf">Rebuilt PDF</a> · <a href="{rel}/page.docx">Editable Word page</a></p><div class="pair"><img src="{rel}/source.png"><img src="{rel}/page.png"></div>'
        rows.append(f'<article><h2>{html.escape(record["id"])}</h2><p>{html.escape(record["status"])} · {html.escape(record.get("template", "blank"))}</p>{images}</article>')
    (out / "index.html").write_text('<!doctype html><meta charset="utf-8"><title>Local page rebuild audit</title><style>body{font:16px system-ui;max-width:1300px;margin:30px auto;background:#f5f4ef}article{background:white;padding:20px;margin:20px 0}h2{font-size:18px}.pair{display:flex;gap:16px}.pair img{width:49%;object-fit:contain;align-self:start}p{color:#555}</style><h1>Local page rebuild audit</h1><p>Original scan on the left; locally rebuilt printed worksheet on the right. Each source file is listed, including repeats and blank backs. Printed content is rebuilt from existing editable layouts. Handwriting is retained in source images and has not been transcribed. These builder runs make no model requests.</p>' + "".join(rows))


def main(argv=None) -> int:
    started = time.monotonic()
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--manifest", type=Path, required=True)
    ap.add_argument("--layouts", type=Path, required=True)
    ap.add_argument("--bindings", type=Path, default=ROOT / "tests/page_rebuild/bindings.json")
    ap.add_argument("--fixes", type=Path, default=ROOT / "tests/page_rebuild/layout_fixes.json")
    ap.add_argument("--layout-reviews", type=Path, help="optional visual reviews pinned to PDF and rendered-image checksums")
    ap.add_argument("--out", type=Path, required=True)
    ap.add_argument("--documentashion", type=Path, default=ROOT.parent / "documentaSHion/target/release/documentashion")
    ap.add_argument("--figure", type=Path, default=ROOT.parent / "documentaSHion/tools/vershion-figure/target/release/vershion-figure")
    ap.add_argument("--workers", type=int, default=2)
    ap.add_argument("--timeout", type=int, default=120)
    args = ap.parse_args(argv)
    if not 1 <= args.workers <= 4:
        ap.error("--workers must be between 1 and 4")
    global TOOLS
    TOOLS = {k: tool(v) for k, v in {"documentashion": args.documentashion, "figure": args.figure,
             "wordcraft": "wordcraft-cli", "pdftoppm": "pdftoppm", "pdftotext": "pdftotext", "tesseract": "tesseract"}.items()}
    runner = Runner(args.timeout)
    out = args.out.resolve()
    out.mkdir(parents=True, exist_ok=True)
    reviews_path = args.layout_reviews or out / "layout-review.json"
    reviews = json.loads(reviews_path.read_text()) if reviews_path.exists() else {}
    source_root = args.manifest.resolve().parent
    bindings = json.loads(args.bindings.read_text())
    fixes = json.loads(args.fixes.read_text())
    manifest = json.loads(args.manifest.read_text())
    identities = [item["file"] for item in manifest["pages"]]
    if len(set(identities)) != len(identities):
        raise ValueError("Manifest contains duplicate page identities")
    report = {"mode": "local-builders-only", "model_requests": 0, "tools": TOOLS,
              "harness_sha256": digest(Path(__file__).read_bytes()), "pages": [],
              "scope": "Editable printed worksheets. Handwritten answers/work are not transcribed by this build stage."}
    jobs = {}
    for item in manifest["pages"]:
        relative = item["file"]
        source = contained(source_root, relative)
        if item.get("sha256") and digest(source.read_bytes()) != item["sha256"]:
            raise ValueError(f"Source checksum changed: {relative}")
        bound = bindings.get(relative)
        if bound is None and relative.startswith("worksheet-corpus/"):
            bound = {"template": source.stem, "rotation": 0, "basis": "named corpus layout"}
        blank = item.get("marked_blank", False)
        record = {"id": relative, "source_sha256": digest(source.read_bytes()), "source": str(source),
                  "status": "pending", "blank": blank, "human_fidelity_review": "pending"}
        report["pages"].append(record)
        if blank:
            sheet = {"page": {"width": 612, "height": 792, "margins": [36, 36, 36, 36]}, "blocks": []}
            record.update(template="blank", rotation=0, binding_basis="project blank-page metadata")
            repairs = []
        elif bound:
            layout = contained(args.layouts, bound["template"] + "-fit.json")
            sheet, applied = apply_layout_fixes(json.loads(layout.read_text()), fixes.get(bound["template"], []))
            sheet, repairs = normalized_sheet(sheet)
            record["reviewed_layout_corrections"] = applied
            record.update(template=bound["template"], rotation=bound["rotation"], binding_basis=bound["basis"],
                          layout_source=str(layout), layout_sha256=digest(layout.read_bytes()))
        else:
            record["status"] = "unmatched_source"
            continue
        key = build_identity(sheet, TOOLS)
        record["normalizations"] = repairs
        record["build_identity"] = key
        jobs.setdefault(key, {"sheet": sheet, "records": []})["records"].append(record)
    write_json(out / "report.json", report)
    def build_one(pair):
        key, job = pair
        try:
            return key, build(job["sheet"], out / "cache", runner), None
        except Exception as e:
            return key, None, str(e)
    with ThreadPoolExecutor(args.workers) as pool:
        for future in as_completed([pool.submit(build_one, pair) for pair in jobs.items()]):
            key, result, error = future.result()
            for record in jobs[key]["records"]:
                if error:
                    record.update(status="build_failed", error=error)
                    continue
                folder = out / "pages" / Path(record["id"]).with_suffix("")
                folder.mkdir(parents=True, exist_ok=True)
                for name in ["page.docx", "page.pdf", "page.png", "sheet.json"]:
                    shutil.copy2(Path(result["directory"]) / name, folder / name)
                record.update(output_directory=str(folder), build_ok=result["ok"], cache_hit=result["cache_hit"],
                              docx_checks=result["docx"], pdf_checks={k: v for k, v in result["pdf"].items() if k != "words"})
                record["figure_checks"] = result["figures"]
                review = reviews.get(key, {})
                record["layout_review"] = review if (review.get("pdf_sha256") == result["hashes"]["page.pdf"] and
                    review.get("render_sha256") == result["hashes"]["page.png"]) else {"status": "pending"}
                record["status"] = "build_checked" if result["ok"] else "build_validation_failed"
                record["_build"] = result
            write_json(out / "report.json", {**report, "pages": [{k: v for k, v in r.items() if k != "_build"} for r in report["pages"]]})
            print(f"builder {key[:10]}: " + (error or f"{len(jobs[key]['records'])} source pages"), flush=True)
    def audit_one(record):
        folder = Path(record["output_directory"])
        audit = source_audit(Path(record["source"]), record["rotation"], folder, record["_build"], runner)
        return record, audit
    with ThreadPoolExecutor(args.workers) as pool:
        futures = {pool.submit(audit_one, r): r for r in report["pages"] if "_build" in r and not r["blank"]}
        for future in as_completed(futures):
            record = futures[future]
            try:
                _, audit = future.result()
                record["source_checks"] = audit
                if record["build_ok"]:
                    record["status"] = ("local_checks_passed_layout_reviewed" if record["layout_review"].get("status") == "passed" else
                                        "local_checks_passed_needs_visual_review") if audit["ok"] else "source_match_needs_review"
            except Exception as e:
                record.update(status="source_audit_failed", error=str(e))
            print(f"source {record['id']}: {record['status']}", flush=True)
            write_json(out / "report.json", {**report, "pages": [{k: v for k, v in r.items() if k != "_build"} for r in report["pages"]]})
    merged = PdfWriter()
    for record in report["pages"]:
        record.pop("_build", None)
        if record.get("output_directory"):
            folder = Path(record["output_directory"])
            if record["blank"]:
                if not (folder / "source.png").exists():
                    with Image.open(record["source"]) as image:
                        image.thumbnail((1200, 1600))
                        image.save(folder / "source.png")
                record["status"] = "blank_rebuilt_layout_reviewed" if record["layout_review"].get("status") == "passed" else "blank_rebuilt_needs_visual_review"
            merged.append(folder / "page.pdf", outline_item=record["id"], import_outline=False)
    if merged.pages:
        with (out / "rebuilt-pages.pdf").open("wb") as f:
            merged.write(f)
    report["summary"] = {"input_pages": len(report["pages"]), "rebuilt_pages": sum(bool(r.get("output_directory")) for r in report["pages"]),
                         "unique_local_builds": len(jobs), "blank_pages": sum(r["blank"] for r in report["pages"]),
                         "build_checks_passed": sum(r.get("build_ok", False) for r in report["pages"]),
                         "builder_cache_hits": sum(r.get("cache_hit", False) for r in report["pages"]),
                         "source_audit_cache_hits": sum(r.get("source_checks", {}).get("cache_hit", False) for r in report["pages"]),
                         "nonblank_source_pages": sum(not r["blank"] for r in report["pages"]),
                         "source_checks_passed": sum(r.get("source_checks", {}).get("ok", False) for r in report["pages"]),
                         "layout_reviews_passed": sum(r.get("layout_review", {}).get("status") == "passed" for r in report["pages"]),
                         "elapsed_seconds": round(time.monotonic() - started, 2),
                         "statuses": dict(Counter(r["status"] for r in report["pages"]))}
    write_json(out / "report.json", report)
    gallery(out, report)
    print(json.dumps(report["summary"]), flush=True)
    # Human review is deliberately never inferred from successful serialization.
    return 0 if all(r.get("build_ok", False) and (r["blank"] or r.get("source_checks", {}).get("ok")) for r in report["pages"]) else 1


if __name__ == "__main__":
    raise SystemExit(main())
