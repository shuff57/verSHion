import copy,importlib.util,json,sys,tempfile,unittest
from pathlib import Path
from PIL import Image

ROOT=Path(__file__).resolve().parents[2]
spec=importlib.util.spec_from_file_location('worksheet_banks',ROOT/'tools/worksheet_banks.py')
h=importlib.util.module_from_spec(spec);spec.loader.exec_module(h)

class RunTimingTests(unittest.TestCase):
    def test_suspend_is_measured_separately_without_hiding_active_budget(self):
        from unittest.mock import patch
        with patch.object(h,'clock_sample',return_value={'active':20,'wall':1040,'boot':140}):
            timing=h.timing_since({'active':10,'wall':1000,'boot':100})
        self.assertEqual((timing['active_seconds'],timing['wall_seconds'],timing['suspend_seconds']),(10,40,30))
        self.assertTrue(timing['host_suspend_detected'])
    def test_wall_clock_jump_does_not_masquerade_as_host_suspend(self):
        from unittest.mock import patch
        with patch.object(h,'clock_sample',return_value={'active':20,'wall':1100,'boot':110}):
            timing=h.timing_since({'active':10,'wall':1000,'boot':100})
        self.assertEqual(timing['suspend_seconds'],0)
        self.assertFalse(timing['host_suspend_detected'])
    def test_unsupported_clock_keeps_suspend_unknown_rather_than_inventing_evidence(self):
        from unittest.mock import patch
        with patch.object(h,'clock_sample',return_value={'active':20,'wall':1040,'boot':None}):
            timing=h.timing_since({'active':10,'wall':1000,'boot':None})
        self.assertIsNone(timing['suspend_seconds'])
        self.assertIsNone(timing['host_suspend_detected'])

class KeyTests(unittest.TestCase):
    def test_true_false_is_an_invented_key_unless_marked_keyless(self):
        self.assertTrue(h.has_invented_key({'kind':{'Tf':{'correct':True}}}))
        self.assertFalse(h.has_invented_key({'kind':{'Tf':{'correct':False}},'explanation':h.KEYLESS_SENTINEL}))
        part={'stem':'(a)','kind':{'Tf':{'correct':False}}}
        self.assertTrue(h.has_invented_key({'kind':{'Multipart':{'parts':[part]}}}))
        self.assertFalse(h.has_invented_key({'kind':{'Multipart':{'parts':[part]}},'explanation':h.KEYLESS_SENTINEL}))

class SourceMapTests(unittest.TestCase):
    def test_declared_dotted_labels_map_by_their_last_part(self):
        page={'items':[{'number':None,'label':'2.15','question_index':0,'status':'saved'},{'number':None,'label':'2.16','question_index':1,'status':'saved'}]};qs=[{'stem':'a'},{'stem':'b'}]
        case={'source_id':'linear-algebra','question_numbers':[15,16],'dotted_labels':True}
        self.assertEqual(h.parsed_source_map(case,page,qs)['by_number'],{15:qs[0],16:qs[1]})
        self.assertEqual(h.parsed_source_map(dict(case,dotted_labels=False),page,qs)['by_number'],{})
        # Holdout v8, Lebl appendix exercises: "A.3.1".
        page={'items':[{'number':None,'label':'A.3.1','question_index':0,'status':'saved'},{'number':None,'label':'A.3.2','question_index':1,'status':'saved'}]}
        self.assertEqual(h.parsed_source_map(dict(case,question_numbers=[1,2]),page,qs)['by_number'],{1:qs[0],2:qs[1]})
    def test_a_leading_continuation_takes_the_one_missing_earlier_number(self):
        # Holdout v6, UC Davis: the page opens mid-problem 1, then prints problems 2 and 3.
        items=[{'number':None,'label':'continued','question_index':0,'status':'saved_continued'},{'number':2,'label':'2','question_index':1,'status':'saved'},{'number':3,'label':'3','question_index':2,'status':'saved'}]
        qs=[{'stem':'x'},{'stem':'y'},{'stem':'z'}];case={'source_id':'la','question_numbers':[1,2,3]}
        got=h.parsed_source_map(case,{'items':items},qs)
        self.assertEqual(got['by_number'],{1:qs[0],2:qs[1],3:qs[2]})
        self.assertEqual(got['unresolved'],[1],'a flagged continuation is never silently complete')
        # Not leading, not labelled continued, or the missing number is later: nothing is guessed.
        self.assertNotIn(1,h.parsed_source_map(case,{'items':[dict(items[0],label=None),*items[1:]]},qs)['by_number'])
        self.assertNotIn(4,h.parsed_source_map(dict(case,question_numbers=[2,3,4]),{'items':items},qs)['by_number'])
    def test_a_declared_single_unnumbered_item_maps_to_its_case_number(self):
        page={'items':[{'number':None,'question_index':0,'status':'saved'}]};qs=[{'stem':'Activity 1.2.9'}]
        case={'source_id':'linear-algebra','question_numbers':[9],'single_unnumbered_item':True}
        self.assertEqual(h.parsed_source_map(case,page,qs)['by_number'],{9:qs[0]})
        self.assertEqual(h.parsed_source_map(dict(case,single_unnumbered_item=False),page,qs)['by_number'],{})

class ExecutionEvidenceTests(unittest.TestCase):
    def test_source_focus_and_app_conflicts_are_counted_without_fabricating_judge_calls(self):
        reply={'original_input_sha256':'original','input_image_sha256':['vision','focus'],
               'image_derivations':[{'protocol':'source_reinspection_focus_v1','source_image':1,'output_image':2,'source_sha256':'vision','png_sha256':'focus'}]}
        check={'generator':'coordinate-grid','preview_png_sha256':'candidate','issues':[{'detail':'shaft overhang missing'}],'unsupported':['raster contact']}
        attempt={'number':1,'spec':{'id':'coordinate-grid'},'png_sha256':'candidate','source_plot_check':check}
        page={'figure_runs':[{'executions':[{'protocol':'figure_execution_v1','observation':{'original_sha256':'original','vision_sha256':'vision','replies':[reply]},'attempts':[attempt]}]}]}
        result=h.figure_execution_evidence(page)
        self.assertEqual((result['observer_detail_panels'],result['observer_focus_details'],result['source_plot_checks'],result['source_plot_conflicts'],result['judge_call_attempts']),(1,1,1,1,0))
        self.assertEqual(result['observer_image_contract_issues'],[]);self.assertEqual(result['source_plot_binding_issues'],[])
        check['preview_png_sha256']='different'
        self.assertEqual(len(h.figure_execution_evidence(page)['source_plot_binding_issues']),1)
        attempt['spec']=None;attempt['png_sha256']=None;attempt['error']='native protocol reply failed'
        unavailable=h.figure_execution_evidence(page)
        self.assertEqual(unavailable['source_plot_binding_issues'],[])
        self.assertEqual(len(unavailable['source_plot_without_final_artifact']),1)
        self.assertEqual(unavailable['judge_call_attempts'],0)
        # A partially missing final artifact still is an actual binding issue.
        attempt['spec']={'id':'coordinate-grid'}
        self.assertEqual(len(h.figure_execution_evidence(page)['source_plot_binding_issues']),1)
        check['preview_png_sha256']=None
        self.assertEqual(len(h.figure_execution_evidence(page)['source_plot_binding_issues']),1)
    def test_source_reinspection_preserves_full_input_then_two_derived_panels(self):
        reply={'attempt':2,'original_input_sha256':'original','input_image_sha256':['vision','upper','lower'],
               'image_derivations':[{'protocol':'source_reinspection_detail_v1','source_image':1,'output_image':2,'source_sha256':'vision','png_sha256':'upper'},
                                    {'protocol':'source_reinspection_detail_v1','source_image':1,'output_image':3,'source_sha256':'vision','png_sha256':'lower'}]}
        observation={'original_sha256':'original','vision_sha256':'vision','replies':[{},reply]}
        page={'figure_runs':[{'question_index':0,'executions':[{'protocol':'figure_execution_v1','observation':observation,'attempts':[]}]}]}
        result=h.figure_execution_evidence(page)
        self.assertEqual((result['observer_call_attempts'],result['observer_detail_panels']),(2,2))
        self.assertEqual(result['observer_image_contract_issues'],[])
        reply['input_image_sha256'][0]='substituted'
        self.assertEqual(len(h.figure_execution_evidence(page)['observer_image_contract_issues']),1)
    def test_optional_source_detail_failure_is_diagnostic_not_a_missing_model_call(self):
        observation={'replies':[{}, {'image_detail_error':'optional budget exceeded','input_image_sha256':['vision']} ]}
        result=h.figure_execution_evidence({'figure_runs':[{'executions':[{'protocol':'figure_execution_v1','observation':observation,'attempts':[]}]}]})
        self.assertEqual((result['observer_call_attempts'],result['observer_detail_panels'],result['observer_detail_fallbacks']),(2,0,1))
        self.assertEqual(result['observer_image_contract_issues'],[])
    def test_judge_pixel_details_keep_the_full_candidate_source_pair_and_derived_hashes_ordered(self):
        judge={'appended_messages':[{'image_sha256':['candidate','source','candidate-detail','source-detail']}],
               'image_derivations':[{'protocol':'judge_edge_detail_v1','source_image':1,'output_image':3,'png_sha256':'candidate-detail'},
                                    {'protocol':'judge_edge_detail_v1','source_image':2,'output_image':4,'png_sha256':'source-detail'}]}
        page={'figure_runs':[{'question_index':0,'executions':[{'protocol':'figure_execution_v1','observation':{'vision_sha256':'source'},
              'attempts':[{'number':1,'png_sha256':'candidate','judge_turn':judge}]}]}]}
        result=h.figure_execution_evidence(page)
        self.assertEqual(result['judge_detail_panels'],2);self.assertEqual(result['judge_image_contract_issues'],[])
        judge['appended_messages'][0]['image_sha256'][:2]=['source','candidate']
        self.assertEqual(len(h.figure_execution_evidence(page)['judge_image_contract_issues']),1)
    def test_all_attempts_are_counted_and_earlier_selected_fallback_is_identified(self):
        spec={'id':'geometry-diagram','params':{'width':500}}
        page={'figure_runs':[{'question_index':0,'spec':spec,'executions':[{'protocol':'figure_execution_v1','selected_attempt':1,'observation':{'replies':[{},{}]},'attempts':[
            {'number':1,'spec':spec,'turns':[{'turn':1,'limit':8},{'turn':2,'limit':8}],'judge_turn':{},'steps':[{'tool':'get_figure'},{'tool':'render_figure'}]},
            {'number':2,'spec':None,'turns':[{'turn':1,'limit':8}],'judge_turn':None,'steps':[]}
        ]}]}]}
        result=h.figure_execution_evidence(page)
        self.assertEqual((result['recorded_attempts'],result['maker_turns'],result['observer_call_attempts'],result['judge_call_attempts'],result['render_tool_steps']),(2,3,2,1,1))
        self.assertEqual(result['selected_spec_mismatches'],[]);self.assertEqual(result['turn_budget_violations'],[])
        page['figure_runs'][0]['executions'][0]['selected_attempt']=2
        self.assertEqual(h.figure_execution_evidence(page)['selected_spec_mismatches'],[0])
    def test_legacy_selected_steps_are_not_labelled_as_total_calls(self):
        result=h.figure_execution_evidence({'figure_runs':[{'steps':[{'tool':'render_figure'}]}]})
        self.assertFalse(result['trace_complete']);self.assertEqual(result['legacy_selected_render_steps'],1);self.assertEqual(result['render_tool_steps'],0)

class FigureFailureTests(unittest.TestCase):
    def test_legacy_real_rejections_report_judge_reasons_not_the_makers_note(self):
        fixtures=json.loads((Path(__file__).parent/'fixtures/figure_rejections.json').read_text())
        for case in fixtures:
            run={'spec':{'id':case['generator']},'note':case['candidate_note'],
                 'review':{'matches':False,'problems':case['problems']}}
            result=h.figure_failure_detail(case['number'],{'image':{'file':'source.webp'}},run)
            self.assertEqual(result['kind'],'comparison_rejected')
            self.assertEqual(result['reviewer_problems'],case['problems'])
            for problem in case['problems']:self.assertIn(problem,result['detail'])
            self.assertNotIn(case['candidate_note'],result['detail'])
            self.assertTrue(result['source_image_retained'])
            self.assertFalse(result['capability_gap_confirmed'])
    def test_typed_source_failure_survives_without_reinterpreting_it_as_a_catalogue_gap(self):
        failure={'kind':'source_read_failed','detail':'Essential edge endpoint is ambiguous.',
                 'generator':None,'generator_registered':None,'reviewer_problems':[]}
        result=h.figure_failure_detail(8,{'image':{'file':'source.webp'}},{'failure':failure})
        self.assertEqual((result['number'],result['kind'],result['detail']),(8,'source_read_failed',failure['detail']))
        self.assertIsNone(result['generator_registered'])
        self.assertFalse(result['capability_gap_confirmed'])
    def test_decline_preserves_attempt_errors_but_does_not_prove_a_missing_generator(self):
        run={'executions':[{'declined':'No faithful method found.','attempts':[
            {'number':1,'error':'must request exactly one tool','spec':None}]}]}
        result=h.figure_failure_detail(4,{'image':{'file':'source.webp'}},run)
        self.assertEqual(result['kind'],'model_declined')
        self.assertEqual(result['attempts'][0]['error'],'must request exactly one tool')
        self.assertFalse(result['capability_gap_confirmed'])

class AcceptanceTests(unittest.TestCase):
    def setUp(self):
        self.tmp=tempfile.TemporaryDirectory();self.addCleanup(self.tmp.cleanup);self.store=Path(self.tmp.name)
        self.case={'question_numbers':[1],'required_figure_numbers':[],'source_id':'arithmetic-fractions'}
        self.q={'id':'q1','stem':'$5/4-3/4$','kind':{'Short':{'sample':''}},'points':None,'image':None,'figure':None}
        self.trace={'pages':[{'questions':[copy.deepcopy(self.q)],'raw_reply':json.dumps({'bank':{'questions':[{'label':'1'}]}}),'figure_runs':[],'warnings':[]}]}
        self.save()
    def save(self):
        q=copy.deepcopy(self.q);q['points']=1.0
        (self.store/'banks.json').write_text(json.dumps([{'questions':[q]}]))
        (self.store/'trace.json').write_text(json.dumps(self.trace))
    def score(self):return h.score_case(self.case,self.store,self.trace)
    def figure(self):
        self.case['required_figure_numbers']=[1]
        (self.store/'media').mkdir();Image.new('RGB',(16,16),'white').save(self.store/'media/f.webp')
        self.q['image']={'file':'media/f.webp','width':16,'height':16}
        self.q['figure']={'id':'coordinate-grid','params':{}}
        self.trace['pages'][0]['questions']=[copy.deepcopy(self.q)]
        self.trace['pages'][0]['figure_runs']=[{'question_index':0,'accepted':True,'spec':copy.deepcopy(self.q['figure']),'review':{'matches':True},'steps':[{'tool':'render_figure'}]}]
        self.save()
    def test_real_bank_round_trip(self):self.assertTrue(self.score()['structural_pass'])
    def test_printed_points_survive_and_only_a_missing_point_value_may_take_the_default(self):
        # Holdout v6, SQA: printed marks on some questions, none on another.
        self.case['question_numbers']=[1,2]
        two=dict(copy.deepcopy(self.q),id='q2')
        self.trace['pages'][0]['questions']=[dict(copy.deepcopy(self.q),points=4.0),two]
        self.trace['pages'][0]['raw_reply']=json.dumps({'bank':{'questions':[{'label':'1'},{'label':'2'}]}})
        (self.store/'banks.json').write_text(json.dumps([{'questions':[dict(copy.deepcopy(self.q),points=4.0),dict(two,points=1.0)]}]))
        (self.store/'trace.json').write_text(json.dumps(self.trace))
        self.assertTrue(self.score()['bank_integrity_ok'])
        # A printed value that changed in the bank is still a persistence failure.
        (self.store/'banks.json').write_text(json.dumps([{'questions':[dict(copy.deepcopy(self.q),points=3.0),dict(two,points=1.0)]}]))
        self.assertFalse(self.score()['bank_integrity_ok'])
    def test_webp_media_is_valid(self):
        self.figure();s=self.score();self.assertTrue(s['bank_integrity_ok']);self.assertEqual(s['figures_recreated'],1)
    def test_a_figure_question_with_picture_choices_needs_each_choice_redrawn(self):
        # Holdout v7, NESA #8: the stem picture and four picture choices.
        self.figure()
        pic={'file':'media/f.webp','width':16,'height':16}
        def with_choices(figures):
            self.q['kind']={'Mc':{'answers':[{'text':'','correct':False,'locked':False,'image':pic,**({'figure':{'id':'pie','params':{}}} if f else {})} for f in figures]}}
            self.trace['pages'][0]['questions']=[copy.deepcopy(self.q)];self.save()
            return self.score()['figures_recreated']
        self.assertEqual(with_choices([True,True,True,True]),1)
        self.assertEqual(with_choices([True,False,True,True]),0,'a choice still only a crop is not recreated')
        # A choice run never stands in for the question's own run.
        self.trace['pages'][0]['figure_runs'].append({'question_index':0,'choice':{'part':None,'choice':0},'accepted':False,'spec':None,'review':{'matches':False},'steps':[]})
        self.assertEqual(with_choices([True,True,True,True]),1)
    def test_plain_crop_does_not_pass_recreation(self):
        self.figure();self.q['figure']=None;self.trace['pages'][0]['questions']=[copy.deepcopy(self.q)];self.save()
        self.assertEqual(self.score()['figures_recreated'],0)
    def test_spec_without_render_tool_does_not_pass(self):
        self.figure();self.trace['pages'][0]['figure_runs'][0]['steps']=[];self.save()
        self.assertEqual(self.score()['figures_recreated'],0)
    def test_failed_review_does_not_pass(self):
        self.figure();self.trace['pages'][0]['figure_runs'][0]['review']['matches']=False;self.save()
        self.assertEqual(self.score()['figures_recreated'],0)
    def test_failed_figure_report_names_printed_item_and_exact_reviewer_problem(self):
        self.figure();run=self.trace['pages'][0]['figure_runs'][0]
        run['accepted']=False;run['review']={'matches':False,'problems':['Extra gridline beyond the last source column.']}
        self.q['figure']=None;self.trace['pages'][0]['questions']=[copy.deepcopy(self.q)];self.save()
        result=self.score();failure=result['figure_failures'][0]
        self.assertEqual(result['figures_recreated'],0)
        self.assertEqual((failure['number'],failure['candidate_generator']),(1,'coordinate-grid'))
        self.assertEqual(failure['reviewer_problems'],run['review']['problems'])
        self.assertTrue(failure['source_image_retained'])
    def test_accepted_figures_have_no_failure_record(self):
        self.figure();self.assertEqual(self.score()['figure_failures'],[])
    def test_changed_spec_does_not_pass(self):
        self.figure();self.trace['pages'][0]['figure_runs'][0]['spec']['params']['xmax']=100;self.save()
        self.assertEqual(self.score()['figures_recreated'],0)
    def test_corrupt_media_fails_integrity(self):
        self.figure();(self.store/'media/f.webp').write_bytes(b'RIFFbrokenWEBP')
        self.assertFalse(self.score()['bank_integrity_ok'])
    def test_shortened_trace_does_not_pass_persistence(self):
        self.trace['pages'][0]['questions']=[];self.save();self.assertFalse(self.score()['bank_integrity_ok'])
    def test_invented_answer_fails_keyless_source(self):
        self.q['kind']['Short']['sample']='1/2';self.trace['pages'][0]['questions']=[copy.deepcopy(self.q)];self.save()
        self.assertFalse(self.score()['source_key_only'])
    def test_wrong_order_fails(self):
        self.trace['pages'][0]['raw_reply']=json.dumps({'bank':{'questions':[{'label':'2'}]}});self.save()
        self.assertFalse(self.score()['order_ok'])
    def test_invented_figure_fails(self):
        self.figure();self.case['required_figure_numbers']=[]
        self.assertFalse(self.score()['structural_pass'])
    def test_independent_axis_contract_overrules_model_approval(self):
        self.figure()
        self.case['figure_expectations']={'1':{'generator_ids':['coordinate-grid'],'params':{'xmax':5}}}
        self.q['figure']['params']['xmax']=4
        self.trace['pages'][0]['questions']=[copy.deepcopy(self.q)]
        self.trace['pages'][0]['figure_runs'][0]['spec']=copy.deepcopy(self.q['figure']);self.save()
        self.assertEqual(self.score()['figures_recreated'],0)
    def test_independent_math_reference_can_certify_source_content(self):
        self.case['question_expectations']={'1':{'math':'5/4-3/4'}}
        self.assertEqual(self.score()['content_certification'],'independently_passed')
    def test_dropped_minus_fails_independent_math_reference(self):
        self.case['question_expectations']={'1':{'math':'5/4-3/4'}}
        self.q['stem']='$5/4+3/4$';self.trace['pages'][0]['questions']=[copy.deepcopy(self.q)];self.save()
        self.assertEqual(self.score()['content_certification'],'independently_failed')
    def test_wrong_denominator_fails_independent_math_reference(self):
        self.case['question_expectations']={'1':{'math':'5/4-3/4'}}
        self.q['stem']='$5/3-3/4$';self.trace['pages'][0]['questions']=[copy.deepcopy(self.q)];self.save()
        self.assertEqual(self.score()['content_certification'],'independently_failed')
    def test_fraction_markup_is_equivalent_without_dropping_math_structure(self):
        self.assertEqual(h.math_tree(r'\frac{5}{4}-\frac{3}{4}'),h.math_tree('(5)/(4)-(3)/(4)'))
        self.assertNotEqual(h.math_tree('(5-3)/4'),h.math_tree('5-3/4'))
        self.assertEqual(h.math_tree('(-4)/5-7/8'),h.math_tree('(-4/5)-7/8'))
    def test_partial_reference_does_not_certify_an_entire_page(self):
        self.case['question_numbers']=[1,2];self.case['question_expectations']={'1':{'math':'5/4-3/4'}}
        self.assertEqual(self.score()['content_certification'],'pending independent mathematical reference/review')
    def test_left_delimiter_is_not_rewritten_as_less_equal(self):
        self.assertEqual(h.math_tree(r'\left(-\frac{4}{5}\right)-\frac{7}{8}'),h.math_tree('(-4/5)-7/8'))
    def test_negative_mixed_number_is_not_multiplication(self):
        self.assertEqual(h.math_tree(r'(-2\frac{2}{5})'),h.math_tree(r'\left(-2\frac{2}{5}\right)'))
        self.assertNotEqual(h.math_tree(r'2\frac{2}{5}'),h.math_tree('2*(2/5)'))
    def test_relation_and_polynomial_structure_is_preserved(self):
        self.assertEqual(h.math_tree(r'y \geq -2x+3'),h.math_tree('y>=-2x+3'))
        self.assertNotEqual(h.math_tree('y>3'),h.math_tree('y>=3'))
        self.assertNotEqual(h.math_tree('2x^2-3=0'),h.math_tree('(2x)^2-3=0'))
    def test_repeating_decimal_marker_is_required(self):
        self.assertEqual(h.math_tree('66.6\u0304%'),h.math_tree(r'66.\overline{6}\%'))
        self.assertNotEqual(h.math_tree('66.6\u0304%'),h.math_tree('66.6%'))
    def test_correct_expression_without_source_instruction_fails(self):
        self.case['question_expectations']={'1':{'math':'5/4-3/4','instruction_any':[['evaluate']]}}
        check=self.score()['item_source_checks']['1']
        self.assertTrue(check['math_matches']);self.assertFalse(check['instruction_matches']);self.assertFalse(check['complete_item_pass'])
    def test_added_axis_letters_fail_a_source_with_unlabelled_axes(self):
        self.figure();self.case['figure_expectations']={'1':{'generator_ids':['coordinate-grid'],'params':{'xlabel':''}}}
        self.q['figure']['params']['xlabel']='x';self.trace['pages'][0]['questions']=[copy.deepcopy(self.q)]
        self.trace['pages'][0]['figure_runs'][0]['spec']=copy.deepcopy(self.q['figure']);self.save()
        self.assertEqual(self.score()['figures_recreated'],0)
    def test_missing_arrow_fails_the_source_axis_contract(self):
        self.figure();self.case['figure_expectations']={'1':{'generator_ids':['coordinate-grid'],'params_one_of':{'x_right_end':['Automatic','Triangle arrow']}}}
        self.q['figure']['params']['x_right_end']='None';self.trace['pages'][0]['questions']=[copy.deepcopy(self.q)]
        self.trace['pages'][0]['figure_runs'][0]['spec']=copy.deepcopy(self.q['figure']);self.save()
        self.assertEqual(self.score()['figures_recreated'],0)
    def test_required_solution_method_is_part_of_a_bank_item(self):
        self.q['stem']='Solve $x^2+2x=0$.';self.trace['pages'][0]['questions']=[copy.deepcopy(self.q)]
        self.case['question_expectations']={'1':{'math':'x^2+2x=0','instruction_any':[['solve'],['quadratic formula']]}};self.save()
        self.assertFalse(self.score()['item_source_checks']['1']['content_pass'])
        self.q['stem']='Solve $x^2+2x=0$ with the quadratic formula.';self.trace['pages'][0]['questions']=[copy.deepcopy(self.q)];self.save()
        self.assertTrue(self.score()['item_source_checks']['1']['content_pass'])
    def test_model_approved_added_title_fails_the_source_contract(self):
        self.figure();self.case['figure_expectations']={'1':{'generator_ids':['coordinate-grid'],'no_visible_title':True}}
        self.q['figure']['params'].update(title_mode='Text',title='y > -2')
        self.trace['pages'][0]['questions']=[copy.deepcopy(self.q)]
        self.trace['pages'][0]['figure_runs'][0]['spec']=copy.deepcopy(self.q['figure']);self.save()
        self.assertEqual(self.score()['figures_recreated'],0)
    def test_model_approved_redraw_cannot_override_unresolved_source_scope(self):
        self.figure();self.trace['pages'][0]['source_box_review']={'declared_complete':True,'applied':False};self.save()
        self.assertEqual(self.score()['figures_recreated'],0)
        self.assertFalse(self.score()['source_box_review_completed'])
        self.trace['pages'][0]['source_box_review']['applied']=True;self.save()
        self.assertEqual(self.score()['figures_recreated'],1)
    def test_per_item_scope_uses_raw_index_not_saved_question_index(self):
        self.figure()
        page=self.trace['pages'][0]
        page['items']=[{'index':0,'number':99,'question_index':None,'status':'dropped'},
                       {'index':1,'number':1,'question_index':0,'status':'saved'}]
        page['source_box_review']={'protocol':'per_figure_scope_v2','applied':False,'confirmed_indices':[1]}
        self.save();self.assertEqual(self.score()['figures_recreated'],1)
        page['source_box_review']['confirmed_indices']=[0]
        self.save();self.assertEqual(self.score()['figures_recreated'],0)
    def test_per_item_scope_does_not_guess_mapping_when_provenance_is_absent(self):
        self.figure();self.trace['pages'][0]['source_box_review']={'applied':True,'confirmed_indices':[0]}
        self.save();self.assertEqual(self.score()['figures_recreated'],0)
    def test_new_scope_protocol_never_falls_back_to_page_approval_without_item_decisions(self):
        self.figure();page=self.trace['pages'][0]
        page['items']=[{'index':0,'number':1,'question_index':0,'status':'saved'}]
        for decisions in [None,True,'0']:
            page['source_box_review']={'protocol':'per_figure_scope_v2','applied':True,'confirmed_indices':decisions}
            self.save();self.assertEqual(self.score()['figures_recreated'],0)
    def test_an_unresolved_source_item_cannot_be_an_editable_figure_pass(self):
        self.figure();self.trace['pages'][0]['items']=[{'number':1,'question_index':0,'status':'saved_figure_ownership_unresolved'}];self.save()
        self.assertEqual(self.score()['figures_recreated'],0)
        self.assertEqual(self.score()['review_unresolved'],[1])
    def test_hidden_or_empty_titles_do_not_invent_visible_content(self):
        self.figure();self.case['figure_expectations']={'1':{'generator_ids':['coordinate-grid'],'no_visible_title':True}}
        for mode,title in [('None','unused hidden text'),('Text','')]:
            self.q['figure']['params'].update(title_mode=mode,title=title)
            self.trace['pages'][0]['questions']=[copy.deepcopy(self.q)]
            self.trace['pages'][0]['figure_runs'][0]['spec']=copy.deepcopy(self.q['figure']);self.save()
            self.assertEqual(self.score()['figures_recreated'],1)
    def test_raw_dropped_items_cannot_count_as_saved_source_coverage(self):
        self.case['question_numbers']=[1,2]
        self.trace['pages'][0]['raw_reply']=json.dumps({'bank':{'questions':[{'label':'1'},{'label':'2','stem':''}]}});self.save()
        self.assertEqual(self.score()['source_questions_found'],1)
    def test_custom_endpoint_validation_checks_actual_loopback_hostname(self):
        for u in ['https://proxy.example/v1','http://localhost:8318/v1','http://127.0.0.1:8318/v1','http://[::1]:8318/v1']:
            self.assertTrue(h.custom_base_allowed(u),u)
        for u in ['http://localhost.evil.example/v1','http://127.0.0.1.evil.example/v1','http://192.0.2.1/v1','https://user:secret@proxy.example/v1','https://proxy.example/v1?token=secret','https://proxy.example:invalid/v1']:
            self.assertFalse(h.custom_base_allowed(u),u)
    def test_math_reference_never_executes_code(self):
        with self.assertRaises(ValueError):h.math_tree("__import__('os').system('false')")
    def test_paths_cannot_escape_store(self):
        with self.assertRaises(ValueError):h.contained(self.store,'../outside.png')
    def test_missing_cases_remain_in_95_percent_denominator(self):
        cases=[{'question_numbers':[1],'required_figure_numbers':[]} for _ in range(45)]
        results=[{'status':'completed','structural_pass':True,'bank_integrity_ok':True,'questions_saved':1} for _ in range(42)]+[{'status':'not_run'}]*3
        s=h.summarize(cases,results,.95);self.assertEqual(s['cases_required_to_pass'],43);self.assertFalse(s['gates']['structural_cases']);self.assertFalse(s['accepted'])
    def test_current_98_percent_gate_keeps_all_45_pages_in_denominator(self):
        self.assertEqual(h.DEFAULT_THRESHOLD,.98)
        cases=[{'question_numbers':[1],'required_figure_numbers':[]} for _ in range(45)]
        results=[{'status':'completed','structural_pass':True,'bank_integrity_ok':True,'questions_saved':1} for _ in range(44)]+[{'status':'not_run'}]
        report=h.summarize(cases,results,h.DEFAULT_THRESHOLD)
        self.assertEqual(report['cases_required_to_pass'],45)
        self.assertFalse(report['gates']['structural_cases'])
        self.assertFalse(report['accepted'])
    def test_large_missing_page_cannot_hide_behind_many_easy_pages(self):
        cases=[{'question_numbers':[1],'required_figure_numbers':[]} for _ in range(44)]+[{'question_numbers':list(range(1,101)),'required_figure_numbers':[]}]
        results=[{'status':'completed','structural_pass':True,'bank_integrity_ok':True,'source_questions_found':1} for _ in range(44)]+[{'status':'timeout'}]
        s=h.summarize(cases,results,.95)
        self.assertTrue(s['gates']['import_completion']);self.assertFalse(s['gates']['question_coverage']);self.assertFalse(s['accepted'])
    def test_rescoring_old_banks_after_a_build_keeps_original_app_provenance(self):
        from unittest.mock import patch
        import contextlib,hashlib,io
        root=self.store;out=root/'run';store=out/'cases/c1/store';store.mkdir(parents=True)
        for name in ['banks.json','trace.json']:(store/name).write_bytes((root/name).read_bytes())
        source=root/'page.png';source.write_bytes(b'pinned source fixture')
        case=dict(self.case,id='c1',file='page.png',sha256=h.sha(source))
        manifest=root/'manifest.json';manifest.write_text(json.dumps({'pages':[{'file':'page.png','sha256':h.sha(source)}]}))
        gold=root/'gold.json';gold.write_text(json.dumps({'cases':[case]}))
        binary=root/'new-binary';binary.write_bytes(b'new build - must not execute')
        original='0'*64;settings={'ai_provider':'openrouter','ai_model':'fixture','ai_output_mode':'json','ai_native_tools':True}
        result=h.score_case(case,store,self.trace);result['cache_id']=h.identity({'source':case['sha256'],'binary':original,'settings':settings,'hint':h.HINT})
        h.write(out/'cases/c1/result.json',result)
        h.write(out/'report.json',{'provider':'openrouter','model':'fixture','binary_sha256':original})
        args=['worksheet_banks','--manifest',str(manifest),'--gold',str(gold),'--binary',str(binary),'--out',str(out),'--provider','openrouter','--model','fixture','--replay']
        with patch.object(sys,'argv',args),patch.object(h.subprocess,'Popen',side_effect=AssertionError('Replay must not execute the app')),contextlib.redirect_stdout(io.StringIO()):
            h.main()
        report=json.loads((out/'report.json').read_text())
        self.assertEqual(report['summary']['completed'],1);self.assertTrue(report['cases'][0]['cache_hit'])
        self.assertEqual(report['binary_sha256'],original);self.assertEqual(report['current_app_binary_sha256'],h.sha(binary))
        self.assertEqual(report['run_kind'],'saved_bank_scoring');self.assertFalse(report['model_calls_allowed'])
    def test_model_or_structure_success_is_not_math_certification(self):
        s=h.summarize([self.case],[self.score()],.95)
        self.assertTrue(s['gates']['structural_cases']);self.assertFalse(s['gates']['independently_certified_content']);self.assertFalse(s['accepted'])

class DocumentAndProvenanceTests(unittest.TestCase):
    def setUp(self):
        self.tmp=tempfile.TemporaryDirectory();self.addCleanup(self.tmp.cleanup);self.store=Path(self.tmp.name)
    def q(self,stem,sample=''):return {'id':'q'+stem,'stem':stem,'kind':{'Short':{'sample':sample}},'points':None,'image':None,'figure':None}
    def save(self,pages):
        questions=[dict(q,points=1.0) for p in pages for q in p['questions']]
        (self.store/'banks.json').write_text(json.dumps([{'questions':questions}]));(self.store/'trace.json').write_text(json.dumps({'pages':pages}))
        return {'pages':pages}
    def item(self,index,number,question_index,status='saved',directions=None):
        return {'index':index,'number':number,'question_index':question_index,'status':status,'directions':directions}
    def test_a_dropped_middle_item_never_moves_a_later_number_onto_the_wrong_question(self):
        case={'question_numbers':[1,2,3],'required_figure_numbers':[],'source_id':'s'}
        page={'questions':[self.q('one'),self.q('three')],'figure_runs':[],'warnings':[],'items':[self.item(0,1,0),self.item(1,2,None,'dropped'),self.item(2,3,1)]}
        s=h.score_case(case,self.store,self.save([page]))
        self.assertEqual(s['source_questions_found'],2);self.assertFalse(s['order_ok']);self.assertFalse(s['count_ok'])
        # The old zip of raw labels with surviving questions would have credited number 2 with the stem "three".
        legacy=dict(page,items=None,raw_reply=json.dumps({'bank':{'questions':[{'label':'1'},{'label':'2'},{'label':'3'}]}}))
        self.assertEqual(h.parsed_source_map(case,legacy,legacy['questions'])['by_number'][2]['stem'],'three')
        self.assertNotIn(2,h.parsed_source_map(case,page,page['questions'])['by_number'])
    def test_figure_runs_follow_the_saved_question_not_the_raw_position(self):
        case={'question_numbers':[1,2],'required_figure_numbers':[2],'source_id':'s'}
        (self.store/'media').mkdir();Image.new('RGB',(16,16),'white').save(self.store/'media/f.webp')
        figure={'id':'coordinate-grid','params':{}};two=dict(self.q('two'),image={'file':'media/f.webp','width':16,'height':16},figure=figure)
        run={'question_index':0,'accepted':True,'spec':figure,'review':{'matches':True},'steps':[{'tool':'render_figure'}]}
        page={'questions':[two],'figure_runs':[run],'warnings':[],'items':[self.item(0,1,None,'dropped'),self.item(1,2,0)]}
        self.assertEqual(h.score_case(case,self.store,self.save([page]))['figures_recreated'],1)
    def test_an_unresolved_figure_only_item_is_saved_but_never_a_complete_source_item(self):
        case={'question_numbers':[1],'required_figure_numbers':[],'source_id':'s','question_expectations':{'1':{'math':'1'}}}
        for status in ['saved_figure_only_unresolved','saved_figure_box_unresolved','unknown_future_review_state']:
            with self.subTest(status=status):
                page={'questions':[self.q('$1$')],'figure_runs':[],'warnings':[],'items':[self.item(0,1,0,status)]}
                s=h.score_case(case,self.store,self.save([page]))
                self.assertEqual(s['review_unresolved'],[1]);self.assertTrue(s['item_source_checks']['1']['source_task_unresolved'])
                self.assertFalse(s['item_source_checks']['1']['complete_item_pass']);self.assertEqual(s['source_questions_found'],1)
    def test_a_document_trace_scores_each_page_against_its_own_slice_of_the_one_bank(self):
        c1={'question_numbers':[1],'required_figure_numbers':[],'source_id':'s','source_pdf_page':1}
        c2={'question_numbers':[2],'required_figure_numbers':[],'source_id':'s','source_pdf_page':2}
        inherited={'text':'Evaluate.','origin':'inherited_from_document_context','from_page':1,'document':'s'}
        pages=[{'document':'s','questions':[self.q('one')],'figure_runs':[],'warnings':[],'items':[self.item(0,1,0)]},
               {'document':'s','questions':[self.q('two')],'figure_runs':[],'warnings':[],'items':[self.item(0,2,0,directions=inherited)],'context':{'document':'s','directions':[{'text':'Evaluate.','page':1,'labels':['1']}]}}]
        trace=self.save(pages)
        a=h.score_case(c1,self.store,trace,page_index=0);b=h.score_case(c2,self.store,trace,page_index=1)
        self.assertTrue(a['structural_pass'] and b['structural_pass'])
        self.assertEqual(b['inherited_direction_items'],[2]);self.assertEqual(b['context_directions_supplied'],1);self.assertEqual(a['context_directions_supplied'],0)
        # The whole bank must be accounted for by the trace, or a trimmed trace could hide extra questions.
        (self.store/'banks.json').write_text(json.dumps([{'questions':[dict(self.q('one'),points=1.0),dict(self.q('two'),points=1.0),dict(self.q('extra'),points=1.0)]}]))
        self.assertFalse(h.score_case(c1,self.store,trace,page_index=0)['bank_integrity_ok'])
    def test_documents_group_by_source_in_original_page_order_and_never_mix(self):
        cases=[{'id':'b2','source_id':'b','source_pdf_page':2},{'id':'a1','source_id':'a','source_pdf_page':1},{'id':'b1','source_id':'b','source_pdf_page':1}]
        groups=h.document_groups(cases)
        self.assertEqual(list(groups),['a','b']);self.assertEqual([c['id'] for c in groups['b']],['b1','b2'])
        with self.assertRaises(ValueError):h.document_groups([{'id':'x','source_id':'a','source_pdf_page':1},{'id':'y','source_id':'a','source_pdf_page':1}])
    def test_a_failed_page_in_a_document_keeps_the_other_pages_and_stays_in_the_denominator(self):
        from unittest.mock import patch
        import contextlib,io
        root=self.store;out=root/'run';store=out/'documents/s/store';store.mkdir(parents=True)
        c1={'id':'s-p1','file':'p1.png','question_numbers':[1],'required_figure_numbers':[],'source_id':'s','source_pdf_page':1}
        c2={'id':'s-p2','file':'p2.png','question_numbers':[2],'required_figure_numbers':[],'source_id':'s','source_pdf_page':2}
        pages=[{'document':'s','questions':[self.q('one')],'figure_runs':[],'warnings':[],'items':[self.item(0,1,0)]},{'document':'s','page':2,'questions':[],'warnings':[],'error':'timed out'}]
        trace=self.save(pages);(store/'banks.json').write_bytes((root/'banks.json').read_bytes());(store/'trace.json').write_bytes((root/'trace.json').read_bytes())
        for name in ['p1.png','p2.png']:(root/name).write_bytes(name.encode())
        for c in (c1,c2):c['sha256']=h.sha(root/c['file'])
        manifest=root/'manifest.json';manifest.write_text(json.dumps({'pages':[{'file':c['file'],'sha256':c['sha256']} for c in (c1,c2)]}))
        gold=root/'gold.json';gold.write_text(json.dumps({'cases':[c1,c2]}))
        binary=root/'bin';binary.write_bytes(b'x');original='0'*64
        settings={'ai_provider':'openrouter','ai_model':'fixture','ai_output_mode':'json','ai_native_tools':True}
        cache=h.identity({'source':[c1['sha256'],c2['sha256']],'binary':original,'settings':settings,'hint':h.HINT,'mode':'document'})
        h.write(store.parent/'result.json',{'status':'completed','cache_id':cache,'bank_sha256':h.sha(store/'banks.json'),'trace_sha256':h.sha(store/'trace.json')})
        h.write(out/'report.json',{'provider':'openrouter','model':'fixture','binary_sha256':original})
        args=['wb','--manifest',str(manifest),'--gold',str(gold),'--binary',str(binary),'--out',str(out),'--provider','openrouter','--model','fixture','--replay','--mode','document']
        with patch.object(sys,'argv',args),patch.object(h.subprocess,'Popen',side_effect=AssertionError('Replay must not execute the app')),contextlib.redirect_stdout(io.StringIO()):
            h.main()
        report=json.loads((out/'report.json').read_text());rows={r['id']:r for r in report['cases']}
        self.assertEqual(report['import_mode'],'document');self.assertEqual(rows['s-p1']['status'],'completed');self.assertEqual(rows['s-p2']['status'],'failed')
        self.assertEqual(report['summary']['completed'],1);self.assertEqual(report['summary']['cases'],2);self.assertFalse(report['summary']['accepted'])

if __name__=='__main__':unittest.main()


class CustomProviderCliTests(unittest.TestCase):
    def run_cli(self,*args):
        import subprocess
        return subprocess.run([sys.executable,str(ROOT/'tools/worksheet_banks.py'),'--manifest','/nonexistent/manifest.json','--out','/tmp/never',*args],capture_output=True,text=True)
    def test_custom_requires_explicit_remote_model_and_secure_base(self):
        self.assertIn('--allow-remote',self.run_cli('--provider','custom','--base-url','https://x.example/v1').stderr)
        self.assertIn('--base-url',self.run_cli('--provider','custom','--allow-remote','--model','m').stderr)
        self.assertIn('--base-url',self.run_cli('--provider','custom','--allow-remote','--model','m','--base-url','http://evil.example/v1').stderr)


class AppLinearMathTests(unittest.TestCase):
    """The app's prompt asks for linear math; the reference comparison must read it structurally."""
    def test_linear_mixed_numbers_and_fractions_match_latex_references(self):
        T=h.math_tree
        self.assertEqual(T('(-3 (3)/(5)) - 4 (2)/(5)'),T(r'\left(-3\frac{3}{5}\right) - 4\frac{2}{5}'))
        self.assertEqual(T('(-(4)/(3)) - (-(3)/(2))'),T('(-4/3)-(-3/2)'))
    def test_unicode_relations_and_dashes(self):
        self.assertEqual(T:=h.math_tree('5x + 3y ≥ –9'),h.math_tree('5x+3y>=-9'))
    def test_wrong_linear_math_still_fails(self):
        self.assertNotEqual(h.math_tree('2 (2)/(5)'),h.math_tree(r'3\frac{2}{5}'))
        self.assertNotEqual(h.math_tree('2 (2)/(5)'),h.math_tree('2*(2)/(5)'))
        self.assertNotEqual(h.math_tree('y ≥ 2x'),h.math_tree('y>2x'))
