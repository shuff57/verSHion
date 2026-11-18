import copy,importlib.util,json,tempfile,unittest
from pathlib import Path

ROOT=Path(__file__).resolve().parents[3]
spec=importlib.util.spec_from_file_location('reference_intake',ROOT/'tools/worksheet_reference_intake.py')
h=importlib.util.module_from_spec(spec);spec.loader.exec_module(h)

class ReferenceIntakeTests(unittest.TestCase):
    def setUp(self):
        self.tmp=tempfile.TemporaryDirectory();self.addCleanup(self.tmp.cleanup);self.root=Path(self.tmp.name)
        self.png=self.root/'page.png';self.png.write_bytes(b'pinned source bytes')
        self.pdf=self.root/'source.pdf';self.pdf.write_bytes(b'pinned original PDF bytes')
        (self.root/'manifest.json').write_text(json.dumps({'pages':[{'source_id':'source','pdf_page':1}]}))
        self.assignment={'items':[{'case_id':'source-p1','source_id':'source','source_png':str(self.png),'source_pdf':str(self.pdf),'source_sha256':h.digest(self.png),'pdf_page':1,'question_numbers':[1],'required_figure_numbers':[]}]}
        self.reference={'reviewer':'independent source author','source_first':True,'cases':[{'case_id':'source-p1','source_id':'source','source_sha256':h.digest(self.png),'source_pdf':str(self.pdf),'source_pdf_sha256':h.digest(self.pdf),'pdf_page':1,'document_instructions':[{'text':'Evaluate.','from_pdf_page':1,'applies_to':[1]}],'question_expectations':{'1':{'source_body':'x+1','instruction':'Evaluate.','maths':['x+1'],'source_figure':None,'status':'source_verified','from_pdf_page':1}}}]}
    def check(self):return h.validate_reference(self.assignment,self.reference,self.root)
    def test_valid_intake_is_not_a_candidate_pass(self):
        r=self.check();self.assertEqual(r['questions'],1);self.assertEqual(r['candidate_banks_read'],0)
        self.assertEqual(r['candidate_acceptance'],'not evaluated');self.assertIsNone(r['candidate_pass_rate'])
    def test_changed_source_is_rejected(self):
        self.png.write_bytes(b'changed')
        with self.assertRaises(h.ReferenceError):self.check()
    def test_changed_original_pdf_is_rejected(self):
        self.pdf.write_bytes(b'changed')
        with self.assertRaises(h.ReferenceError):self.check()
    def test_missing_question_cannot_hide_coverage(self):
        self.reference['cases'][0]['question_expectations']={}
        with self.assertRaises(h.ReferenceError):self.check()
    def test_duplicate_case_is_rejected(self):
        self.reference['cases'].append(copy.deepcopy(self.reference['cases'][0]))
        with self.assertRaises(h.ReferenceError):self.check()
    def test_no_source_first_declaration_is_rejected(self):
        self.reference['source_first']=False
        with self.assertRaises(h.ReferenceError):self.check()
    def test_source_uncertainty_remains_pending(self):
        self.reference['cases'][0]['question_expectations']['1']['status']='needs_review'
        r=self.check();self.assertEqual(r['author_uncertain_records'],1);self.assertEqual(r['author_source_verified_records'],0)
    def test_empty_problem_without_figure_is_not_a_reference(self):
        self.reference['cases'][0]['question_expectations']['1']['source_body']=''
        with self.assertRaises(h.ReferenceError):self.check()
    def test_source_figure_can_supply_the_whole_body(self):
        q=self.reference['cases'][0]['question_expectations']['1'];q.update(source_body='',source_figure={'kind':'blank_grid'})
        self.assignment['items'][0]['required_figure_numbers']=[1]
        self.assertEqual(self.check()['source_figures'],1)
    def test_inventory_disagreements_are_explicit(self):
        self.assignment['items'][0]['required_figure_numbers']=[1]
        r=self.check();self.assertEqual(len(r['inventory_disagreements']),1);self.assertEqual(r['candidate_acceptance'],'not evaluated')
    def test_solution_fields_are_not_question_source_facts(self):
        self.reference['cases'][0]['question_expectations']['1']['worked_solution']='x=2'
        with self.assertRaises(h.ReferenceError):self.check()
    def test_key_page_cannot_supply_task_directions(self):
        self.reference['cases'][0]['document_instructions'][0]['from_pdf_page']=2
        with self.assertRaises(h.ReferenceError):self.check()
    def test_source_path_cannot_escape_corpus(self):
        self.reference['cases'][0]['source_pdf']=str(self.root.parent/'external.pdf')
        with self.assertRaises(h.ReferenceError):self.check()
    def test_worksheet_section_can_span_multiple_assigned_pages(self):
        png2=self.root/'page2.png';png2.write_bytes(b'page two')
        (self.root/'manifest.json').write_text(json.dumps({'pages':[{'source_id':'source','pdf_page':1},{'source_id':'source','pdf_page':2}]}))
        a2=copy.deepcopy(self.assignment['items'][0]);a2.update(case_id='source-p2',source_png=str(png2),source_sha256=h.digest(png2),pdf_page=2,question_numbers=[2]);self.assignment['items'].append(a2)
        c2=copy.deepcopy(self.reference['cases'][0]);c2.update(case_id='source-p2',source_sha256=h.digest(png2),pdf_page=2)
        c2['question_expectations']={'2':dict(c2['question_expectations']['1'],from_pdf_page=2)}
        self.reference['cases'].append(c2)
        for case in self.reference['cases']:case['document_instructions'][0]['applies_to']=[1,2]
        r=self.check();self.assertEqual(r['questions'],2)
    def test_heading_context_image_hash_is_also_verified(self):
        (self.root/'manifest.json').write_text(json.dumps({'pages':[{'source_id':'source','pdf_page':1,'file':'page.png','sha256':h.digest(self.png)}]}))
        instruction=self.reference['cases'][0]['document_instructions'][0]
        instruction.update(heading_source_png=str(self.png),heading_source_sha256='0'*64)
        with self.assertRaises(h.ReferenceError):self.check()
        instruction['heading_source_sha256']=h.digest(self.png)
        self.assertEqual(self.check()['pages'],1)
    def test_future_heading_cannot_be_assigned_to_current_problem(self):
        (self.root/'manifest.json').write_text(json.dumps({'pages':[{'source_id':'source','pdf_page':1},{'source_id':'source','pdf_page':2}]}))
        self.reference['cases'][0]['document_instructions'][0]['from_pdf_page']=2
        with self.assertRaises(h.ReferenceError):self.check()

if __name__=='__main__':unittest.main()
