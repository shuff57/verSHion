#!/usr/bin/env python3
"""Validate council source-reference proposals without approving any candidate bank.

This checks provenance, coverage and reference structure. Source transcription
confidence and candidate fidelity remain separate; no bank result is read here.
"""
from __future__ import annotations
import argparse,hashlib,json
from collections import Counter
from pathlib import Path

FORBIDDEN_ANSWER_KEYS={'suggestedAnswer','correct_answer','worked_answer','worked_solution','inferred_answer','solution'}

class ReferenceError(ValueError):pass

def digest(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def require(condition,message):
    if not condition:raise ReferenceError(message)
def contained(root,path):
    resolved=Path(path).resolve()
    require(resolved.is_relative_to(root.resolve()),'Reference source path escapes corpus')
    return resolved

def no_solutions(value):
    if isinstance(value,dict):
        require(not FORBIDDEN_ANSWER_KEYS.intersection(value),'Reference includes an answer/solution field')
        for v in value.values():no_solutions(v)
    elif isinstance(value,list):
        for v in value:no_solutions(v)

def validate_reference(assignment,reference,corpus):
    require(reference.get('source_first') is True,'Source-first independence must be declared')
    require(isinstance(reference.get('reviewer'),str) and reference['reviewer'].strip(),'Missing source reviewer')
    items=assignment.get('items',[]);cases=reference.get('cases',[])
    require(isinstance(items,list) and bool(items),'Assignment is empty')
    require(isinstance(cases,list),'Reference cases must be a list')
    assigned={c['case_id']:c for c in items}
    require(len(assigned)==len(items),'Duplicate assignment case')
    ids=[c.get('case_id') for c in cases]
    require(len(set(ids))==len(ids),'Duplicate reference case')
    require(set(ids)==set(assigned),'Reference must account for every assigned case exactly once')
    manifest=json.loads((corpus/'manifest.json').read_text())
    allowed_pages={}
    for p in manifest['pages']:allowed_pages.setdefault(p['source_id'],set()).add(p['pdf_page'])
    page_records={(p['source_id'],p['pdf_page']):p for p in manifest['pages']}
    document_questions={}
    for c in items:document_questions.setdefault(c['source_id'],set()).update(c['question_numbers'])
    counts=Counter();inventory_differences=[];case_reports=[]
    for case in cases:
        id=case['case_id'];expected=assigned[id];source=contained(corpus,expected['source_png'])
        require(case.get('source_id')==expected['source_id'],id+': source document differs')
        require(source.is_file(),id+': source PNG is missing')
        sha=digest(source)
        require(sha==expected['source_sha256']==case.get('source_sha256'),id+': source PNG checksum mismatch')
        if 'source_png' in case:require(contained(corpus,case['source_png'])==source,id+': wrong source PNG path')
        if 'source_pdf' in case:
            pdf=contained(corpus,case['source_pdf'])
            require(pdf==contained(corpus,expected['source_pdf']),id+': wrong preserved PDF')
            require(pdf.is_file(),id+': preserved PDF is missing')
            if case.get('source_pdf_sha256'):require(digest(pdf)==case['source_pdf_sha256'],id+': preserved PDF checksum mismatch')
        if 'pdf_page' in case:require(case['pdf_page']==expected['pdf_page'],id+': wrong original PDF page')
        questions=case.get('question_expectations')
        require(isinstance(questions,dict),id+': question expectations must be an object')
        require(set(questions)==set(map(str,expected['question_numbers'])),id+': question numbers differ from assignment')
        figure_numbers=[]
        for number,q in questions.items():
            prefix=id+' question '+number
            require(isinstance(q,dict),prefix+': expected an object')
            require(q.get('status') in {'source_verified','needs_review'},prefix+': source status is missing/invalid')
            require(isinstance(q.get('source_body'),str),prefix+': full source body missing')
            require(isinstance(q.get('instruction'),str),prefix+': source task context missing')
            maths=q.get('maths');require(isinstance(maths,list) and all(isinstance(x,str) and x.strip() for x in maths),prefix+': mathematical facts malformed')
            fig=q.get('source_figure')
            require(fig is None or isinstance(fig,dict),prefix+': source figure malformed')
            require(bool(q['source_body'].strip()) or bool(fig),prefix+': empty body without source figure evidence')
            if fig:
                require(bool(fig.get('kind')),prefix+': missing figure kind')
                figure_numbers.append(int(number));counts['figures']+=1
            require(q.get('from_pdf_page',expected['pdf_page'])==expected['pdf_page'],prefix+': body provenance points to another page')
            no_solutions(q)
            counts['questions']+=1;counts[q['status']]+=1
        declared=set(expected['required_figure_numbers']);actual=set(figure_numbers)
        if declared!=actual:
            inventory_differences.append({'case_id':id,'assignment_figure_numbers':sorted(declared),'source_reference_figure_numbers':sorted(actual)})
        for instruction in case.get('document_instructions',[]):
            require(isinstance(instruction.get('text'),str) and bool(instruction['text'].strip()),id+': empty section direction')
            page=instruction.get('from_pdf_page')
            applies=set(instruction.get('applies_to',[]))
            require(isinstance(page,int) and not isinstance(page,bool) and page in allowed_pages[expected['source_id']],id+': direction provenance is not a question page in this document')
            require(not applies.intersection(expected['question_numbers']) or page<=expected['pdf_page'],id+': current-page direction incorrectly comes from a later page')
            require(applies.issubset(document_questions[expected['source_id']]),id+': direction mapped outside the assigned document questions')
            if instruction.get('heading_source_png'):
                heading=contained(corpus,instruction['heading_source_png']);record=page_records[(expected['source_id'],page)]
                require(heading==contained(corpus,corpus/record['file']),id+': heading image belongs to a different source page')
                require(digest(heading)==record['sha256']==instruction.get('heading_source_sha256'),id+': heading source checksum mismatch')
        counts['pages']+=1
        case_reports.append({'case_id':id,'source_sha256':sha,'questions':len(questions),'source_figures':len(figure_numbers)})
    return {'proposal_integrity':'passed','reviewer':reference['reviewer'],'source_first_declared':True,
            'pages':counts['pages'],'questions':counts['questions'],'source_figures':counts['figures'],
            'author_source_verified_records':counts['source_verified'],'author_uncertain_records':counts['needs_review'],
            'inventory_disagreements':inventory_differences,'source_fact_reconciliation':'pending coordinator source review',
            'candidate_banks_read':0,'candidate_acceptance':'not evaluated','candidate_pass_rate':None,'cases':case_reports}

def main():
    ap=argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--assignment',type=Path,required=True);ap.add_argument('--reference',type=Path,required=True)
    ap.add_argument('--corpus',type=Path,required=True);ap.add_argument('--out',type=Path,required=True)
    a=ap.parse_args()
    result=validate_reference(json.loads(a.assignment.read_text()),json.loads(a.reference.read_text()),a.corpus)
    result.update(assignment_sha256=digest(a.assignment),reference_sha256=digest(a.reference))
    a.out.parent.mkdir(parents=True,exist_ok=True);a.out.write_text(json.dumps(result,indent=2)+'\n')
    print(json.dumps({k:v for k,v in result.items() if k!='cases'},indent=2))
if __name__=='__main__':main()
