"""Regression tests for false passes, stale caches and stalled child builders."""
import importlib.util
import json
from pathlib import Path
import sys
import tempfile
import unittest
import zipfile
from unittest.mock import patch

from pypdf import PdfWriter

MODULE = Path(__file__).resolve().parents[2] / "tools/page_rebuild.py"
spec = importlib.util.spec_from_file_location("page_rebuild", MODULE)
h = importlib.util.module_from_spec(spec)
spec.loader.exec_module(h)
bspec = importlib.util.spec_from_file_location("battery", MODULE.parents[1] / "tests/import_battery/battery.py")
b = importlib.util.module_from_spec(bspec)
bspec.loader.exec_module(b)


class HarnessTests(unittest.TestCase):
    def test_failed_blank_conversion_is_not_a_successful_control(self):
        checks = b.score_control("blank", {"convert_ok":False, "parse_ok":False, "questions":[]})
        self.assertFalse(checks[0][2])

    def test_legacy_battery_keeps_signs_and_requires_remote_opt_in(self):
        self.assertNotEqual(b.norm("x+2=7"), b.norm("x-2=7"))
        with self.assertRaises(ValueError):
            b.check_provider("openrouter", "https://openrouter.ai/api/v1", "model", False)
        with self.assertRaises(ValueError):
            b.check_provider("ollama", "http://localhost:11434", "model:cloud", False)
        with self.assertRaises(ValueError):
            b.check_provider("ollama", "https://remote.example", "local-name", False)

    def test_an_ollama_alias_cannot_hide_a_cloud_model(self):
        import io
        with patch.object(b, "urlopen", return_value=io.BytesIO(b'{"remote_host":"https://ollama.com"}')):
            with self.assertRaises(ValueError):
                b.check_provider("ollama", "http://localhost:11434", "looks-local", False)

    def test_legacy_local_profile_cannot_inherit_real_store_or_provider_keys(self):
        with patch.object(b, "RUN_STORE", Path("/scratch/profile")), patch.object(b, "RUN_ENV", {}), patch.object(b, "REMOTE_ALLOWED", False):
            with patch.dict(b.os.environ, {"VERSHION_STORE_DIR":"/real/store", "VERSHION_SEED":"1", "OPENROUTER_API_KEY":"test-only"}):
                with patch.object(b.subprocess, "run") as run:
                    b.sh(["fake"])
                    env = run.call_args.kwargs["env"]
                    self.assertEqual(env["VERSHION_STORE_DIR"], "/scratch/profile")
                    self.assertNotIn("VERSHION_SEED", env)
                    self.assertNotIn("OPENROUTER_API_KEY", env)

    def test_signs_are_not_discarded(self):
        self.assertNotEqual(h.tokens("x+2=7"), h.tokens("x-2=7"))
        self.assertNotEqual(h.tokens("-17"), h.tokens("17"))
        self.assertEqual(h.tokens("x−2"), h.tokens("x-2"))

    def test_cache_identity_changes_when_layout_or_builder_changes(self):
        sheet = {"blocks": [{"type": "p", "runs": [{"math": "x+1=2"}]}]}
        tools = {"wordcraft": {"sha256": "version-a"}}
        key = h.build_identity(sheet, tools)
        self.assertEqual(key, h.build_identity(json.loads(json.dumps(sheet)), tools))
        changed = json.loads(json.dumps(sheet))
        changed["blocks"][0]["runs"][0]["math"] = "x-1=2"
        self.assertNotEqual(key, h.build_identity(changed, tools))
        self.assertNotEqual(key, h.build_identity(sheet, {"wordcraft": {"sha256": "version-b"}}))

    def test_truncated_or_corrupt_cache_entries_are_rebuilt(self):
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            (out / "validation.json").write_text('{"hashes":{}}')
            self.assertIsNone(h.cached_build(out))
            (out / "validation.json").write_text("truncated")
            self.assertIsNone(h.cached_build(out))
            for name in ["page.docx", "page.pdf", "page.png", "sheet.json"]:
                (out / name).write_text(name)
            hashes = {name: h.digest((out / name).read_bytes()) for name in ["page.docx", "page.pdf", "page.png"]}
            (out / "validation.json").write_text(json.dumps({"hashes": hashes}))
            self.assertIsNotNone(h.cached_build(out))
            (out / "page.pdf").write_text("corrupt")
            self.assertIsNone(h.cached_build(out))

    def test_manifest_cannot_read_outside_source_root(self):
        with tempfile.TemporaryDirectory() as tmp:
            with self.assertRaises(ValueError):
                h.contained(Path(tmp), "../outside.png")

    def test_repairs_are_narrow_and_recorded(self):
        sheet = {"blocks": [{"type": "p", "runs": [{"math": r"x=\\frac{1}{4}"}, {"math": r"x=\frac{1}{4}"}]}]}
        fixed, repairs = h.normalized_sheet(sheet)
        self.assertEqual(len(repairs), 1)
        self.assertEqual(fixed["blocks"][0]["runs"][0]["math"], r"x=\frac{1}{4}")
        self.assertEqual(sheet["blocks"][0]["runs"][0]["math"], r"x=\\frac{1}{4}")

    def test_coordinate_grid_rows_use_the_builders_delimiter(self):
        sheet = {"blocks": [{"type": "figure", "vershion": "coordinate-grid",
                            "params": {"equations": "y=x+1\ny=-x+2"}}]}
        fixed, repairs = h.normalized_sheet(sheet)
        self.assertEqual(fixed["blocks"][0]["params"]["equations"], "y=x+1; y=-x+2")
        self.assertEqual(len(repairs), 1)

    def test_reviewed_fix_refuses_an_unexpected_source_layout(self):
        sheet = {"blocks": [{"type": "figure", "params": {"equations": "y=7x"}}]}
        patch = {"figure": 0, "parameter": "equations", "from": "", "to": "y=x"}
        with self.assertRaises(ValueError):
            h.apply_layout_fixes(sheet, [patch])

    def test_missing_given_graph_lines_fail_even_when_an_image_exists(self):
        class FakeRunner:
            def run(self, *args):
                return '<svg xmlns="http://www.w3.org/2000/svg"><line x1="0" x2="10"/></svg>'
        h.TOOLS = {"figure": {"path": "/tmp/vershion-figure"}}
        sheet = {"blocks": [{"type": "figure", "vershion": "coordinate-grid",
                            "params": {"equations": ""}, "alt": "Grid with two parallel lines"}]}
        self.assertFalse(h.inspect_figures(sheet, Path("/unused"), FakeRunner())["ok"])

    def test_dropped_minus_in_native_word_equation_fails(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "bad.docx"
            document = '<w:document xmlns:w="' + h.NS["w"] + '" xmlns:m="' + h.NS["m"] + '"><w:body><w:p><m:oMath><m:r><m:t>x+2=7</m:t></m:r></m:oMath></w:p></w:body></w:document>'
            with zipfile.ZipFile(path, "w") as z:
                z.writestr("word/document.xml", document)
            result = h.inspect_docx(path, {"blocks": [{"type": "p", "runs": [{"math": "x-2=7"}]}]})
            self.assertFalse(result["ok"])
            self.assertEqual(len(result["math_errors"]), 1)

    def test_reordered_questions_cannot_pass_by_word_counts(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "reordered.docx"
            document = '<w:document xmlns:w="' + h.NS["w"] + '"><w:body><w:p><w:r><w:t>Second question. First question.</w:t></w:r></w:p></w:body></w:document>'
            with zipfile.ZipFile(path, "w") as z:
                z.writestr("word/document.xml", document)
            sheet = {"blocks": [{"type": "p", "runs": [{"text": "First question. Second question."}]}]}
            result = h.inspect_docx(path, sheet)
            self.assertFalse(result["ok"])
            self.assertFalse(result["text_order_ok"])
            self.assertEqual(result["missing_text_tokens"], [])

    def test_fraction_written_as_plain_text_fails(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "bad.docx"
            document = '<w:document xmlns:w="' + h.NS["w"] + '" xmlns:m="' + h.NS["m"] + '"><w:body><w:p><m:oMath><m:r><m:t>x2</m:t></m:r></m:oMath></w:p></w:body></w:document>'
            with zipfile.ZipFile(path, "w") as z:
                z.writestr("word/document.xml", document)
            result = h.inspect_docx(path, {"blocks": [{"type": "p", "runs": [{"math": r"\frac{x}{2}"}]}]})
            self.assertFalse(result["ok"])

    def test_word_delimiter_properties_preserve_parentheses(self):
        import xml.etree.ElementTree as ET
        node = ET.fromstring('<m:oMath xmlns:m="' + h.NS["m"] + '"><m:r><m:t>t</m:t></m:r><m:d><m:dPr><m:begChr m:val="("/><m:endChr m:val=")"/></m:dPr><m:e><m:r><m:t>n</m:t></m:r></m:e></m:d></m:oMath>')
        self.assertEqual(h.word_math_glyphs(node), "t(n)")

    def test_overflow_page_is_not_hidden_by_rendering_only_page_one(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "overflow.pdf"
            writer = PdfWriter()
            writer.add_blank_page(612, 792)
            writer.add_blank_page(612, 792)
            with path.open("wb") as f:
                writer.write(f)
            result = h.inspect_pdf(path, '<doc><page width="612" height="792"/></doc>')
            self.assertFalse(result["ok"])
            self.assertEqual(result["page_count"], 2)

    def test_clipped_text_fails(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "clipped.pdf"
            writer = PdfWriter()
            writer.add_blank_page(612, 792)
            with path.open("wb") as f:
                writer.write(f)
            xml = '<doc><page width="612" height="792"><word xMin="600" yMin="20" xMax="650" yMax="30">clipped</word></page></doc>'
            self.assertFalse(h.inspect_pdf(path, xml)["ok"])

    def test_a_table_rule_through_text_fails(self):
        from pypdf.generic import DecodedStreamObject, NameObject, DictionaryObject
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "overlap.pdf"
            writer = PdfWriter()
            page = writer.add_blank_page(612, 792)
            font = DictionaryObject({NameObject("/Type"): NameObject("/Font"),
                                     NameObject("/Subtype"): NameObject("/Type1"),
                                     NameObject("/BaseFont"): NameObject("/Helvetica")})
            page[NameObject("/Resources")] = DictionaryObject({NameObject("/Font"): DictionaryObject({
                NameObject("/F1"): writer._add_object(font)})})
            stream = DecodedStreamObject()
            stream.set_data(b"BT /F1 12 Tf 30 390 Td (heading) Tj ET 0 0 0 RG 0 400 m 612 400 l S")
            page[NameObject("/Contents")] = writer._add_object(stream)
            with path.open("wb") as f:
                writer.write(f)
            xml = '<doc><page width="612" height="792"><word xMin="30" yMin="385" xMax="90" yMax="400">heading</word></page></doc>'
            result = h.inspect_pdf(path, xml)
            self.assertFalse(result["ok"])
            self.assertEqual(len(result["text_rule_overlaps"]), 1)

    def test_an_invisible_answer_leader_fails(self):
        sheet = {"size": 12, "blocks": [{"type": "p",
                 "runs": [{"text":"The slope and y-intercept"}, {"tab":True}, {"tab":True}, {"tab":True}, {"tab":True}],
                 "tabs": [{"pos":n,"leader":"underscore"} for n in [100,200,300,400]]}]}
        pdf = {"words":[{"text":"slope","box":[20,100,50,112]},{"text":"y-intercept","box":[60,100,100,112]}],
               "horizontal_rules":[[110,140,112],[210,240,112],[310,340,112]]}
        result = h.inspect_answer_leaders(sheet, pdf)
        self.assertFalse(result["ok"])
        self.assertEqual(result["paragraphs"][0]["visible_answer_lines"], 3)
        pdf["words"].append({"text":"______","box":[410,100,450,112]})
        self.assertTrue(h.inspect_answer_leaders(sheet, pdf)["ok"])

    def test_builder_never_invokes_a_model_command(self):
        runner = h.Runner()
        for command in [["curl", "https://openrouter.ai"], ["documentashion", "ws", "rebuild", "page.png", "out.docx"],
                        ["documentashion", "ws", "extract", "page.png", "out.json"]]:
            with self.assertRaises(ValueError):
                runner.run(command, Path("unused"))

    def test_large_stdout_and_stderr_are_drained_without_deadlock(self):
        with tempfile.TemporaryDirectory() as tmp:
            executable = Path(tmp) / "documentashion"
            executable.write_text(f"#!{sys.executable}\nimport sys\nsys.stdout.write('a'*200000)\nsys.stderr.write('b'*200000)\n")
            executable.chmod(0o700)
            h.TOOLS = {"figure": {"path": "/bin/false"}}
            output = h.Runner(5).run([str(executable), "ws", "build"], Path(tmp) / "run")
            self.assertEqual(len(output), 200000)
            self.assertEqual((Path(tmp) / "run.stderr.log").stat().st_size, 200000)

    def test_timeout_kills_descendants(self):
        with tempfile.TemporaryDirectory() as tmp:
            executable = Path(tmp) / "documentashion"
            executable.write_text(f"#!{sys.executable}\nimport subprocess,time,sys\np=subprocess.Popen([sys.executable,'-c','import time; time.sleep(60)'])\nprint(p.pid,flush=True)\ntime.sleep(60)\n")
            executable.chmod(0o700)
            h.TOOLS = {"figure": {"path": "/bin/false"}}
            with self.assertRaises(RuntimeError):
                h.Runner(.2).run([str(executable), "ws", "build"], Path(tmp) / "run")
            import time
            time.sleep(.1)
            child_pid = int((Path(tmp) / "run.stdout.log").read_text().strip())
            stat = Path(f"/proc/{child_pid}/stat")
            self.assertTrue(not stat.exists() or stat.read_text().split()[2] == "Z")
            leftovers = []
            for proc in Path("/proc").glob("[0-9]*"):
                try:
                    if str(executable).encode() in (proc / "cmdline").read_bytes():
                        leftovers.append(proc.name)
                except OSError:
                    pass
            self.assertEqual(leftovers, [])


if __name__ == "__main__":
    unittest.main()
