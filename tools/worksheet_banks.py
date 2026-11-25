#!/usr/bin/env python3
"""Real worksheet-image -> app import -> persisted bank acceptance evaluation.

No source answer overlays are submitted. Missing/unrun/failed cases stay in the
fixed denominator. Structural and figure metrics never claim mathematical truth.
"""
from __future__ import annotations
import argparse
from concurrent.futures import ThreadPoolExecutor, as_completed
import ast,hashlib,json,math,os,re,signal,subprocess,time,threading
from pathlib import Path
from urllib.parse import urlsplit
from PIL import Image

ROOT=Path(__file__).resolve().parents[1]
DEFAULT_THRESHOLD=.98

def clock_sample():
    """BOOTTIME includes Linux suspend; monotonic subprocess budgets do not."""
    boot=None
    if hasattr(time,'CLOCK_BOOTTIME'):
        try:boot=time.clock_gettime(time.CLOCK_BOOTTIME)
        except (OSError,ValueError):pass
    return {'active':time.monotonic(),'wall':time.time(),'boot':boot}

def timing_since(start):
    end=clock_sample();active=max(0,end['active']-start['active']);wall=max(0,end['wall']-start['wall'])
    suspend=None if start['boot'] is None or end['boot'] is None else max(0,(end['boot']-start['boot'])-active)
    return {'active_seconds':round(active,2),'wall_seconds':round(wall,2),
            'suspend_seconds':None if suspend is None else round(suspend,2),
            'host_suspend_detected':None if suspend is None else suspend>1,
            'suspend_detection':'linux_boottime_minus_monotonic' if suspend is not None else 'unavailable',
            'timeout_basis':'monotonic_active_time',
            'note':'Wall time may include clock adjustments; suspend detection requires BOOTTIME, not just a wall-clock gap.'}
HINT=('Import clean printed questions only, ignoring handwriting. Use exactly one bank question per printed numbered problem, '
      'in printed numerical order; keep subparts together as Multipart. Preserve the printed question number in label. '
      'For an unnumbered worksheet keep its main problem and lettered subparts together as one Multipart. '
      'The page contains no answer key: do not solve or invent correct/suggested answers. Preserve every mathematical sign, '
      'fraction, exponent, table and label. Blank plotting grids are source figures: include their tight boxes and recreate '
      'their axes without solving or adding curves. Include instructions needed to understand each standalone bank question.')

def sha(path):return hashlib.sha256(Path(path).read_bytes()).hexdigest()
def identity(value):return hashlib.sha256(json.dumps(value,sort_keys=True).encode()).hexdigest()
def write(path,value):
    path=Path(path);path.parent.mkdir(parents=True,exist_ok=True)
    tmp=path.with_suffix(path.suffix+'.tmp');tmp.write_text(json.dumps(value,indent=2,ensure_ascii=False)+'\n');tmp.replace(path)
def contained(root,relative):
    result=(root/relative).resolve()
    if not result.is_relative_to(root.resolve()):raise ValueError('Source path escapes corpus')
    return result

def custom_base_allowed(base):
    try:
        url=urlsplit(base or '')
        port=url.port
        return bool(url.hostname) and not (url.username or url.password or url.query or url.fragment) and (url.scheme=='https' or (url.scheme=='http' and url.hostname in {'localhost','127.0.0.1','::1'}))
    except ValueError:return False

def raw_questions(page):
    raw=page.get('raw_reply','').strip()
    if raw.startswith('```'):raw=raw.split('\n',1)[-1].rsplit('```',1)[0]
    try:return json.loads(raw)['bank']['questions']
    except (ValueError,KeyError,TypeError):return []

def source_number(raw):
    label=raw.get('label')
    if label is None:return None
    match=re.fullmatch(r'\s*(?:Question\s*)?(\d+)[.)]?\s*',str(label),re.I)
    return int(match[1]) if match else None

SUPERSCRIPT_DIGITS = '⁰¹²³⁴⁵⁶⁷⁸⁹'
SUPERSCRIPT_TRANSLATION = str.maketrans(SUPERSCRIPT_DIGITS + '⁺⁻', '0123456789+-')


def math_tree(text):
    """Structural numeric math comparison, not evaluation or answer generation."""
    if '_ws_mixed' in text:raise ValueError('Reserved math marker')
    text=text.replace('−','-').replace('–','-').replace('×','*').replace('÷','/').replace('^','**')
    # Keep Unicode exponents as powers, including their sign and all digits.
    def superscript_power(match):
        exponent = match[0].translate(SUPERSCRIPT_TRANSLATION)
        if not re.fullmatch(r'[+-]?[0-9]+', exponent):
            raise ValueError('Invalid superscript exponent')
        return f'**({int(exponent)})'

    text = re.sub(rf'(?<=[\w)])[{SUPERSCRIPT_DIGITS}⁺⁻]+', superscript_power, text)
    # The app's own prompt asks for linear math: Unicode relations and (a)/(b) fractions.
    text=text.replace('≥','>=').replace('≤','<=').replace('≠','!=')
    relations={'geq':'>=','ge':'>=','leq':'<=','le':'<=','neq':'!='}
    text=re.sub(r'\\(geq|ge|leq|le|neq)\b',lambda m:relations[m[1]],text)
    text=text.replace(r'\%','%').replace('\u0305','\u0304')
    text=re.sub(r'\\(?:overline|bar)\{(\d+)\}',lambda m:m[1]+'\u0304',text)
    if re.fullmatch(r'\s*\d+(?:\.\d+)?\u0304?%\s*',text):return ('percent',text.strip())
    text=re.sub(r'\\(?:left|right)\b','',text)
    def group(value,start):
        while start<len(value) and value[start].isspace():start+=1
        if start>=len(value) or value[start]!='{':raise ValueError('Expected math group')
        depth=1;end=start+1
        while end<len(value) and depth:
            depth+=(value[end]=='{')-(value[end]=='}');end+=1
        if depth:raise ValueError('Unclosed math group')
        return value[start+1:end-1],end
    mixed={}
    # Linear mixed number: `2 (2)/(5)` is whole 2 plus the fraction, never 2*(2/5).
    while match:=re.search(r'(?<![\w.])(\d+)\s+\((\d+)\)/\((\d+)\)',text):
        marker='_ws_mixed'+str(len(mixed));mixed[marker]=('mixed',int(match[1]),('number',int(match[2])),('number',int(match[3])))
        text=text[:match.start()]+marker+text[match.end():]
    while match:=re.search(r'(?<![\w.])(\d+)\s*\\(?:dfrac|tfrac|frac)\b',text):
        a,end=group(text,match.end());b,end=group(text,end);marker='_ws_mixed'+str(len(mixed))
        mixed[marker]=('mixed',int(match[1]),math_tree(a),math_tree(b))
        text=text[:match.start()]+marker+text[end:]
    while match:=re.search(r'\\(?:dfrac|tfrac|frac)\b',text):
        a,end=group(text,match.end());b,end=group(text,end)
        text=text[:match.start()]+'('+a+')/('+b+')'+text[end:]
    text=text.replace('{','(').replace('}',')').strip()
    text=re.sub(r'\\[,;! ]','',text)
    text=re.sub(r'(?<=\d)(?=[A-Za-z])','*',text)
    def node(n):
        if isinstance(n,ast.Constant) and isinstance(n.value,(int,float)) and not isinstance(n.value,bool):return ('number',n.value)
        if isinstance(n,ast.Name) and n.id in mixed:return mixed[n.id]
        if isinstance(n,ast.Name) and len(n.id)==1:return ('variable',n.id)
        if isinstance(n,ast.UnaryOp) and isinstance(n.op,ast.USub):return ('negative',node(n.operand))
        if isinstance(n,ast.BinOp) and isinstance(n.op,(ast.Add,ast.Sub,ast.Mult,ast.Div,ast.Pow)):
            a,b=node(n.left),node(n.right);name=type(n.op).__name__
            if isinstance(n.op,ast.Div):
                negative=False
                if a[0]=='negative':negative=not negative;a=a[1]
                if b[0]=='negative':negative=not negative;b=b[1]
                result=(name,a,b)
                return ('negative',result) if negative else result
            return (name,a,b)
        raise ValueError('Unsupported independent math reference')
    relation=re.split(r'(>=|<=|!=|=|>|<)',text)
    if len(relation)==3:return ('relation',relation[1],node(ast.parse(relation[0].strip(),mode='eval').body),node(ast.parse(relation[2].strip(),mode='eval').body))
    if len(relation)!=1:raise ValueError('Unsupported chained relation')
    return node(ast.parse(text,mode='eval').body)

def item_reference_checks(case,by_number):
    refs=case.get('question_expectations',{})
    checks={}
    for number in case['question_numbers']:
        ref=refs.get(str(number));q=by_number.get(number)
        if ref is None:continue
        math_ok=False;instruction_ok=False;missing=[]
        if q is not None and ('math' in ref or 'maths' in ref):
            stem=q['stem'];segments=re.findall(r'(?<!\\)\$(.+?)(?<!\\)\$',stem,re.S)
            if not segments:segments=[stem.strip().splitlines()[-1]]
            expected=ref.get('maths',[ref.get('math')])
            try:math_ok=[math_tree(x) for x in segments]==[math_tree(x) for x in expected]
            except (ValueError,SyntaxError,TypeError):pass
            plain=' '.join(stem.casefold().split())
            for alternatives in ref.get('instruction_any',[]):
                if not any(re.search(r'\b'+re.escape(a.casefold())+r'\b',plain) for a in alternatives):missing.append(alternatives)
            instruction_ok=not missing
        checks[str(number)]={'math_matches':math_ok,'instruction_matches':instruction_ok,'missing_instruction_groups':missing,'content_pass':math_ok and instruction_ok}
    return checks

def content_reference(case,by_number,threshold=DEFAULT_THRESHOLD):
    checks=item_reference_checks(case,by_number)
    if len(checks)!=len(case['question_numbers']):return 'pending independent mathematical reference/review',None
    ratio=sum(x['content_pass'] for x in checks.values())/len(checks) if checks else 0
    return ('independently_passed' if ratio>=threshold else 'independently_failed'),ratio

# The app's marker on a question saved without its key (src/model.rs KEYLESS_SENTINEL).
KEYLESS_SENTINEL='No answer key: this question was imported without its key. Add the correct answers before printing a key.'

def has_invented_key(q):
    kind=q.get('kind',{})
    if 'Short' in kind:return bool(kind['Short'].get('sample','').strip())
    if 'Blank' in kind:return any(any(str(x).strip() for x in accepts) for accepts in kind['Blank'].get('accept',[]))
    if 'Multipart' in kind:return any(has_invented_key({'kind':p.get('kind',{}),'explanation':q.get('explanation')}) for p in kind['Multipart'].get('parts',[]))
    # A newly invented MC/Tf answer is also a source-key failure on these keyless pages. A True/False
    # always holds a value, so only the keyless marker shows that value is a placeholder, not a key.
    return ('Mc' in kind and any(a.get('correct') for a in kind['Mc'].get('answers',[]))) or ('Tf' in kind and q.get('explanation')!=KEYLESS_SENTINEL)

def keyed_choice_answers(q):
    """((part index or None, choice index), answer) for every answer choice of a saved question."""
    kind=q.get('kind') or {}
    if 'Mc' in kind:return [((None,i),a) for i,a in enumerate(kind['Mc'].get('answers') or [])]
    out=[]
    for pi,part in enumerate((kind.get('Multipart') or {}).get('parts') or []):
        out.extend(((pi,i),a) for i,a in enumerate(((part.get('kind') or {}).get('Mc') or {}).get('answers') or []))
    return out

def choice_answers(q):
    """Every answer choice of a saved question: its own (multiple choice) and its parts'."""
    kind=q.get('kind') or {}
    if 'Mc' in kind:return list(kind['Mc'].get('answers') or [])
    out=[]
    for part in (kind.get('Multipart') or {}).get('parts') or []:
        out.extend(((part.get('kind') or {}).get('Mc') or {}).get('answers') or [])
    return out

def leading_continuation(case,items,numbers):
    """A page that opens mid-problem: the importer saves the printed continuation first, labelled
    "continued" and unnumbered. It answers to the one expected number nothing else claims, and only
    when that number comes before every number the page does print."""
    if not items or numbers[0] is not None:return numbers
    if re.sub(r'[\s()\[\]]','',str(items[0].get('label') or '')).lower()!='continued':return numbers
    printed=[n for n in numbers if n is not None]
    missing=[n for n in case['question_numbers'] if n not in printed]
    if len(missing)==1 and all(missing[0]<n for n in printed):return [missing[0],*numbers[1:]]
    return numbers

def parsed_source_map(case,page,questions,previous=None):
    """Source number -> saved question, from the importer's own per-raw-item provenance.

    The importer records every raw item (saved or dropped) with its printed number and the index of the
    question it became, so a dropped item can never shift a later number onto the wrong question. Traces
    from before that record fall back to zipping raw labels with surviving questions, which is only right
    when nothing was dropped.

    Continued content the importer joined onto the previous page's last question (status
    merged_into_previous_page) resolves to that question in `previous`, the previous page's questions."""
    items=page.get('items')
    if items is None:
        raw=raw_questions(page);numbers=[source_number(q) for q in raw]
        if len(numbers)==1 and numbers[0] is None and (case['source_id']=='statistics-confidence-intervals' or (case.get('single_unnumbered_item') and len(case['question_numbers'])==1)):numbers=[1 if case['source_id']=='statistics-confidence-intervals' else case['question_numbers'][0]]
        by_number={n:q for n,q in zip(numbers,questions) if n is not None}
        return {'numbers':numbers,'by_number':by_number,'index':{n:i for i,n in enumerate(numbers) if n is not None},
                'all_saved':len(raw)==len(questions),'unresolved':[],'status':{},'provenance':'legacy_zip'}
    numbers=[i.get('number') for i in items]
    # A case declaring dotted labels ("Exercise 2.15") keys each item by the label's last part.
    # Section-numbered items whose last parts repeat across sections ("5.3" and "6.3") key by the
    # whole number, section*100+item, with a trailing part letter ("6.3a") kept on its item.
    # A case printing letter-prefixed numbers ("A1", "B3") declares its own label -> number map.
    if case.get('label_numbers'):
        lab=lambda l:re.sub(r'[\s().]','',str(l or '')).upper()
        table={lab(k):v for k,v in case['label_numbers'].items()}
        numbers=[table.get(lab(i.get('label'))) for i in items]
    elif case.get('dotted_full'):
        numbers=[int(m.group(1))*100+int(m.group(2)) if (m:=re.fullmatch(r'\s*(?:Q(?:uestion)?\s*)?(\d+)\.(\d+)\s*(?:\(?[a-z]\)?)?\s*',str(i.get('label') or ''),re.I)) else None for i in items]
    elif case.get('dotted_labels'):
        numbers=[n if n is not None else (int(m.group(1)) if (m:=re.fullmatch(r'\s*(?:[A-Za-z]\.)?\d+(?:\.\d+)*\.(\d+)\s*',str(i.get('label') or ''))) else None) for n,i in zip(numbers,items)]
    if len(numbers)==1 and numbers[0] is None and (case['source_id']=='statistics-confidence-intervals' or (case.get('single_unnumbered_item') and len(case['question_numbers'])==1)):numbers=[1 if case['source_id']=='statistics-confidence-intervals' else case['question_numbers'][0]]
    numbers=leading_continuation(case,items,numbers)
    # A page that prints no question numbers (the reference numbers its items in page order): the
    # saved items without a number, parts split from one item counted once, take the reference's
    # numbers in order, only when the counts agree.
    if case.get('ordinal_items'):
        free=[k for k,n in enumerate(numbers) if n is None]
        groups_in_order=[]
        for k in free:
            g=str(items[k].get('id') or k).split('.')[0]
            if g not in groups_in_order:groups_in_order.append(g)
        if len(groups_in_order)==len(case['question_numbers']):
            numbers=list(numbers)
            for k in free:numbers[k]=case['question_numbers'][groups_in_order.index(str(items[k].get('id') or k).split('.')[0])]
    by_number={};index={};status={};unresolved=[];groups={};merged={}
    for number,item in zip(numbers,items):
        if number is None:continue
        status[number]=item.get('status')
        mi=item.get('merged_into')
        if item.get('status')=='merged_into_previous_page' and previous is not None and type(mi) is int and 0<=mi<len(previous):
            by_number[number]=previous[mi];merged[number]=mi;continue
        qi=item.get('question_index')
        if qi is not None and 0<=qi<len(questions):
            by_number[number]=questions[qi];index[number]=qi
            groups.setdefault(number,[]).append(qi)
            # A valid crop can still overlap another problem or have no visible task. New or
            # explicit review states must never silently become complete source items.
            if item.get('status') not in {'saved','saved_figure_only'}:unresolved.append(number)
    return {'numbers':numbers,'by_number':by_number,'index':index,
            'all_saved':all(i.get('question_index') is not None or (i.get('status')=='merged_into_previous_page' and i.get('merged_into') in merged.values()) for i in items),
            'unresolved':unresolved,'status':status,'provenance':'importer_source_items','groups':groups,'merged':merged}

def figure_execution_evidence(page):
    """Diagnostic logical-call counts across ALL retained attempts, never a source-quality gate.

    Older traces only expose selected-attempt steps; do not present those as total calls.
    Provider-side HTTP retries/billing are outside this logical conversation trace.
    """
    result={'recorded_executions':0,'recorded_attempts':0,'maker_turns':0,'observer_call_attempts':0,
            'judge_call_attempts':0,'render_tool_steps':0,'schema_tool_steps':0,'discovery_steps':0,
            'legacy_runs':0,'legacy_selected_render_steps':0,'selected_spec_mismatches':[],
            'turn_budget_violations':[],'trace_complete':True,
            'judge_detail_panels':0,'judge_detail_fallbacks':0,'judge_image_contract_issues':[],
            'observer_detail_panels':0,'observer_detail_fallbacks':0,'observer_image_contract_issues':[],
            'observer_focus_details':0,'source_plot_checks':0,'source_plot_conflicts':0,'source_plot_unsupported':0,
            'source_plot_binding_issues':[],'source_plot_without_final_artifact':[]}
    for run in page.get('figure_runs',[]):
        executions=run.get('executions')
        if executions is None:
            result['legacy_runs']+=1;result['trace_complete']=False
            result['legacy_selected_render_steps']+=sum(s.get('tool')=='render_figure' for s in run.get('steps',[]))
            continue
        for group,execution in enumerate(executions):
            if execution.get('protocol')!='figure_execution_v1':result['trace_complete']=False;continue
            result['recorded_executions']+=1
            observation=execution.get('observation') or {};replies=observation.get('replies',[])
            result['observer_call_attempts']+=len(replies)
            for reply in replies:
                details=reply.get('image_derivations') or []
                result['observer_detail_panels']+=len(details)
                result['observer_focus_details']+=sum(d.get('protocol')=='source_reinspection_focus_v1' for d in details)
                result['observer_detail_fallbacks']+=bool(reply.get('image_detail_error'))
                if details:
                    hashes=reply.get('input_image_sha256') or []
                    protocol=('source_reinspection_focus_v1' if len(details)==1 else 'source_reinspection_detail_v1')
                    valid=(len(details) in (1,2) and len(hashes)==len(details)+1 and hashes[0]==observation.get('vision_sha256')
                           and reply.get('original_input_sha256')==observation.get('original_sha256'))
                    valid=valid and all(d.get('source_image')==1 and d.get('output_image')==i+2
                        and d.get('source_sha256')==hashes[0] and d.get('png_sha256')==hashes[i+1]
                        and d.get('protocol')==protocol for i,d in enumerate(details))
                    if not valid:result['observer_image_contract_issues'].append({'question_index':run.get('question_index'),'execution':group,'attempt':reply.get('attempt')})
            attempts=execution.get('attempts',[]);result['recorded_attempts']+=len(attempts)
            for attempt in attempts:
                check=attempt.get('source_plot_check')
                if isinstance(check,dict):
                    result['source_plot_checks']+=1
                    result['source_plot_conflicts']+=bool(check.get('issues'))
                    result['source_plot_unsupported']+=bool(check.get('unsupported'))
                    if attempt.get('spec') is None and attempt.get('png_sha256') is None:
                        # A failed call after a preview has no selected final artifact. Its
                        # retained diagnostic cannot be bound to a nonexistent final PNG;
                        # that is unavailable evidence, not proof of a mismatched hash.
                        result['source_plot_without_final_artifact'].append({'question_index':run.get('question_index'),'execution':group,'attempt':attempt.get('number')})
                    elif (not isinstance(attempt.get('spec'),dict) or not attempt['spec'].get('id')
                        or not isinstance(attempt.get('png_sha256'),str) or not attempt['png_sha256']
                        or not isinstance(check.get('preview_png_sha256'),str) or not check['preview_png_sha256']
                        or check.get('preview_png_sha256')!=attempt.get('png_sha256')
                        or check.get('generator')!=attempt['spec'].get('id')):
                        result['source_plot_binding_issues'].append({'question_index':run.get('question_index'),'execution':group,'attempt':attempt.get('number')})
                turns=attempt.get('turns',[]);result['maker_turns']+=len(turns)
                result['judge_call_attempts']+=attempt.get('judge_turn') is not None
                judge=attempt.get('judge_turn') or {};details=judge.get('image_derivations') or []
                result['judge_detail_panels']+=len(details)
                result['judge_detail_fallbacks']+=bool(judge.get('image_detail_error'))
                if details:
                    hashes=((judge.get('appended_messages') or [{}])[0]).get('image_sha256') or []
                    expected=[attempt.get('png_sha256'),(execution.get('observation') or {}).get('vision_sha256')]
                    valid=(len(hashes)==4 and len(details)==2 and all(isinstance(h,str) for h in expected) and hashes[:2]==expected)
                    valid=valid and all(d.get('source_image')==i+1 and d.get('output_image')==i+3 and d.get('png_sha256')==hashes[i+2]
                                            and d.get('protocol')=='judge_edge_detail_v1' for i,d in enumerate(details))
                    if not valid:result['judge_image_contract_issues'].append({'question_index':run.get('question_index'),'execution':group,'attempt':attempt.get('number')})
                for turn in turns:
                    n,limit=turn.get('turn'),turn.get('limit')
                    if type(n) is not int or type(limit) is not int or not 1<=n<=limit<=10:
                        result['turn_budget_violations'].append({'question_index':run.get('question_index'),'execution':group,'attempt':attempt.get('number'),'turn':n,'limit':limit})
                steps=attempt.get('steps',[])
                result['render_tool_steps']+=sum(s.get('tool')=='render_figure' for s in steps)
                result['schema_tool_steps']+=sum(s.get('tool')=='get_figure' for s in steps)
                result['discovery_steps']+=sum(s.get('tool')=='discover_fallback' for s in steps)
            if group==len(executions)-1 and run.get('spec') is not None:
                selected=[a for a in attempts if a.get('number')==execution.get('selected_attempt')]
                if len(selected)!=1 or selected[0].get('spec')!=run['spec']:
                    result['selected_spec_mismatches'].append(run.get('question_index'))
    return result

def figure_failure_detail(number, question, run, *, scope_ready=True, unresolved=False, contract=None):
    """Explain an uncredited figure without guessing that the catalogue lacks a tool.

    New app traces retain a typed reason; legacy traces still have the exact reviewer
    objections. A maker's own compromise note must never replace that rejection.
    Capability gaps require a separate registry/schema check, not an inferred refusal.
    """
    failure=run.get('failure') or {}
    problems=(run.get('review') or {}).get('problems') or []
    executions=run.get('executions') or []
    decline=(executions[-1].get('declined') if executions else None)
    if failure:
        kind,detail=failure['kind'],failure['detail']
        problems=failure.get('reviewer_problems',problems)
    elif question is None:
        kind,detail='missing_question','The source question was not retained, so its figure could not be attached.'
    elif not scope_ready:
        kind,detail='source_scope_unconfirmed','The source crop was not confirmed complete; an editable redraw cannot be credited.'
    elif unresolved:
        kind,detail='source_item_unresolved','The source item is held for review; any redraw remains uncredited.'
    elif contract:
        kind,detail='figure_contract_mismatch','The saved figure differs from required figure parameters.'
    elif run.get('error'):
        kind,detail='generation_error',run['error']
    elif decline:
        kind,detail='model_declined',decline
    elif (run.get('review') or {}).get('matches') is False:
        kind='comparison_rejected'
        detail='; '.join(problems) or 'The reviewer rejected the redraw without a specific mismatch.'
    elif run.get('spec') and run.get('review') is None:
        kind,detail='review_unavailable','No source-comparison verdict was recorded.'
    else:
        kind,detail='unverified_output','A saved spec, image, matching render trace and positive source comparison were not all present.'
    attempts=[]
    for execution in executions:
        for attempt in execution.get('attempts',[]):
            attempts.append({'attempt':attempt.get('number'),'generator':(attempt.get('spec') or {}).get('id'),
                             'error':attempt.get('error'),'declined':attempt.get('declined'),
                             'judge_error':attempt.get('judge_error'),'review':attempt.get('review')})
    return {'number':number,'kind':kind,'detail':detail,'reviewer_problems':problems,
            'candidate_generator':failure.get('generator') or (run.get('spec') or {}).get('id'),
            'generator_registered':failure.get('generator_registered'),
            'capability_gap_confirmed':False,
            'capability_note':'A failed or declined redraw alone does not prove a missing Figures-tab type or renderer feature.',
            'source_image_retained':bool(question and question.get('image') and not question.get('figure')),
            'contract_differences':contract or [],'attempts':attempts}

def score_case(case,store,trace,threshold=DEFAULT_THRESHOLD,page_index=0):
    banks=json.loads((store/'banks.json').read_text());pages=trace.get('pages',[])
    if len(banks)!=1 or not pages or not 0<=page_index<len(pages):raise ValueError('Expected one isolated bank and a page trace')
    if len(pages)>1 and any(p.get('document') is None for p in pages):raise ValueError('Expected one isolated bank and one page trace')
    page=pages[page_index]
    offset=sum(len(p['questions']) for p in pages[:page_index])
    questions=banks[0]['questions'][offset:offset+len(page['questions'])]
    prev_page=pages[page_index-1] if page_index>0 else None
    previous=banks[0]['questions'][offset-len(prev_page['questions']):offset] if prev_page is not None else None
    source=parsed_source_map(case,page,questions,previous);numbers=source['numbers'];expected=case['question_numbers']
    merged=source.get('merged',{})
    by_number=source['by_number']
    # A question's own figure run; runs that redrew one of its answer choices carry 'choice'.
    runs={x['question_index']:x for x in page.get('figure_runs',[]) if x.get('choice') is None}
    matched_figures=[];missing_figures=[];invented_figures=[];broken_media=[];contract_failures={};figure_failures=[]
    required=set(case['required_figure_numbers'])
    scope=page.get('source_box_review')
    scope_ready=scope is None or scope.get('applied') is True
    confirmed_scope=None if scope is None else scope.get('confirmed_indices')
    if scope is not None and ('confirmed_indices' in scope or 'protocol' in scope):
        if not isinstance(confirmed_scope,list) or not all(type(i) is int and i>=0 for i in confirmed_scope):confirmed_scope=[]
    raw_index={item.get('question_index'):item.get('index') for item in page.get('items') or []}
    choice_runs={(x['question_index'],(x.get('choice') or {}).get('part'),(x.get('choice') or {}).get('choice')):x for x in page.get('figure_runs',[]) if x.get('choice') is not None}
    def verified(run,spec):
        return bool(spec) and run.get('accepted') and run.get('spec')==spec and run.get('review',{}).get('matches') is True and any(s['tool']=='render_figure' for s in run.get('steps',[]))
    def part_pictures(qi):
        """(has a picture, every picture redrawn and verified) for one saved question: its own and its choices'."""
        q=questions[qi];own=bool(q.get('image'));pictured=[(key,a) for key,a in keyed_choice_answers(q) if a.get('image')]
        # The page scope review checks a question's own figure box; answer-choice pictures are not
        # part of it (holdout v13, Nova Scotia #1: four option pictures beside two scope-reviewed figures).
        scope_ok=not own or (scope_ready if confirmed_scope is None else raw_index.get(qi) in confirmed_scope)
        own_ok=not own or verified(runs.get(qi,{}),q.get('figure'))
        choices_ok=all(verified(choice_runs.get((qi,)+key,{}),a.get('figure')) for key,a in pictured)
        return own or bool(pictured),bool(scope_ok and own_ok and choices_ok)
    def merged_pictures(mi):
        """part_pictures for a continuation joined onto question `mi` of the previous page, checked
        against that page's own figure runs and scope review."""
        q=previous[mi];own=bool(q.get('image'));pictured=[(key,a) for key,a in keyed_choice_answers(q) if a.get('image')]
        prev_runs={x['question_index']:x for x in prev_page.get('figure_runs',[]) if x.get('choice') is None}
        prev_choice={(x['question_index'],(x.get('choice') or {}).get('part'),(x.get('choice') or {}).get('choice')):x for x in prev_page.get('figure_runs',[]) if x.get('choice') is not None}
        prev_scope=prev_page.get('source_box_review')
        scope_ok=not own or prev_scope is None or prev_scope.get('applied') is True
        own_ok=not own or verified(prev_runs.get(mi,{}),q.get('figure'))
        choices_ok=all(verified(prev_choice.get((mi,)+key,{}),a.get('figure')) for key,a in pictured)
        return own or bool(pictured),bool(scope_ok and own_ok and choices_ok),prev_runs.get(mi,{})
    for n in required:
        if n in merged:
            has,ok,run=merged_pictures(merged[n]);q=previous[merged[n]]
            good=has and ok and not has_invented_key(q)
            (matched_figures if good else missing_figures).append(n)
            if not good:figure_failures.append(figure_failure_detail(n,q,run,scope_ready=scope_ready,unresolved=False,contract=None))
            continue
        q=by_number.get(n);index=source['index'].get(n)
        # Every saved question carrying this number: one, or each part of a question split for its figures.
        parts=[part_pictures(qi) for qi in source.get('groups',{}).get(n,[index] if index is not None else [])]
        pictured=[ok for has,ok in parts if has]
        primary=next((qi for qi in source.get('groups',{}).get(n,[]) if questions[qi].get('image')),index)
        if primary is not None:q=questions[primary];index=primary
        run=runs.get(index,{})
        item_scope_ready=scope_ready if confirmed_scope is None else raw_index.get(index) in confirmed_scope
        good=q is not None and n not in source['unresolved'] and bool(pictured) and all(pictured)
        spec_ref=case.get('figure_expectations',{}).get(str(n))
        if good and spec_ref:
            differences=[]
            if q['figure']['id'] not in spec_ref['generator_ids']:differences.append({'field':'id','expected':spec_ref['generator_ids'],'actual':q['figure']['id']})
            differences.extend({'field':k,'expected':v,'actual':q['figure']['params'].get(k)} for k,v in spec_ref.get('params',{}).items() if q['figure']['params'].get(k)!=v)
            differences.extend({'field':k,'expected_one_of':values,'actual':q['figure']['params'].get(k)} for k,values in spec_ref.get('params_one_of',{}).items() if q['figure']['params'].get(k) not in values)
            if spec_ref.get('no_visible_title'):
                mode=q['figure']['params'].get('title_mode','Text')
                title=q['figure']['params'].get('title','')
                if mode!='None' and (mode!='Text' or str(title).strip()):differences.append({'field':'visible_title','expected':'none','actual':title,'title_mode':mode})
            if differences:contract_failures[str(n)]=differences
            good=not differences
        (matched_figures if good else missing_figures).append(n)
        if not good:
            figure_failures.append(figure_failure_detail(n,q,run,scope_ready=item_scope_ready,
                unresolved=n in source['unresolved'],contract=contract_failures.get(str(n))))
    for n,q in by_number.items():
        if n not in required and n not in merged and (q.get('image') or q.get('figure')):invented_figures.append(n)
    for q in questions:
        image=q.get('image')
        if image:
            p=contained(store,image['file'])
            try:
                with Image.open(p) as decoded:
                    decoded.verify()
                if re.fullmatch(r'[a-f0-9]{64}',p.stem) and sha(p)!=p.stem:broken_media.append(q['id'])
            except (OSError,ValueError):broken_media.append(q['id'])
    count_ok=len(questions)+len(merged)==len(expected)
    order_ok=numbers==expected and source['all_saved']
    key_ok=not any(has_invented_key(q) for q in questions)
    # The bank gives a question with no printed points the default; that is the only allowed
    # difference, judged per question (holdout v6: printed marks on four questions, none on one).
    persisted_ok=len(questions)==len(page['questions']) and all(q==t or (t.get('points') is None and dict(q,points=None)==t) for q,t in zip(questions,page['questions']))
    whole_bank_ok=len(banks[0]['questions'])==sum(len(p['questions']) for p in pages)
    integrity_ok=bool(questions) and persisted_ok and whole_bank_ok and not broken_media and len({q['id'] for q in questions})==len(questions)
    no_fake_figures=not invented_figures
    figure_ok=not missing_figures and no_fake_figures
    certification,content_ratio=content_reference(case,by_number,threshold)
    item_checks=item_reference_checks(case,by_number)
    for n,check in item_checks.items():
        check['figure_pass']=int(n) not in required or int(n) in matched_figures
        check['source_task_unresolved']=int(n) in source['unresolved']
        check['complete_item_pass']=check['content_pass'] and check['figure_pass'] and not has_invented_key(by_number.get(int(n),{})) and not check['source_task_unresolved']
    return {'status':'completed','questions_expected':len(expected),'questions_saved':len(questions),
            'source_questions_found':len(set(by_number).intersection(expected)),
            'question_numbers':numbers,'count_ok':count_ok,'order_ok':order_ok,'source_key_only':key_ok,
            'bank_integrity_ok':integrity_ok,'figure_required':len(required),'figures_recreated':len(matched_figures),
            'figures_model_approved':sum(bool(run.get('accepted')) for run in runs.values()),
            'figure_execution_evidence':figure_execution_evidence(page),
            'figure_missing':missing_figures,'invented_figures':invented_figures,'figure_ok':figure_ok,
            'figure_contract_failures':contract_failures,
            'figure_failures':sorted(figure_failures,key=lambda f:f['number']),
            'structural_pass':count_ok and order_ok and key_ok and integrity_ok and no_fake_figures,
            'content_certification':certification,'independent_content_score':content_ratio,
            'item_source_checks':item_checks,'source_provenance':source['provenance'],'source_items':page.get('items'),
            'review_unresolved':source['unresolved'],'inherited_direction_items':[i.get('number') for i in page.get('items') or [] if (i.get('directions') or {}).get('origin')=='inherited_from_document_context'],
            'source_box_review_completed':None if scope is None else scope_ready,
            'context_directions_supplied':len((page.get('context') or {}).get('directions',[])),
            'warnings':page.get('warnings',[]),'store':str(store),'bank_sha256':sha(store/'banks.json'),
            'trace_sha256':sha(store/'trace.json')}

def document_groups(cases):
    """Cases grouped by source document, each document's pages in original PDF order. A document boundary
    is a new source_id, so continuation context can never cross documents."""
    groups={}
    for case in cases:groups.setdefault(case['source_id'],[]).append(case)
    for source_id,items in groups.items():
        items.sort(key=lambda c:c['source_pdf_page'])
        pages=[c['source_pdf_page'] for c in items]
        if len(set(pages))!=len(pages):raise ValueError('Duplicate source page in '+source_id)
    return dict(sorted(groups.items()))

def summarize(cases,results,threshold):
    n=len(cases);completed=sum(r.get('status')=='completed' for r in results)
    structural=sum(bool(r.get('structural_pass')) for r in results)
    figures_total=sum(len(c['required_figure_numbers']) for c in cases)
    figures_ok=sum(r.get('figures_recreated',0) for r in results)
    integrity=sum(bool(r.get('bank_integrity_ok')) for r in results)
    certified=sum(r.get('content_certification')=='independently_passed' for r in results)
    total_questions=sum(len(c['question_numbers']) for c in cases)
    item_checks=[x for r in results for x in r.get('item_source_checks',{}).values()]
    metrics={'import_completion':completed/n,'structural_cases':structural/n,'bank_integrity':integrity/n,
             'question_coverage':sum(r.get('source_questions_found',0) for r in results)/total_questions if total_questions else None,
             'figure_recreation':figures_ok/figures_total if figures_total else None,
             'independently_certified_content':certified/n,
             'complete_source_items':sum(x['complete_item_pass'] for x in item_checks)/total_questions if total_questions else None}
    gates={k:(v is None or v>=threshold) for k,v in metrics.items()}
    return {'threshold':threshold,'cases':n,'cases_required_to_pass':math.ceil(threshold*n),'completed':completed,
            'completed_cases_with_source_content_references':sum(r.get('independent_content_score') is not None for r in results),
            'source_figures_in_completed_cases':sum(r.get('figure_required',0) for r in results),
            'model_approved_figures_before_independent_contracts':sum(r.get('figures_model_approved',0) for r in results),
            'source_items_checked':len(item_checks),
            'source_math_items_matched':sum(x['math_matches'] for x in item_checks),
            'source_instructions_missing':sum(not x['instruction_matches'] for x in item_checks),
            'complete_source_items_passed':sum(x['complete_item_pass'] for x in item_checks),
            'expected_questions':sum(len(c['question_numbers']) for c in cases),
            'saved_questions':sum(r.get('questions_saved',0) for r in results),
            'expected_figures':figures_total,'recreated_figures':figures_ok,'metrics':metrics,'gates':gates,
            'accepted':all(gates.values()),'accuracy_claim':'No mathematical accuracy certification from counts, serialization, or a model judging its own figures.'}

def main():
    ap=argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--manifest',type=Path,required=True);ap.add_argument('--gold',type=Path,default=ROOT/'tests/worksheet_banks/cases.json')
    ap.add_argument('--binary',type=Path,default=ROOT/'target/debug/vershion');ap.add_argument('--out',type=Path,required=True)
    ap.add_argument('--provider',choices=['ollama','openrouter','cloud','custom','chatgpt'],default='ollama');ap.add_argument('--model')
    ap.add_argument('--base-url',help='OpenAI-compatible base URL, required for --provider custom');ap.add_argument('--key-env',default='VERSHION_AI_KEY',help='Environment variable that holds the custom endpoint key (name only; the value is never stored)')
    ap.add_argument('--allow-remote',action='store_true');ap.add_argument('--replay',action='store_true');ap.add_argument('--only',action='append',default=[])
    ap.add_argument('--mode',choices=['page','document'],default='page',help='page: each page imported alone (ablation). document: each source document imported in page order with earlier-page directions as context, one process per document.');ap.add_argument('--threshold',type=float,default=DEFAULT_THRESHOLD);ap.add_argument('--workers',type=int,default=2);ap.add_argument('--timeout',type=int,default=240)
    a=ap.parse_args()
    if not 0<a.threshold<=1:ap.error('Threshold must be in (0, 1]')
    if not 1<=a.workers<=4:ap.error('Workers must be 1..4')
    if a.provider!='ollama' and not a.replay and (not a.allow_remote or not a.model):ap.error('Remote providers require --allow-remote and an explicit --model')
    if a.provider=='custom' and not a.replay:
        if not custom_base_allowed(a.base_url):ap.error('Custom provider needs --base-url (https, or loopback http, without embedded credentials/query/fragment)')
    a.model=a.model or 'gemma4:31b'
    if a.provider=='ollama' and not a.replay:
        import sys
        sys.path.insert(0,str(ROOT/'tests/import_battery'))
        from battery import check_provider
        check_provider('ollama','http://127.0.0.1:11434',a.model,False)
    corpus=a.manifest.resolve().parent;manifest=json.loads(a.manifest.read_text());gold=json.loads(a.gold.read_text())
    inputs={p['file']:p for p in manifest['pages']};cases=gold['cases']
    assert set(inputs)=={c['file'] for c in cases},'Gold must account for the entire manifest'
    for c in cases:
        assert inputs[c['file']]['sha256']==c['sha256']
        assert sha(contained(corpus,c['file']))==c['sha256']
    selected=[c for c in cases if not a.only or c['id'] in a.only]
    if a.only and set(a.only)-{c['id'] for c in cases}:ap.error('Unknown --only case')
    a.out.mkdir(parents=True,exist_ok=True)
    env={k:v for k,v in os.environ.items() if not k.startswith('VERSHION_') and not k.endswith('API_KEY')}
    if a.provider=='custom' and not a.replay:
        if os.environ.get(a.key_env):env['VERSHION_AI_KEY']=os.environ[a.key_env]
    elif a.provider in ('openrouter','cloud') and not a.replay:
        keys=json.loads((Path.home()/'.local/share/vershion/ai-keys.json').read_text())
        env['OPENROUTER_API_KEY' if a.provider=='openrouter' else 'OLLAMA_API_KEY']=keys['openrouter' if a.provider=='openrouter' else 'cloud']
    settings={'ai_provider':a.provider,'ai_model':a.model,'ai_output_mode':'json','ai_native_tools':True}
    if a.provider=='custom':settings['ai_base']=(a.base_url or '').rstrip('/')
    current_binary_sha=sha(a.binary)
    binary_sha=current_binary_sha
    # Re-scoring a saved bank evaluates the app version that produced it. A new
    # build must not erase old evidence, nor be credited with old results.
    previous_report=a.out/'report.json'
    if a.replay and previous_report.exists():
        previous=json.loads(previous_report.read_text())
        if previous.get('provider')==a.provider and previous.get('model')==a.model:
            saved_sha=previous.get('binary_sha256','')
            if re.fullmatch(r'[a-f0-9]{64}',saved_sha):binary_sha=saved_sha
    results={};start=clock_sample();provider_blocked=threading.Event()
    def run(case):
        directory=a.out/'cases'/case['id'];directory.mkdir(parents=True,exist_ok=True)
        cache_id=identity({'source':case['sha256'],'binary':binary_sha,'settings':settings,'hint':HINT})
        state=directory/'result.json';store=directory/'store'
        if state.exists():
            previous=json.loads(state.read_text())
            if previous.get('cache_id')==cache_id and previous.get('status')=='completed':
                try:
                    assert sha(store/'banks.json')==previous['bank_sha256'] and sha(store/'trace.json')==previous['trace_sha256']
                    result=score_case(case,store,json.loads((store/'trace.json').read_text()),a.threshold);result.update(cache_id=cache_id,cache_hit=True,seconds=0)
                    write(state,result);return case['id'],result
                except (OSError,ValueError,AssertionError):pass
            if a.replay and previous.get('cache_id')==cache_id:return case['id'],previous
        if a.replay:return case['id'],{'status':'cache_missing_or_invalid','seconds':0}
        if provider_blocked.is_set():return case['id'],{'status':'provider_blocked_not_run','seconds':0}
        store.mkdir(exist_ok=True);write(store/'settings.json',settings)
        # Reversible scratch profile only. A rerun replaces its own previous bank file.
        (store/'banks.json').unlink(missing_ok=True);(store/'trace.json').unlink(missing_ok=True)
        child_env=dict(env,VERSHION_STORE_DIR=str(store.resolve()),VERSHION_IMPORT_IMAGE=str(contained(corpus,case['file'])),
                       VERSHION_BANK_NAME='Worksheet harness: '+case['id'],VERSHION_IMPORT_TRACE=str((store/'trace.json').resolve()),
                       VERSHION_IMPORT_HINT=HINT,VERSHION_AI_PARALLEL='1')
        t=clock_sample();result={'status':'failed'}
        with (directory/'stdout.log').open('w') as stdout,(directory/'stderr.log').open('w') as stderr:
            p=subprocess.Popen([str(a.binary.resolve())],env=child_env,stdout=stdout,stderr=stderr,start_new_session=True)
            try:
                p.wait(timeout=a.timeout)
                if p.returncode==0:result=score_case(case,store,json.loads((store/'trace.json').read_text()),a.threshold)
                else:
                    detail=(directory/'stderr.log').read_text()[-1600:]
                    result['error']='Import process exited '+str(p.returncode)+': '+detail
                details=result.get('error','')+' '.join(result.get('warnings',[]))
                if any(marker in details for marker in ['answered 401:','answered 402:','answered 403:','answered 429:']):provider_blocked.set()
            except subprocess.TimeoutExpired:
                os.killpg(p.pid,signal.SIGKILL);p.wait();result={'status':'timeout','error':'Page import exceeded time budget'}
            except Exception as e:result={'status':'failed','error':str(e)}
        timing=timing_since(t)
        result.update(cache_id=cache_id,cache_hit=False,seconds=timing['active_seconds'],timing=timing);write(state,result)
        return case['id'],result
    def run_document(source_id,doc_cases):
        directory=a.out/'documents'/source_id;directory.mkdir(parents=True,exist_ok=True)
        cache_id=identity({'source':[c['sha256'] for c in doc_cases],'binary':binary_sha,'settings':settings,'hint':HINT,'mode':'document'})
        state=directory/'result.json';store=directory/'store'
        def per_case(trace,extra):
            out={}
            for k,c in enumerate(doc_cases):
                if trace['pages'][k].get('error'):
                    result={'status':'failed','error':trace['pages'][k]['error'],'seconds':extra.get('seconds',0)}
                    result.update(extra,cache_id=cache_id,import_mode='document',document=source_id,document_pages=len(doc_cases),page_index=k)
                    write(a.out/'cases'/c['id']/'result.json',result);out[c['id']]=result;continue
                result=score_case(c,store,trace,a.threshold,page_index=k)
                result.update(extra,cache_id=cache_id,import_mode='document',document=source_id,document_pages=len(doc_cases),page_index=k)
                write(a.out/'cases'/c['id']/'result.json',result);out[c['id']]=result
            return out
        if state.exists():
            previous=json.loads(state.read_text())
            if previous.get('cache_id')==cache_id and previous.get('status')=='completed':
                try:
                    assert sha(store/'banks.json')==previous['bank_sha256'] and sha(store/'trace.json')==previous['trace_sha256']
                    return per_case(json.loads((store/'trace.json').read_text()),{'cache_hit':True,'seconds':0})
                except (OSError,ValueError,AssertionError):pass
        failure=lambda status,extra={}:{c['id']:dict({'status':status,'seconds':0,'import_mode':'document','document':source_id,'cache_id':cache_id},**extra) for c in doc_cases}
        if a.replay:return failure('cache_missing_or_invalid')
        if provider_blocked.is_set():return failure('provider_blocked_not_run')
        store.mkdir(exist_ok=True);write(store/'settings.json',settings)
        (store/'banks.json').unlink(missing_ok=True);(store/'trace.json').unlink(missing_ok=True)
        listing=directory/'pages.txt';listing.write_text('\n'.join(str(contained(corpus,c['file'])) for c in doc_cases)+'\n')
        child_env=dict(env,VERSHION_STORE_DIR=str(store.resolve()),VERSHION_IMPORT_DOCUMENT=str(listing),VERSHION_DOCUMENT_ID=source_id,
                       VERSHION_BANK_NAME='Worksheet harness document: '+source_id,VERSHION_IMPORT_TRACE=str((store/'trace.json').resolve()),
                       VERSHION_IMPORT_HINT=HINT,VERSHION_AI_PARALLEL='1')
        budget=a.timeout*len(doc_cases);t=clock_sample();status='failed';error=None
        with (directory/'stdout.log').open('w') as stdout,(directory/'stderr.log').open('w') as stderr:
            p=subprocess.Popen([str(a.binary.resolve())],env=child_env,stdout=stdout,stderr=stderr,start_new_session=True)
            try:
                p.wait(timeout=budget)
                if p.returncode==0:status='completed'
                else:error='Import process exited '+str(p.returncode)+': '+(directory/'stderr.log').read_text()[-1600:]
                if any(m in (error or '') for m in ['answered 401:','answered 402:','answered 403:','answered 429:']):provider_blocked.set()
            except subprocess.TimeoutExpired:
                os.killpg(p.pid,signal.SIGKILL);p.wait();status='timeout';error='Document import exceeded its time budget of %d seconds (%d pages x %d)'%(budget,len(doc_cases),a.timeout)
            except Exception as e:error=str(e)
        timing=timing_since(t);seconds=timing['active_seconds']
        if status=='completed':
            try:
                trace=json.loads((store/'trace.json').read_text())
                write(state,{'status':'completed','cache_id':cache_id,'bank_sha256':sha(store/'banks.json'),'trace_sha256':sha(store/'trace.json'),'seconds':seconds,'timing':timing,'budget_seconds':budget})
                return per_case(trace,{'cache_hit':False,'seconds':seconds,'document_seconds':seconds,'timing':timing,'budget_seconds':budget})
            except (OSError,ValueError) as e:status='failed';error=str(e)
        write(state,{'status':status,'cache_id':cache_id,'error':error,'seconds':seconds,'timing':timing,'budget_seconds':budget})
        return failure(status,{'error':error,'seconds':seconds,'document_seconds':seconds,'timing':timing,'budget_seconds':budget})
    def save():
        rows=[dict(id=c['id'],**results.get(c['id'],{'status':'not_run'})) for c in cases]
        timing=timing_since(start)
        report={'scope':'full_corpus' if not a.only else 'subset_with_full_corpus_denominator','import_mode':a.mode,'provider':a.provider,'model':a.model,'endpoint':(a.base_url or '').rstrip('/') if a.provider=='custom' else None,
                'model_calls_allowed':not a.replay,'binary_sha256':binary_sha,'gold_sha256':sha(a.gold),'elapsed_seconds':timing['active_seconds'],'timing':timing,
                'run_kind':'saved_bank_scoring' if a.replay else 'live_import','current_app_binary_sha256':current_binary_sha,
                'summary':summarize(cases,rows,a.threshold),'cases':rows}
        write(a.out/'report.json',report)
        return report
    save()
    with ThreadPoolExecutor(max_workers=a.workers) as pool:
        if a.mode=='document':
            wanted={c['id'] for c in selected};groups={k:v for k,v in document_groups(cases).items() if any(c['id'] in wanted for c in v)}
            futures={pool.submit(run_document,k,v):k for k,v in groups.items()}
            for f in as_completed(futures):
                for key,value in f.result().items():
                    results[key]=value;print(key,value['status'],'questions',value.get('questions_saved',0),'figures',value.get('figures_recreated',0),'seconds',value.get('seconds',0),flush=True)
                save()
        else:
            futures={pool.submit(run,c):c for c in selected}
            for f in as_completed(futures):
                key,value=f.result();results[key]=value;report=save()
                print(key,value['status'],'questions',value.get('questions_saved',0),'figures',value.get('figures_recreated',0),'seconds',value['seconds'],flush=True)
    report=save();print(json.dumps(report['summary'],indent=2))
    return 0 if report['summary']['accepted'] else 1
if __name__=='__main__':raise SystemExit(main())
