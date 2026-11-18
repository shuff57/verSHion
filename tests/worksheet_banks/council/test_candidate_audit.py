import copy
import importlib.util
import json
import sys
import tempfile
import unittest
from pathlib import Path

TOOLS = Path(__file__).resolve().parents[3] / 'tools'
sys.path.insert(0, str(TOOLS))
spec = importlib.util.spec_from_file_location('candidate_audit', TOOLS / 'worksheet_candidate_audit.py')
h = importlib.util.module_from_spec(spec)
spec.loader.exec_module(h)


class CandidateAuditTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        (self.root / 'source.png').write_bytes(b'pinned source')
        self.case = {'id': 'doc-p1', 'source_id': 'doc', 'file': 'source.png', 'sha256': h.digest(self.root / 'source.png'), 'question_numbers': [1], 'required_figure_numbers': []}
        self.q = {'id': 'q1', 'stem': 'Evaluate $x$', 'kind': {'Short': {'sample': ''}}, 'image': None, 'figure': None, 'points': None}
        self.trace = {'pages': [{'questions': [self.q], 'items': [{'number': 1, 'question_index': 0, 'status': 'saved'}], 'figure_runs': []}]}
        self.result = {'id': 'doc-p1', 'store': str(self.root), 'page_index': 0}
        self.audit_case = {'case_id': 'doc-p1', 'items': {'1': {'source_status': 'source_verified', 'content_status': 'pass', 'figure_visual_status': 'not_required', 'editable_recreation_status': 'not_required', 'reasons': [], 'evidence': {'source_comparison': 'Original x and task preserved.'}}}}
        self.reference = {'cases': [{'case_id': 'doc-p1', 'source_sha256': self.case['sha256'], 'question_expectations': {'1': {'status': 'source_verified'}}}]}
        self.audit = {'reviewer': 'independent council reviewer', 'run_report_sha256': 'run-sha', 'reference_sha256': 'source-ref-sha', 'cases': [self.audit_case]}
        self.save()

    def save(self):
        (self.root / 'banks.json').write_text(json.dumps([{'questions': [self.q]}]))
        (self.root / 'trace.json').write_text(json.dumps(self.trace))
        for key, name in [('bank_sha256', 'banks.json'), ('trace_sha256', 'trace.json')]:
            self.result[key] = h.digest(self.root / name)
            self.audit_case[key] = self.result[key]

    def check(self, audits=None):
        return h.reconcile([self.case], {'cases': [self.result]}, 'run-sha', [('source-ref-sha', self.reference)], [('audit-sha', self.audit)] if audits is None else audits, self.root)

    def test_missing_audit_stays_in_denominator(self):
        report = self.check([])
        self.assertEqual(report['item_denominator'], 1)
        self.assertEqual(report['counts']['bank_item_status'], {'pending': 1})
        self.assertFalse(report['independent_item_and_figure_gates_passed'])

    def test_current_98_percent_gate_and_explicit_historical_threshold_are_distinct(self):
        nums=list(range(1,101));self.case['question_numbers']=nums
        questions=[dict(copy.deepcopy(self.q),id=f'q{n}') for n in nums]
        self.trace['pages'][0].update(questions=questions,items=[{'number':n,'question_index':n-1,'status':'saved'} for n in nums])
        (self.root/'banks.json').write_text(json.dumps([{'questions':questions}]))
        (self.root/'trace.json').write_text(json.dumps(self.trace))
        for key,name in [('bank_sha256','banks.json'),('trace_sha256','trace.json')]:
            self.result[key]=self.audit_case[key]=h.digest(self.root/name)
        original=copy.deepcopy(self.audit_case['items']['1'])
        self.audit_case['items']={str(n):copy.deepcopy(original) for n in nums}
        self.reference['cases'][0]['question_expectations']={str(n):{'status':'source_verified'} for n in nums}
        for n in [99,100]:self.audit_case['items'][str(n)]['content_status']='fail'
        report=self.check()
        self.assertEqual(report['threshold'],.98)
        self.assertTrue(report['independent_item_and_figure_gates_passed'])
        self.audit_case['items']['98']['content_status']='fail'
        self.assertFalse(self.check()['independent_item_and_figure_gates_passed'])
        historical=h.reconcile([self.case],{'cases':[self.result]},'run-sha',[('source-ref-sha',self.reference)],[('audit-sha',self.audit)],self.root,threshold=.95)
        self.assertEqual(historical['threshold'],.95)
        self.assertTrue(historical['independent_item_and_figure_gates_passed'])

    def test_source_uncertainty_cannot_be_promoted_by_a_candidate_pass(self):
        self.reference['cases'][0]['question_expectations']['1']['status'] = 'needs_review'
        self.assertEqual(self.check()['counts']['bank_item_status'], {'pending': 1})

    def test_export_failure_is_separate_from_stored_content(self):
        self.audit_case['items']['1']['worksheet_render_status'] = 'fail'
        report = self.check()
        self.assertEqual(report['counts']['content_status'], {'pass': 1})
        self.assertEqual(report['counts']['bank_item_status'], {'pass': 1})
        self.assertEqual(report['counts']['complete_with_worksheet_status'], {'fail': 1})

    def test_changed_source_or_bank_or_trace_cannot_reuse_an_old_audit(self):
        for name in ['source.png', 'banks.json', 'trace.json']:
            file = self.root / name
            old = file.read_bytes()
            file.write_bytes(old + b' ')
            with self.assertRaises(h.AuditError):
                self.check()
            file.write_bytes(old)

    def test_another_report_or_reference_cannot_reuse_an_old_audit(self):
        for key in ['run_report_sha256', 'reference_sha256']:
            old = self.audit[key]
            self.audit[key] = 'another-sha'
            with self.assertRaises(h.AuditError):
                self.check()
            self.audit[key] = old

    def test_review_must_cover_every_item_with_evidence(self):
        original = copy.deepcopy(self.audit_case['items'])
        self.audit_case['items'] = {}
        with self.assertRaises(h.AuditError):
            self.check()
        self.audit_case['items'] = original
        self.audit_case['items']['1']['evidence'] = {}
        with self.assertRaises(h.AuditError):
            self.check()

    def test_overlapping_reviews_are_rejected(self):
        with self.assertRaises(h.AuditError):
            self.check([('one', self.audit), ('two', self.audit)])

    def test_a_crop_does_not_become_an_editable_recreation(self):
        self.case['required_figure_numbers'] = [1]
        self.audit_case['items']['1'].update(figure_visual_status='pass', editable_recreation_status='pass')
        with self.assertRaises(h.AuditError):
            self.check()
        self.audit_case['items']['1']['editable_recreation_status'] = 'fail'
        report = self.check()
        self.assertEqual(report['counts']['figure_visual_status'], {'pass': 1})
        self.assertEqual(report['counts']['bank_item_status'], {'fail': 1})
        self.assertEqual(report['rates_with_pending_in_denominator']['editable_recreation_status'], 0)

    def test_an_invented_answer_cannot_pass_even_if_review_misses_it(self):
        self.q['kind']['Short']['sample'] = 'model supplied answer'
        self.save()
        self.assertEqual(self.check()['counts']['bank_item_status'], {'fail': 1})

    def test_a_dropped_source_item_cannot_borrow_a_surviving_question(self):
        self.trace['pages'][0]['items'][0].update(status='dropped', question_index=None)
        self.save()
        self.assertEqual(self.check()['counts']['bank_item_status'], {'fail': 1})

    def test_an_unresolved_task_cannot_be_promoted_by_a_review(self):
        self.trace['pages'][0]['items'][0]['status'] = 'saved_figure_only_unresolved'
        self.save()
        self.assertEqual(self.check()['counts']['bank_item_status'], {'pending': 1})

    def test_empty_denominator_is_rejected(self):
        with self.assertRaises(h.AuditError):
            h.reconcile([], {'cases': []}, '', [], [], self.root)


if __name__ == '__main__':
    unittest.main()
