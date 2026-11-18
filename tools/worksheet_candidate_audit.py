#!/usr/bin/env python3
"""Combine independent item audits against pinned source, bank and trace bytes.

This never calls a model and never promotes a model's own figure verdict to an
independent source check. Missing reviews and uncertain source facts stay pending.
"""
from __future__ import annotations

import argparse
import hashlib
import json
from collections import Counter
from pathlib import Path

import worksheet_banks as harness


class AuditError(ValueError):
    pass


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def require(condition, message):
    if not condition:
        raise AuditError(message)


def combined(*statuses):
    if 'fail' in statuses:
        return 'fail'
    return 'pending' if 'pending' in statuses else 'pass'


def reconcile(cases, run, run_sha, references, audits, corpus, threshold=harness.DEFAULT_THRESHOLD):
    """references/audits are lists of (actual file SHA-256, decoded JSON)."""
    require(isinstance(threshold,(int,float)) and 0 < threshold <= 1, 'Threshold must be greater than zero and at most one')
    require(bool(cases) and all(c.get('question_numbers') for c in cases), 'Empty case/item denominator')
    expected = {c['id']: c for c in cases}
    require(len(expected) == len(cases), 'Duplicate expected case')
    results = {c['id']: c for c in run['cases']}
    require(len(results) == len(run['cases']), 'Duplicate run case')
    require(set(results) == set(expected), 'Run must retain the full case denominator')
    source_refs = {}
    for ref_sha, reference in references:
        for case in reference['cases']:
            cid = case.get('case_id', case.get('id'))
            # The fixed harness cases file also inventories the 38 formerly unreferenced pages.
            questions = case.get('question_expectations', {})
            if not questions:
                continue
            require(cid in expected, 'Reference case is outside corpus: ' + str(cid))
            require(cid not in source_refs, 'Overlapping source reference: ' + cid)
            require(set(questions) == set(map(str, expected[cid]['question_numbers'])), 'Incomplete source reference: ' + cid)
            require(case.get('source_sha256', case.get('sha256')) == expected[cid]['sha256'], 'Source reference hash mismatch: ' + cid)
            source_refs[cid] = (ref_sha, questions)

    reviewed = {}
    verified_stores = {}
    for audit_sha, audit in audits:
        reviewer = audit.get('reviewer')
        require(isinstance(reviewer, str) and reviewer.strip(), 'Audit reviewer is missing')
        require(audit.get('run_report_sha256') == run_sha, 'Audit refers to another run report')
        seen = set()
        for case in audit['cases']:
            cid = case['case_id']
            require(cid in expected and cid in source_refs, 'Audit has no pinned source reference: ' + cid)
            require(cid not in seen and cid not in reviewed, 'Overlapping candidate audit: ' + cid)
            seen.add(cid)
            ref_sha, refs = source_refs[cid]
            require(audit.get('reference_sha256') == ref_sha, 'Audit source reference changed: ' + cid)
            original = Path(corpus) / expected[cid]['file']
            require(digest(original) == expected[cid]['sha256'], 'Source PNG changed: ' + cid)
            result = results[cid]
            require(case['bank_sha256'] == result['bank_sha256'], 'Audit bank differs: ' + cid)
            require(case['trace_sha256'] == result['trace_sha256'], 'Audit trace differs: ' + cid)
            store = Path(result['store'])
            if str(store) not in verified_stores:
                require(digest(store / 'banks.json') == result['bank_sha256'], 'Candidate bank changed: ' + cid)
                require(digest(store / 'trace.json') == result['trace_sha256'], 'Candidate trace changed: ' + cid)
                trace = json.loads((store / 'trace.json').read_text())
                # Recheck actual saved spec/media/render trace, not merely the report's counts.
                verified_stores[str(store)] = trace
            else:
                require(digest(store / 'banks.json') == result['bank_sha256'] and digest(store / 'trace.json') == result['trace_sha256'], 'Inconsistent document hashes: ' + cid)
            verified = harness.score_case(expected[cid], store, verified_stores[str(store)], threshold=threshold, page_index=result.get('page_index', 0))
            page = verified_stores[str(store)]['pages'][result.get('page_index', 0)]
            saved = harness.parsed_source_map(expected[cid], page, page['questions'])
            require(set(case['items']) == set(map(str, expected[cid]['question_numbers'])), 'Audit must account for every item: ' + cid)
            items = {}
            required = set(expected[cid]['required_figure_numbers'])
            for number, item in case['items'].items():
                prefix = cid + ' #' + number
                require(isinstance(item.get('evidence'), dict) and item['evidence'], 'Audit evidence is missing: ' + prefix)
                require(isinstance(item.get('reasons'), list), 'Audit reasons are missing: ' + prefix)
                content = item.get('content_status')
                visual = item.get('figure_visual_status')
                editable = item.get('editable_recreation_status')
                require(content in {'pass', 'fail', 'pending'}, 'Invalid content status: ' + prefix)
                is_figure = int(number) in required
                allowed = {'pass', 'fail', 'pending'} if is_figure else {'not_required'}
                require(visual in allowed and editable in allowed, 'Invalid figure status: ' + prefix)
                if editable == 'pass':
                    require(visual == 'pass', 'Editable pass requires independent visual pass: ' + prefix)
                    require(int(number) not in verified['figure_missing'], 'Editable pass lacks valid render/spec/media trace: ' + prefix)
                    require(verified['bank_integrity_ok'], 'Editable pass has broken persisted media/bank: ' + prefix)
                source_status = refs[number].get('status', 'source_verified')
                source = 'pass' if source_status == 'source_verified' else 'pending'
                require(source_status in {'source_verified', 'needs_review'}, 'Invalid source confidence: ' + prefix)
                bank_status = combined(source, content, visual if is_figure else 'pass', editable if is_figure else 'pass')
                question = saved['by_number'].get(int(number))
                if not verified['bank_integrity_ok'] or question is None or harness.has_invented_key(question):
                    bank_status = 'fail'
                elif int(number) in saved['unresolved']:
                    bank_status = combined(bank_status, 'pending')
                rendering = item.get('worksheet_render_status', 'not_checked')
                require(rendering in {'pass', 'fail', 'pending', 'not_checked'}, 'Invalid worksheet render status: ' + prefix)
                render_status = 'pending' if rendering == 'not_checked' else rendering
                items[number] = {
                    'source_status': source_status, 'content_status': combined(source, content),
                    'figure_visual_status': combined(source, visual) if is_figure else 'not_required',
                    'editable_recreation_status': combined(source, editable) if is_figure else 'not_required',
                    'bank_item_status': bank_status, 'worksheet_render_status': render_status,
                    'complete_with_worksheet_status': combined(bank_status, render_status),
                    'reasons': item['reasons'], 'evidence': item['evidence'],
                }
            reviewed[cid] = {'case_id': cid, 'reviewer': reviewer, 'audit_sha256': audit_sha, 'items': items}

    pages = []
    for cid, case in expected.items():
        if cid in reviewed:
            pages.append(reviewed[cid])
            continue
        pages.append({'case_id': cid, 'reviewer': None, 'items': {str(n): {
            'content_status': 'pending',
            'figure_visual_status': 'pending' if n in case['required_figure_numbers'] else 'not_required',
            'editable_recreation_status': 'pending' if n in case['required_figure_numbers'] else 'not_required',
            'bank_item_status': 'pending', 'worksheet_render_status': 'pending',
            'complete_with_worksheet_status': 'pending', 'reasons': ['Independent candidate audit missing.'],
        } for n in case['question_numbers']}})
    items = [i for page in pages for i in page['items'].values()]
    fields = ('content_status', 'figure_visual_status', 'editable_recreation_status', 'bank_item_status', 'worksheet_render_status', 'complete_with_worksheet_status')
    counts = {field: dict(Counter(item[field] for item in items)) for field in fields}
    total = len(items)
    figures = sum(len(c['required_figure_numbers']) for c in cases)
    rates = {field: counts[field].get('pass', 0) / denominator if denominator else None
             for field, denominator in [('content_status', total), ('editable_recreation_status', figures), ('bank_item_status', total)]}
    return {
        'run_report_sha256': run_sha, 'threshold': threshold, 'case_count': len(cases), 'item_denominator': total,
        'figure_denominator': figures, 'independently_reviewed_cases': len(reviewed), 'counts': counts,
        'rates_with_pending_in_denominator': rates,
        'independent_item_and_figure_gates_passed': all(rate is None or rate >= threshold for rate in rates.values()),
        'scope': 'Independent council source comparison; render traces are necessary, never sufficient. Worksheet export checks are separately reported.',
        'cases': pages,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--cases', type=Path, default=Path(__file__).resolve().parents[1] / 'tests/worksheet_banks/cases.json')
    parser.add_argument('--run', type=Path, required=True)
    parser.add_argument('--reference', type=Path, action='append', default=[])
    parser.add_argument('--audit', type=Path, action='append', default=[])
    parser.add_argument('--corpus', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    parser.add_argument('--threshold', type=float, default=harness.DEFAULT_THRESHOLD)
    args = parser.parse_args()
    load = lambda p: json.loads(p.read_text())
    cases = load(args.cases)['cases']
    references = [(digest(p), load(p)) for p in [args.cases, *args.reference]]
    audits = [(digest(p), load(p)) for p in args.audit]
    result = reconcile(cases, load(args.run), digest(args.run), references, audits, args.corpus, threshold=args.threshold)
    result.update(evaluator_sha256=digest(__file__), harness_sha256=digest(Path(__file__).with_name('worksheet_banks.py')),
                  producer_binary_sha256=load(args.run).get('binary_sha256'), inference_performed=False)
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps({k: v for k, v in result.items() if k != 'cases'}, indent=2))


if __name__ == '__main__':
    main()
