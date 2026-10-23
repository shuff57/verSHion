// Round 9: THE licence rule, one table. Two rounds of critics pulled opposite ways (round 7: never
// lock an honest teacher out; round 8: a verbatim read or import relabelled is a wrong public badge).
// The design resolution (license.ts decideLicence, provenance.ts bearingOf) must satisfy BOTH, so
// every earlier repro lives here with its expected result:
//
// * a copy of a source in the licence sense (same content, points / difficulty / topics aside), read
//   or imported, is hard; so is an adaptation of an import or of a declared origin; a changed copy of
//   something only read is soft (credited, flagged);
// * identical sources are UNITED (any one of them justifies a verbatim copy); distinct materials an
//   adaptation drew on are intersected; when they contradict each other the union of what any one
//   allows applies (flag license_conflict), never an empty lock-out;
// * a fixed 32-item allowance per question: the outcome is the same in any bank size and any order;
// * a republish never refuses a question its author did not touch (license_grandfathered);
// * independent_of (with a statement, audited, flagged) is the way out for reads, common questions
//   and adaptations of imports.
import assert from "node:assert/strict";
import { test } from "node:test";
import { genQuestion, publish, rng, World, type User } from "./harness.ts";
// Round 16: an adapter's explanation binds the original author when she copies it as a lift (at
// least 25 words, publish.ts LIFT_WORDS), whenever she was served it.
const LONG_S = "S's better explanation: the light reactions in the thylakoid membranes split water and make ATP and NADPH, which the Calvin cycle in the stroma then spends to fix carbon dioxide into sugar, step by step.";

const DAY = 86_400_000;
type Q = Record<string, any>;
const LIC = ["CC-BY-4.0", "CC0-1.0", "CC-BY-SA-4.0", "CC-BY-NC-4.0", "CC-BY-NC-SA-4.0"];
const bankOf = (qs: Q[]) => JSON.stringify({ schema_version: 2, bank: { id: "x", name: "b", questions: qs, created: 1, modified: 1 } });
const q = (stem: string, extra: Q = {}): Q => ({ id: "q", stem, kind: { Short: { sample: "secret answer" } }, points: null, difficulty: null, topics: [], image: null, explanation: null, figure: null, created: 1, modified: 1, ...extra });
const tf = (stem: string, correct = true): Q => q(stem, { kind: { Tf: { correct } } });
const imp = (w: World, u: User, id: string) => w.json<any>("POST", `/items/${id}/import`, { user: u });
const get = (w: World, u: User, id: string) => w.json<any>("GET", `/items/${id}`, { user: u });
const served = (r: any): Q[] => r.body.payload.bank.questions;
const withOrigin = (r: any) =>
  served(r).map((x: Q, i: number) => ({ ...x, origin: { item_id: r.body.item.id, qid: r.body.questions[i].qid, publish_id: r.body.item.publish_id, version: r.body.item.version, author_name: r.body.item.author_name, license: r.body.license, form: r.body.item.form } }));
const own = (pub: any, qs: Q[]) => qs.map((x, i) => (i < pub.qids.length ? { ...x, published: { item_id: pub.item_id, qid: pub.qids[i], publish_id: pub.publish_id, version: pub.version } } : x));
const filler = (n: number) => Array.from({ length: n }, (_, i) => tf(`Own filler statement number ${i} about igneous rocks.`, i % 2 === 0));
const flagsOf = (w: World, id: unknown) => (typeof id === "string" ? w.q<{ kind: string }>(`SELECT kind FROM admin_flags WHERE item_id = ? ORDER BY kind`, id).map((x) => x.kind) : []);
/** "201", "200", or "422[allowed...]" — what a publisher is told. */
const out = (r: { status: number; body: Q }) => (r.status === 422 && r.body.error === "license_incompatible" ? `422${JSON.stringify(r.body.allowed)}` : String(r.status));
const each = async (f: (lic: string) => Promise<{ status: number; body: Q }>) => {
  const o: string[] = [];
  for (const lic of LIC) o.push(`${lic}:${out(await f(lic))}`);
  return o;
};

interface Case {
  id: string;
  what: string;
  run: () => Promise<unknown>;
  expect: unknown;
}

const CASES: Case[] = [
  // ---- round 7 critics (lock-outs) ------------------------------------------------------------------------
  {
    id: "r7 x1",
    what: "a LATER attacker bank repeating the victim's question 40 times (SA) and an NC copy, read AND imported by the victim, never lock the victim's own question out",
    run: async () => {
      const w = new World();
      const victim = w.user("teacher", "Victim"), att = w.user("teacher", "Attacker");
      const Qv = genQuestion(rng(42), "Mc", 0).json as Q;
      await publish(w, victim, bankOf([Qv]), { license: "CC-BY-4.0" });
      w.advance(1000);
      const a1 = (await publish(w, att, bankOf(Array.from({ length: 40 }, (_, i) => ({ ...Qv, id: `dup${i}` }))), { license: "CC-BY-SA-4.0" })).body;
      const a2 = (await publish(w, att, bankOf([Qv]), { license: "CC-BY-NC-4.0" })).body;
      for (const id of [a1.item_id, a2.item_id] as string[]) await get(w, victim, id);
      const read = await each((lic) => publish(w, victim, bankOf([Qv]), { license: lic }));
      for (const id of [a1.item_id, a2.item_id] as string[]) await imp(w, victim, id);
      const imported = await each((lic) => publish(w, victim, bankOf([Qv]), { license: lic }));
      return { read, imported };
    },
    expect: { read: LIC.map((l) => `${l}:201`), imported: LIC.map((l) => `${l}:201`) },
  },
  {
    id: "r7 x1b",
    what: "...and the victim can republish their own bank (a typo fixed in another question) or attest independence",
    run: async () => {
      const w = new World();
      const victim = w.user("teacher", "Victim"), att = w.user("teacher", "Attacker");
      const Qv = genQuestion(rng(42), "Mc", 0).json as Q;
      const v1 = (await publish(w, victim, bankOf([Qv, tf("Is the sky blue?")]), { license: "CC-BY-4.0" })).body;
      w.advance(1000);
      const a1 = (await publish(w, att, bankOf(Array.from({ length: 40 }, (_, i) => ({ ...Qv, id: `dup${i}` }))), { license: "CC-BY-SA-4.0" })).body;
      await get(w, victim, a1.item_id as string);
      await w.call("DELETE", `/items/${a1.item_id}`, { user: att });
      const ind = await publish(w, victim, bankOf([Qv]), { license: "CC-BY-4.0", independent: [a1.item_id as string] });
      const rp = await publish(w, victim, bankOf(own(v1, [Qv, tf("Edited typo fix: is the sky blue?")])), { license: "CC-BY-4.0", base: v1.publish_id as string }, v1.item_id as string);
      return [out(ind), out(rp)];
    },
    expect: ["201", "200"],
  },
  {
    id: "r7 x2",
    what: "a COMMON question (5 authors) only read: no evidence, in a 10- or a 1000-question bank",
    run: async () => {
      const w = new World();
      const v = w.user("teacher", "Victim");
      const Q0 = tf("The mitochondria is the powerhouse of the cell.");
      for (let k = 0; k < 5; k++) await get(w, v, (await publish(w, w.user(), bankOf([Q0]), { license: LIC[k]! })).body.item_id as string);
      return [out(await publish(w, v, bankOf([Q0, ...filler(9)]), { license: "CC-BY-4.0" })), out(await publish(w, v, bankOf([Q0, ...filler(999)]), { license: "CC-BY-4.0" }))];
    },
    expect: ["201", "201"],
  },
  {
    id: "r7 s5",
    what: "her own version (own explanation) of a question two IMPORTED banks contain (SA first, then NC): an adaptation of imports that contradict: the EARLIER source's terms (round 10 M-1); independence or a verbatim copy",
    run: async () => {
      const w = new World();
      const S = w.user("teacher", "Sam"), N = w.user("teacher", "Nia"), T = w.user("teacher", "Tess");
      const capital = (e: string) => q("What is the capital of France?", { kind: { Short: { sample: "Paris" } }, explanation: e });
      const s = (await publish(w, S, bankOf([capital("Sam: Paris has been the capital since 987."), q("Sam's rivers question?")]), { license: "CC-BY-SA-4.0" })).body;
      const n = (await publish(w, N, bankOf([capital("Nia: Paris, on the Seine."), q("Nia's mountains question?")]), { license: "CC-BY-NC-4.0" })).body;
      w.advance(DAY);
      await imp(w, T, s.item_id as string);
      await imp(w, T, n.item_id as string);
      w.advance(DAY);
      const mine = [capital("Tess: my own explanation from my lesson."), q("Tess's deserts question?")];
      const byLicence = await each((lic) => publish(w, T, bankOf(mine), { license: lic }));
      const sa = (await publish(w, T, bankOf(mine), { license: "CC-BY-SA-4.0" })).body;
      const ind = await publish(w, T, bankOf(mine), { license: "CC-BY-4.0", independent: [s.item_id as string, n.item_id as string], statement: "I wrote this capital question myself." });
      const verbatim = await publish(w, T, bankOf([capital("Sam: Paris has been the capital since 987.")]), { license: "CC-BY-4.0" });
      return { byLicence, conflictFlag: flagsOf(w, sa.item_id).includes("license_conflict"), independent: out(ind), verbatim: [out(verbatim), (verbatim.body.per_question as Q[])[0]!.bound_by.map((b: Q) => b.author_name)] };
    },
    expect: {
      // Round 9 allowed the union [SA, NC, NC-SA]; round 10 (M-1): the earliest-published source decides.
      byLicence: ['CC-BY-4.0:422["CC-BY-SA-4.0"]', 'CC0-1.0:422["CC-BY-SA-4.0"]', "CC-BY-SA-4.0:201", 'CC-BY-NC-4.0:422["CC-BY-SA-4.0"]', 'CC-BY-NC-SA-4.0:422["CC-BY-SA-4.0"]'],
      conflictFlag: true,
      independent: "201",
      verbatim: ['422["CC-BY-SA-4.0"]', ["Sam"]],
    },
  },
  {
    id: "r7 s9",
    what: "a verbatim copy (origin declared) of an imported CC0 question stays CC0 beside 31 or 33 imported adaptations under other licences",
    run: async () => {
      const o: string[] = [];
      for (const N of [31, 33]) {
        const w = new World();
        const T = w.user("teacher", "Tess"), Z = w.user("teacher", "Zero");
        const conv = (e: string) => q("How many centimetres are in one metre?", { kind: { Short: { sample: "100" } }, explanation: e });
        const z = (await publish(w, Z, bankOf([conv("Zero's explanation"), q("Zero's units question?")]), { license: "CC0-1.0" })).body;
        const others: string[] = [];
        for (let i = 0; i < N; i++) {
          w.advance(1000);
          others.push((await publish(w, w.user(), bankOf([conv(`explanation ${i}`), q(`Author ${i}'s question ${i}?`)]), { license: LIC[(i % 4) + 1]! })).body.item_id as string);
        }
        w.advance(DAY);
        const iz = await imp(w, T, z.item_id as string);
        for (const id of others) await imp(w, T, id);
        o.push(`${N}:${out(await publish(w, T, bankOf(withOrigin(iz)), { license: "CC0-1.0" }))}/${out(await publish(w, T, bankOf(withOrigin(iz)), { license: "CC-BY-4.0" }))}`);
      }
      return o;
    },
    expect: ["31:201/201", "33:201/201"],
  },
  {
    id: "r7 s9c",
    what: "...in a bank of 1000 or of 11, with 4 or 5 sources",
    run: async () => {
      const o: string[] = [];
      for (const [size, nsrc] of [[1000, 4], [1000, 5], [11, 5]] as const) {
        const w = new World();
        const T = w.user("teacher", "Tess"), Z = w.user("teacher", "Zero");
        const c = (e: string) => q("What is the chemical symbol for sodium?", { kind: { Short: { sample: "Na" } }, explanation: e });
        const z = (await publish(w, Z, bankOf([c("Zero's"), q("Zero's periodic table question?")]), { license: "CC0-1.0" })).body;
        const ids: string[] = [];
        for (let i = 0; i < nsrc - 1; i++) {
          w.advance(1000);
          ids.push((await publish(w, w.user(), bankOf([c(`expl ${i}`), q(`A${i}'s metals question ${i}?`)]), { license: LIC[(i % 4) + 1]! })).body.item_id as string);
        }
        w.advance(DAY);
        const iz = await imp(w, T, z.item_id as string);
        for (const id of ids) await imp(w, T, id);
        o.push(`${size}/${nsrc}:${out(await publish(w, T, bankOf([withOrigin(iz)[0], ...filler(size - 1)]), { license: "CC0-1.0" }))}`);
      }
      return o;
    },
    expect: ["1000/4:201", "1000/5:201", "11/5:201"],
  },
  // ---- round 8 critics (laundering) -----------------------------------------------------------------------
  {
    id: "r8 s1 (J1)",
    what: "an imported NC question: verbatim, points changed, hidden key, or an explanation added: never CC0; any bank size",
    run: async () => {
      const w = new World();
      const A = w.user("teacher", "Alice"), T = w.user("teacher", "Tom");
      const Q0 = q("Explain why the sky appears blue during the day in two sentences.", { points: 1 });
      const a = (await publish(w, A, bankOf([Q0]), { license: "CC-BY-NC-4.0" })).body;
      w.advance(DAY);
      const ia = await imp(w, T, a.item_id as string);
      const s0 = served(ia)[0]!;
      return {
        may: ia.body.may_republish_as,
        verbatim: out(await publish(w, T, bankOf([s0]), { license: "CC0-1.0" })),
        points: out(await publish(w, T, bankOf([{ ...s0, points: 2 }]), { license: "CC0-1.0" })),
        pointsBig: out(await publish(w, T, bankOf([{ ...s0, points: 2, difficulty: "High", topics: ["optics"] }, ...filler(999)]), { license: "CC0-1.0" })),
        hidden: out(await publish(w, T, bankOf([s0]), { license: "CC0-1.0", hideKey: true })),
        adapted: out(await publish(w, T, bankOf([{ ...s0, explanation: "Tom's explanation" }]), { license: "CC0-1.0" })),
        adaptedNcSa: out(await publish(w, T, bankOf([{ ...s0, explanation: "Tom's explanation" }]), { license: "CC-BY-NC-SA-4.0" })),
      };
    },
    expect: { may: ["CC-BY-NC-4.0"], verbatim: '422["CC-BY-NC-4.0"]', points: '422["CC-BY-NC-4.0"]', pointsBig: '422["CC-BY-NC-4.0"]', hidden: '422["CC-BY-NC-4.0"]', adapted: '422["CC-BY-NC-4.0","CC-BY-NC-SA-4.0"]', adaptedNcSa: "201" },
  },
  {
    id: "r8 s3 (J1)",
    what: "a plain READ copied verbatim (NC, SA) as CC0 is refused like an import; a changed copy of a read is soft (published, flagged)",
    run: async () => {
      const o: Record<string, unknown> = {};
      for (const lic of ["CC-BY-NC-4.0", "CC-BY-SA-4.0"]) {
        const w = new World();
        const A = w.user("teacher", "Alice"), T = w.user("teacher", "Tom");
        const Q0 = q("Describe the three main stages of cellular respiration in order.", { explanation: "Alice's detailed explanation." });
        const a = (await publish(w, A, bankOf([Q0]), { license: lic })).body;
        w.advance(DAY);
        const g = await get(w, T, a.item_id as string);
        const v = await publish(w, T, bankOf([served(g)[0]]), { license: "CC0-1.0" });
        const ch = await publish(w, T, bankOf([{ ...served(g)[0], explanation: "Tom's own words entirely." }]), { license: "CC0-1.0" });
        o[lic] = [out(v), v.body.per_question?.[0]?.bound_by?.map((b: Q) => b.author_name), out(ch), flagsOf(w, ch.body.item_id).includes("license_soft")];
      }
      return o;
    },
    expect: { "CC-BY-NC-4.0": ['422["CC-BY-NC-4.0"]', ["Alice"], "201", true], "CC-BY-SA-4.0": ['422["CC-BY-SA-4.0"]', ["Alice"], "201", true] },
  },
  {
    id: "r8 s1/S2 + J3",
    what: "a CC0 original and an NC-SA verbatim relabel, both imported: a copy identical to them may be CC0 (no origin, origin = the CC0 item, or independent of the relabel)",
    run: async () => {
      const w = new World();
      const Z = w.user("teacher", "Zero"), B = w.user("teacher", "Bob"), T = w.user("teacher", "Tom");
      const Q0 = q("What is the boiling point of water at sea level in Celsius?", { kind: { Short: { sample: "100" } } });
      const z = (await publish(w, Z, bankOf([Q0]), { license: "CC0-1.0" })).body;
      w.advance(DAY);
      const bz = await imp(w, B, z.item_id as string);
      const b = await publish(w, B, bankOf(withOrigin(bz)), { license: "CC-BY-NC-SA-4.0" });
      w.advance(DAY);
      const tz = await imp(w, T, z.item_id as string);
      const tb = await imp(w, T, b.body.item_id as string);
      return {
        relabel: out(b),
        mayB: tb.body.may_republish_as,
        noOrigin: out(await publish(w, T, bankOf([served(tz)[0]]), { license: "CC0-1.0" })),
        originZero: out(await publish(w, T, bankOf(withOrigin(tz)), { license: "CC0-1.0" })),
        indepB: out(await publish(w, T, bankOf([served(tz)[0]]), { license: "CC0-1.0", independent: [b.body.item_id as string] })),
        originBob: await each((lic) => publish(w, T, bankOf(withOrigin(tb)), { license: lic })),
      };
    },
    // A copy of Bob's relabel IS a copy of Zero's CC0 question too (same content): any licence.
    expect: { relabel: "201", mayB: [...LIC, "circle-only"], noOrigin: "201", originZero: "201", indepB: "201", originBob: LIC.map((l) => `${l}:201`) },
  },
  {
    id: "r8 com (J3)",
    what: "five identical sources under five licences, all imported: the earliest-published one (CC-BY) is the original and decides, never allowed []; in either import order (round 10: later copies cannot widen it)",
    run: async () => {
      const o: string[][] = [];
      for (const order of ["forward", "reverse"]) {
        const w = new World();
        const T = w.user("teacher", "Tess");
        const C = q("What is 7 times 8?", { kind: { Short: { sample: "56" } } });
        const ids: string[] = [];
        for (let k = 0; k < 5; k++) {
          ids.push((await publish(w, w.user(), bankOf([C, q(`Author ${k}'s distinctive question on fractions?`)]), { license: LIC[k]! })).body.item_id as string);
          w.advance(1000);
        }
        w.advance(DAY);
        for (const id of order === "forward" ? ids : [...ids].reverse()) await imp(w, T, id);
        w.advance(DAY);
        o.push(await each((lic) => publish(w, T, bankOf([C, q("Tess's own question about long division?")]), { license: lic })));
      }
      return o;
    },
    expect: [LIC.map((l) => (l === "CC-BY-4.0" ? `${l}:201` : `${l}:422["CC-BY-4.0"]`)), LIC.map((l) => (l === "CC-BY-4.0" ? `${l}:201` : `${l}:422["CC-BY-4.0"]`))],
  },
  {
    id: "r8 lock (J3)",
    what: "copying an original verbatim beside a relabel of it: CC0 original -> any licence; CC-BY original -> CC-BY only (a verbatim CC-BY copy keeps its licence)",
    run: async () => {
      const o: Record<string, string[]> = {};
      for (const [orig, relabel] of [["CC0-1.0", "CC-BY-4.0"], ["CC-BY-4.0", "CC-BY-NC-4.0"]] as const) {
        const w = new World();
        const B = w.user("teacher", "Bea"), A = w.user("teacher", "Ann"), T = w.user("teacher", "Tom");
        const Q0 = q("Name the three branches of the United States federal government.", { kind: { Short: { sample: "Legislative, executive, judicial" } } });
        const b = (await publish(w, B, bankOf([Q0]), { license: orig })).body;
        w.advance(DAY);
        const ab = await imp(w, A, b.item_id as string);
        const a = await publish(w, A, bankOf(withOrigin(ab)), { license: relabel });
        w.advance(DAY);
        const tb = await imp(w, T, b.item_id as string);
        if (a.status === 201) await imp(w, T, a.body.item_id as string);
        o[`${orig}+${relabel}:${a.status}`] = await each((lic) => publish(w, T, bankOf(withOrigin(tb)), { license: lic }));
      }
      return o;
    },
    expect: {
      "CC0-1.0+CC-BY-4.0:201": LIC.map((l) => `${l}:201`),
      "CC-BY-4.0+CC-BY-NC-4.0:422": LIC.map((l) => (l === "CC-BY-4.0" ? `${l}:201` : `${l}:422["CC-BY-4.0"]`)),
    },
  },
  {
    id: "r9 contradiction",
    what: "a copy of a source whose own grants contradict (a soft adaptation of an NC read published as CC-BY): the older terms (its NC credit) win, never CC0 or CC-BY; flagged",
    run: async () => {
      const w = new World();
      const A = w.user("teacher", "Alice"), T = w.user("teacher", "Tom"), U = w.user("teacher", "Uma");
      const a = (await publish(w, A, bankOf([q("Explain how glaciers carve U-shaped valleys over time.")]), { license: "CC-BY-NC-4.0" })).body;
      w.advance(DAY);
      const g = await get(w, T, a.item_id as string);
      const t = await publish(w, T, bankOf([{ ...served(g)[0], explanation: "Tom's own worked explanation." }]), { license: "CC-BY-4.0" });
      w.advance(DAY);
      const iu = await imp(w, U, t.body.item_id as string);
      const by = await publish(w, U, bankOf(withOrigin(iu)), { license: "CC-BY-NC-4.0" });
      return { tom: [out(t), flagsOf(w, t.body.item_id).includes("license_soft")], uma: await each((lic) => publish(w, U, bankOf(withOrigin(iu)), { license: lic })), flagged: flagsOf(w, by.body.item_id).includes("license_conflict") };
    },
    expect: {
      tom: ["201", true],
      uma: ['CC-BY-4.0:422["CC-BY-NC-4.0","CC-BY-NC-SA-4.0"]', 'CC0-1.0:422["CC-BY-NC-4.0","CC-BY-NC-SA-4.0"]', 'CC-BY-SA-4.0:422["CC-BY-NC-4.0","CC-BY-NC-SA-4.0"]', "CC-BY-NC-4.0:201", "CC-BY-NC-SA-4.0:201"],
      flagged: true,
    },
  },
  {
    id: "r8 rev (J2)",
    what: "a republish never refuses a question its author did not touch; a touched one is checked",
    run: async () => {
      const o: Record<string, unknown> = {};
      // (a) the critic's own flow: points changed is a copy now (J1), refused at the first publish.
      // (b) an adaptation of an IMPORT published with an independence claim, then a typo fixed in
      //     ANOTHER question without restating it: grandfathered (and flagged); reverting the adapted
      //     question to verbatim touches it: refused.
      // (c) an adaptation of a READ (soft), republished: the server's record of its source keeps
      //     the bearing it had (soft).
      for (const mode of ["import", "get"]) {
        const w = new World();
        const A = w.user("teacher", "Alice"), T = w.user("teacher", "Tom");
        const Q0 = q("Explain the difference between weather and climate in your own words.", { points: 1 });
        const a = (await publish(w, A, bankOf([Q0]), { license: "CC-BY-NC-4.0" })).body;
        w.advance(DAY);
        const ia = mode === "import" ? await imp(w, T, a.item_id as string) : await get(w, T, a.item_id as string);
        const mine2 = q("Tom's own question about the greenhouse effect?");
        const points = await publish(w, T, bankOf([{ ...served(ia)[0], points: 3 }, mine2]), { license: "CC-BY-4.0" });
        const adapted = { ...served(ia)[0], explanation: "Tom's adaptation: a worked example." };
        const p1 = await publish(w, T, bankOf([adapted, mine2]), { license: "CC-BY-4.0", ...(mode === "import" ? { independent: [a.item_id as string] } : {}) });
        w.advance(DAY);
        const p2 = await publish(w, T, bankOf(own(p1.body, [adapted, q("Tom's own question about the greenhouse effect, typo fixed?")])), { license: "CC-BY-4.0", base: p1.body.publish_id as string }, p1.body.item_id as string);
        const p3 = await publish(w, T, bankOf(own(p2.body, [served(ia)[0], q("Tom's own question about the greenhouse effect, typo fixed?")])), { license: "CC-BY-4.0", base: p2.body.publish_id as string }, p1.body.item_id as string);
        o[mode] = { points: out(points), first: out(p1), typoElsewhere: out(p2), grandfatheredFlag: flagsOf(w, p1.body.item_id).includes("license_grandfathered"), revertToVerbatim: out(p3) };
      }
      return o;
    },
    expect: {
      import: { points: '422["CC-BY-NC-4.0"]', first: "201", typoElsewhere: "200", grandfatheredFlag: true, revertToVerbatim: '422["CC-BY-NC-4.0"]' },
      get: { points: '422["CC-BY-NC-4.0"]', first: "201", typoElsewhere: "200", grandfatheredFlag: false, revertToVerbatim: '422["CC-BY-NC-4.0"]' },
    },
  },
  // ---- round 10 critics ------------------------------------------------------------------------------------
  {
    id: "r10 s8 (M-1)",
    what: "an adaptation of an imported NC question beside later CC0 and SA variants: more imports never widen it (CC0 and CC-BY refused, NC kept)",
    run: async () => {
      const w = new World();
      const A = w.user("teacher", "A"), B = w.user("teacher", "B"), C = w.user("teacher", "C"), M = w.user("teacher", "M");
      const X = (e: string) => q("Explain why the Krebs cycle in Alice's unique wording produces three NADH per turn.", { explanation: e });
      const ids: string[] = [];
      for (const [u, e, lic] of [[A, "Alice E1", "CC-BY-NC-4.0"], [B, "B's E", "CC0-1.0"], [C, "C's E", "CC-BY-SA-4.0"]] as const) {
        ids.push((await publish(w, u, bankOf([X(e)]), { license: lic })).body.item_id as string);
        w.advance(DAY);
      }
      for (const id of ids) await imp(w, M, id);
      return await each((lic) => publish(w, M, bankOf([X("Alice E1, slightly reworded by M")]), { license: lic }));
    },
    expect: ['CC-BY-4.0:422["CC-BY-NC-4.0","CC-BY-NC-SA-4.0"]', 'CC0-1.0:422["CC-BY-NC-4.0","CC-BY-NC-SA-4.0"]', 'CC-BY-SA-4.0:422["CC-BY-NC-4.0","CC-BY-NC-SA-4.0"]', "CC-BY-NC-4.0:201", "CC-BY-NC-SA-4.0:201"],
  },
  {
    id: "r10 s1 (M-2)",
    what: "a READ copied verbatim stays bound when honest importers copied it too: 'common' counts independent authors (lineage roots) only",
    run: async () => {
      const w = new World();
      const A = w.user("teacher", "A"), C = w.user("teacher", "C"), D = w.user("teacher", "D"), E = w.user("teacher", "E"), M = w.user("teacher", "M");
      const X = q("Explain why the Krebs cycle in Alice's unique wording produces three NADH per turn.", { explanation: "Alice's long explanation E1." });
      const a = (await publish(w, A, bankOf([X]), { license: "CC-BY-NC-4.0" })).body;
      w.advance(DAY);
      for (const U of [C, D, E]) {
        await publish(w, U, bankOf(withOrigin(await imp(w, U, a.item_id as string))), { license: "CC-BY-NC-4.0" });
        w.advance(DAY);
      }
      const r = await get(w, M, a.item_id as string);
      return await each((lic) => publish(w, M, bankOf(served(r)), { license: lic }));
    },
    expect: LIC.map((l) => (l === "CC-BY-NC-4.0" ? `${l}:201` : l === "CC-BY-NC-SA-4.0" ? `${l}:422["CC-BY-NC-4.0"]` : `${l}:422["CC-BY-NC-4.0"]`)),
  },
  {
    id: "r10 s2 (M-3)",
    what: "trivial edits of a read (trailing period, space, NBSP, case, dropped explanation) stay copies; a one-word stem edit is flagged origin_suspect, soft for a read, an ADAPTATION of an import",
    run: async () => {
      const o: Record<string, unknown> = {};
      const X = () => q("Explain why the Krebs cycle in Alice's unique wording produces three NADH per turn.", { explanation: "Alice's long explanation E1." });
      const edits: [string, (x: Q) => Q][] = [
        ["period", (x) => ({ ...x, explanation: x.explanation + "." })],
        ["space", (x) => ({ ...x, explanation: x.explanation + " " })],
        ["nbsp", (x) => ({ ...x, explanation: (x.explanation as string).replace(" ", "\u00a0") })],
        ["case", (x) => ({ ...x, stem: (x.stem as string).toUpperCase() })],
        ["dropExpl", (x) => ({ ...x, explanation: null })],
      ];
      for (const [nm, ed] of edits) {
        const w = new World();
        const A = w.user("teacher", "A"), M = w.user("teacher", "M");
        const a = (await publish(w, A, bankOf([X()]), { license: "CC-BY-NC-4.0" })).body;
        w.advance(DAY);
        const r = await get(w, M, a.item_id as string);
        o[nm] = out(await publish(w, M, bankOf(served(r).map(ed)), { license: "CC0-1.0" }));
      }
      for (const how of ["read", "import"]) {
        const w = new World();
        const A = w.user("teacher", "A"), M = w.user("teacher", "M");
        const a = (await publish(w, A, bankOf([X()]), { license: "CC-BY-NC-4.0" })).body;
        w.advance(DAY);
        const r = how === "read" ? await get(w, M, a.item_id as string) : await imp(w, M, a.item_id as string);
        const p = await publish(w, M, bankOf(served(r).map((x) => ({ ...x, stem: (x.stem as string).replace("produces", "yields") }))), { license: "CC0-1.0" });
        o[`stem-${how}`] = [out(p), p.status < 300 ? flagsOf(w, p.body.item_id).includes("origin_suspect") : null];
      }
      return o;
    },
    expect: { period: '422["CC-BY-NC-4.0"]', space: '422["CC-BY-NC-4.0"]', nbsp: '422["CC-BY-NC-4.0"]', case: '422["CC-BY-NC-4.0"]', dropExpl: '422["CC-BY-NC-4.0"]', "stem-read": ["201", true], "stem-import": ['422["CC-BY-NC-4.0","CC-BY-NC-SA-4.0"]', null] },
  },
  {
    id: "r10 s4 (M-4)",
    what: "the author relicenses the SAME content NC -> CC-BY; after a re-import an adaptation may be CC-BY (identical content across versions is united); the import response agrees",
    run: async () => {
      const w = new World();
      const A = w.user("teacher", "A"), M = w.user("teacher", "M");
      const X = q("Explain why the Krebs cycle in Alice's unique wording produces three NADH per turn.", { explanation: "E1" });
      const a = (await publish(w, A, bankOf([X]), { license: "CC-BY-NC-4.0" })).body;
      w.advance(DAY);
      await imp(w, M, a.item_id as string);
      await publish(w, A, bankOf([{ ...X, published: { item_id: a.item_id, qid: (a.qids as string[])[0], publish_id: a.publish_id, version: 1 } }]), { license: "CC-BY-4.0", base: a.publish_id as string }, a.item_id as string);
      w.advance(DAY);
      const r = await imp(w, M, a.item_id as string);
      return {
        may: r.body.may_republish_as,
        verbatim: await each((lic) => publish(w, M, bankOf(served(r)), { license: lic })),
        adapted: out(await publish(w, M, bankOf(served(r).map((x) => ({ ...x, explanation: "M's own better explanation." }))), { license: "CC-BY-4.0" })),
      };
    },
    expect: {
      may: ["CC-BY-4.0", "CC-BY-NC-4.0"],
      verbatim: ["CC-BY-4.0:201", 'CC0-1.0:422["CC-BY-4.0","CC-BY-NC-4.0"]', 'CC-BY-SA-4.0:422["CC-BY-4.0","CC-BY-NC-4.0"]', "CC-BY-NC-4.0:201", 'CC-BY-NC-SA-4.0:422["CC-BY-4.0","CC-BY-NC-4.0"]'],
      adapted: "201",
    },
  },
  {
    id: "r10 s4 show/hide",
    what: "a read of a hidden-key NC question with the publisher's own key: shown or hidden, the same outcome (hard)",
    run: async () => {
      const o: string[] = [];
      for (const hide of [false, true]) {
        const w = new World();
        const A = w.user("teacher", "A"), M = w.user("teacher", "M");
        const a = (await publish(w, A, bankOf([q("Explain why the Krebs cycle in Alice's unique wording produces three NADH per turn.", { explanation: "E1" })]), { license: "CC-BY-NC-4.0", hideKey: true })).body;
        w.advance(DAY);
        const s0 = served(await get(w, M, a.item_id as string))[0]!;
        const { keyless: _k, ...rest } = s0;
        o.push(out(await publish(w, M, bankOf([{ ...rest, kind: { Short: { sample: "mine" } } }]), { license: "CC0-1.0", hideKey: hide })));
      }
      return o;
    },
    expect: ['422["CC-BY-NC-4.0"]', '422["CC-BY-NC-4.0"]'],
  },
  {
    id: "r10 s20/s21 (M-5)",
    what: "the original author is never locked into a copier's licence: own hidden original shown later, own purged original republished; but taking a copier's EXPLANATION still binds",
    run: async () => {
      const own = (pub: any, qs: Q[]) => qs.map((x, i) => ({ ...x, published: { item_id: pub.item_id, qid: pub.qids[i], publish_id: pub.publish_id, version: pub.version ?? 1 } }));
      const o: Record<string, unknown> = {};
      for (const how of ["read", "import"] as const) {
        const w = new World();
        const T = w.user("teacher", "T"), S = w.user("teacher", "S");
        const st = tf("Teacher T's statement: the mitochondrial matrix hosts the Krebs cycle reactions in cells.", true);
        const t = (await publish(w, T, bankOf([st]), { license: "CC-BY-NC-4.0", hideKey: true })).body;
        w.advance(DAY);
        const sq = served(await imp(w, S, t.item_id as string)).map(({ keyless: _k, ...x }: Q) => ({ ...x, kind: { Tf: { correct: true } } }));
        // S adds only the key (a copy of T's NC item: S may publish it as NC only).
        const s = (await publish(w, S, bankOf(sq), { license: "CC-BY-NC-4.0" })).body;
        assert.ok(s.item_id, "S's copy is published");
        w.advance(DAY);
        how === "read" ? await get(w, T, s.item_id as string) : await imp(w, T, s.item_id as string);
        o[`hidden-${how}`] = out(await publish(w, T, bankOf(own(t, [st])), { license: "CC-BY-4.0", base: t.publish_id as string }, t.item_id as string));
      }
      {
        const w = new World();
        const T = w.user("teacher", "T"), S = w.user("teacher", "S");
        const Q0 = q("Teacher T's original question on why the Krebs cycle produces three NADH per turn.", { explanation: "T's explanation" });
        const t = (await publish(w, T, bankOf([Q0]), { license: "CC-BY-NC-4.0" })).body;
        w.advance(DAY);
        const s = (await publish(w, S, bankOf(withOrigin(await imp(w, S, t.item_id as string))), { license: "CC-BY-NC-4.0" })).body;
        w.advance(DAY);
        await w.json("DELETE", `/items/${t.item_id}`, { user: T });
        w.advance(40 * DAY);
        await w.pool.daily();
        await get(w, T, s.item_id as string);
        o.purged = out(await publish(w, T, bankOf([Q0]), { license: "CC-BY-4.0" }));
      }
      {
        const w = new World();
        const T = w.user("teacher", "T"), S = w.user("teacher", "S");
        const Q0 = q("Teacher T's original question on why the Krebs cycle produces three NADH per turn.", { explanation: "T's explanation" });
        const t = (await publish(w, T, bankOf([Q0]), { license: "CC-BY-NC-4.0" })).body;
        w.advance(DAY);
        const si = await imp(w, S, t.item_id as string);
        const s = (await publish(w, S, bankOf([{ ...withOrigin(si)[0], explanation: LONG_S }]), { license: "CC-BY-NC-SA-4.0" })).body;
        w.advance(DAY);
        await imp(w, T, s.item_id as string);
        o.takesExplanation = out(await publish(w, T, bankOf([{ ...Q0, explanation: LONG_S }]), { license: "CC-BY-4.0" }));
      }
      return o;
    },
    expect: { "hidden-read": "200", "hidden-import": "200", purged: "201", takesExplanation: '422["CC-BY-NC-SA-4.0"]' },
  },
  {
    id: "r10 H4 (M-5 short)",
    what: "a short common statement imported with someone's bank: refused as a copy, but the publisher may attest independence (flagged)",
    run: async () => {
      const w = new World();
      const T = w.user("teacher", "T"), B = w.user("teacher", "B");
      const st = tf("The Earth orbits the Sun once every year.", true);
      const b = (await publish(w, B, bankOf([st, tf("B's statement about magnets attracting iron filings strongly.")]), { license: "CC-BY-NC-SA-4.0" })).body;
      w.advance(DAY);
      await imp(w, T, b.item_id as string);
      w.advance(DAY);
      const mine = [st, tf("T's statement about plants needing light for growth.")];
      const ind = await publish(w, T, bankOf(mine), { license: "CC-BY-4.0", independent: [b.item_id as string] });
      return [out(await publish(w, T, bankOf(mine), { license: "CC-BY-4.0" })), out(ind), flagsOf(w, ind.body.item_id).includes("independent_claim")];
    },
    expect: ['422["CC-BY-NC-SA-4.0"]', "201", true],
  },
];

for (const c of CASES)
  test(`licence case ${c.id}: ${c.what}`, async () => {
    assert.deepEqual(await c.run(), c.expect);
  });

test("licence property: for random sources (read / imported, identical / changed, any licence), the outcome is the same in any bank size and any order", async () => {
  for (let seed = 1; seed <= 3; seed++) {
    const outs: string[] = [];
    for (const [size, reverse] of [[1, false], [1, true], [100, false], [1000, true]] as const) {
      const r = rng(seed * 104729);
      const w = new World();
      const T = w.user("teacher", "Tess");
      const Qn = q("How many grams are in one kilogram?", { kind: { Short: { sample: "1000" } } });
      const plan: { id: string; how: string }[] = [];
      for (let k = 0, n = 2 + r.int(8); k < n; k++) {
        const v = r.int(3);
        const content = v === 0 ? Qn : v === 1 ? { ...Qn, points: 1 + r.int(3) } : { ...Qn, explanation: `explanation ${k}` };
        plan.push({ id: (await publish(w, w.user(), bankOf([content]), { license: r.pick(LIC) })).body.item_id as string, how: r.pick(["import", "get"]) });
        w.advance(1000);
      }
      for (const p of reverse ? [...plan].reverse() : plan) await (p.how === "import" ? imp(w, T, p.id) : get(w, T, p.id));
      const subject = r.bool() ? Qn : { ...Qn, explanation: "Tess's own" };
      outs.push((await each((lic) => publish(w, T, bankOf([subject, ...filler(size - 1)]), { license: lic }))).join(" "));
    }
    assert.ok(outs.every((x) => x === outs[0]), `seed ${seed}:\n${outs.join("\n")}`);
    assert.ok(!outs[0]!.includes("422[]"), `never an empty lock-out: ${outs[0]}`);
  }
});

/** What a publisher may choose for `payload` in a fresh world built by `build` (CC0 tried first: a 422 lists the rest). */
async function allowedIn(build: () => Promise<{ w: World; T: User; payload: string }>): Promise<string[]> {
  const { w, T, payload } = await build();
  const r = await publish(w, T, payload, { license: "CC0-1.0" });
  return r.status === 201 ? ["ALL"] : r.status === 422 ? (r.body.allowed as string[]) : [`ERR${r.status}`];
}

test("monotonicity (round 10 M-1): adding a LATER-published source to the imports or reads never widens the allowed set (adaptations and copies alike)", async () => {
  for (let seed = 1; seed <= 6; seed++) {
    const r0 = rng(seed * 7717);
    const n = 1 + r0.int(4);
    const plan = Array.from({ length: n + 1 }, () => ({ lic: r0.pick(LIC), same: r0.bool(0.4), how: r0.pick(["import", "get"]) }));
    const subjectSame = r0.bool();
    const X = (e: string | null) => q("Describe in your own words how the water cycle moves water between the oceans and land.", { explanation: e });
    const build = (k: number) => async () => {
      const w = new World();
      const T = w.user("teacher", "Tess");
      for (let i = 0; i < k; i++) {
        const p = plan[i]!;
        const id = (await publish(w, w.user(), bankOf([X(p.same ? "the shared explanation" : `explanation ${i}`)]), { license: p.lic })).body.item_id as string;
        w.advance(DAY);
        await (p.how === "import" ? imp(w, T, id) : get(w, T, id));
      }
      return { w, T, payload: bankOf([X(subjectSame ? "the shared explanation" : "Tess's own explanation")]) };
    };
    const before = await allowedIn(build(n));
    const after = await allowedIn(build(n + 1));
    const all = (x: string[]) => (x[0] === "ALL" ? [...LIC] : x);
    assert.ok(all(after).every((l) => all(before).includes(l)), `seed ${seed}: ${JSON.stringify(plan)} subjectSame=${subjectSame}\n before ${before}\n after  ${after}`);
  }
});

test("version histories (round 10 M-4): the import response's may_republish_as is exactly what a verbatim copy of the import may be published as", async () => {
  for (let seed = 1; seed <= 6; seed++) {
    const r = rng(seed * 3301);
    const w = new World();
    const A = w.user("teacher", "A"), M = w.user("teacher", "M");
    const X = (e: string) => q("Explain how plate tectonics produce mountains, trenches and volcanoes along margins.", { explanation: e });
    let lic = r.pick(LIC);
    let expl = "v0";
    let a = (await publish(w, A, bankOf([X(expl)]), { license: lic })).body as any;
    for (let v = 0; v < 1 + r.int(4); v++) {
      w.advance(DAY);
      if (r.bool()) await imp(w, M, a.item_id);
      lic = r.bool() ? lic : r.pick(LIC);
      expl = r.bool() ? expl : `v${v + 1}`;
      const rp = await publish(w, A, bankOf([{ ...X(expl), published: { item_id: a.item_id, qid: a.qids[0], publish_id: a.publish_id, version: a.version } }]), { license: lic, base: a.publish_id }, a.item_id);
      if (rp.status === 200) a = { ...a, ...rp.body };
    }
    w.advance(DAY);
    const ir = await imp(w, M, a.item_id);
    const may = (ir.body.may_republish_as as string[]).filter((l) => l !== "circle-only");
    const accepted: string[] = [];
    for (const l of LIC) if ((await publish(w, M, bankOf(served(ir)), { license: l })).status === 201) accepted.push(l);
    assert.deepEqual(accepted, LIC.filter((l) => may.includes(l)), `seed ${seed}`);
  }
});

// ---- round 11 (critics 15a / 15b), inverted ---------------------------------------------------------------

const mcq = (stem: string, answers: [string, boolean][], extra: Q = {}): Q => q(stem, { kind: { Mc: { answers: answers.map(([text, correct]) => ({ text, correct, locked: false })) } }, ...extra });
const Qa = () => mcq("Which organelle is the main site of aerobic respiration in eukaryotic cells?", [["Mitochondrion", true], ["Ribosome", false], ["Golgi body", false]]);
const chainOf = async (w: World, u: User, id: unknown) => ((await get(w, u, id as string)).body.questions as Q[]).map((x) => (x.chain as Q[]).map((l) => l.item_id));
const curOf = (w: World, id: unknown) => w.q<{ current_publish_id: string }>(`SELECT current_publish_id FROM items WHERE id = ?`, id)[0]!.current_publish_id;

test("P1 (eE2, eE_fuzzy_frontrun): the original author's own lineage wins over a later copier: reword, case, punctuation, unchanged, a new item", async () => {
  const stem = "Describe how the water cycle moves water between the oceans the air and the land";
  const edits: [string, string][] = [
    ["reword", "Describe how the water cycle moves water between the oceans the air and the soil"],
    ["case-only", stem.toUpperCase()],
    ["punctuation", "Describe how the water cycle moves water between the oceans, the air and the land."],
    ["unchanged", stem],
  ];
  for (const [name, edited] of edits) {
    const w = new World();
    const V = w.user("teacher", "Original"), X = w.user("teacher", "Copier"), Y = w.user("teacher", "Reader");
    const v = (await publish(w, V, bankOf([q(stem)]), { license: "CC-BY-4.0" })).body;
    w.advance(DAY);
    // The copier took the text outside the pool (round 12 M5: a verbatim copy of a read can no
    // longer be attested independent), so its chain does not lead back to the original.
    const x = (await publish(w, X, bankOf([q(stem)]), { license: "CC-BY-NC-SA-4.0" })).body;
    assert.ok(x.item_id, `${name}: the copier publishes`);
    w.advance(DAY);
    await get(w, V, x.item_id as string); // the original author looks at the copy
    if (name === "reword") await imp(w, V, x.item_id as string); // ... or even imports it: still theirs first
    const own0 = served(await get(w, V, v.item_id as string))[0]!;
    const rp = await publish(w, V, bankOf([{ ...own0, stem: edited }]), { base: curOf(w, v.item_id), license: "CC-BY-4.0" }, v.item_id as string);
    assert.equal(rp.status, 200, `${name}: never refused (bound by the later copier)`);
    assert.deepEqual(await chainOf(w, Y, v.item_id), [[]], `${name}: the copier is never credited on the original`);
    assert.deepEqual((await get(w, Y, v.item_id as string)).body.license_check, { status: "ok", basis: "recorded_sources" });
    assert.ok(!flagsOf(w, v.item_id).includes("origin_suspect"), `${name}: no false origin_suspect on the original author`);
    // The same edit in a NEW item of the original author: their own earlier stem decides too.
    const n = await publish(w, V, bankOf([q(edited)]), { license: "CC-BY-4.0" });
    assert.equal(n.status, 201, `${name}: new item`);
    assert.deepEqual(await chainOf(w, Y, n.body.item_id), [[]], `${name}: new item, no copier credit`);
    assert.ok(!flagsOf(w, n.body.item_id).includes("origin_suspect"), `${name}: new item, no origin_suspect`);
    // A FRESH reword in another new item (never published before): their own question, same body.
    if (name === "reword") {
      const fresh = await publish(w, V, bankOf([q("Describe how the water cycle moves water between the oceans the air and the ground")]), { license: "CC-BY-4.0" });
      assert.deepEqual(await chainOf(w, Y, fresh.body.item_id), [[]], "fresh reword: no copier credit");
      assert.ok(!flagsOf(w, fresh.body.item_id).includes("origin_suspect"), "fresh reword: no origin_suspect");
    }
  }
  // Inverted: the COPIER's reword of what it read is still credited and flagged.
  const w = new World();
  const V = w.user("teacher", "Original"), X = w.user("teacher", "Copier");
  const v = (await publish(w, V, bankOf([q(stem)]), { license: "CC-BY-4.0" })).body;
  w.advance(DAY);
  await get(w, X, v.item_id as string);
  const x = await publish(w, X, bankOf([q("Describe how the water cycle moves water between the oceans the air and the soil")]), { license: "CC0-1.0" });
  assert.equal(x.status, 201);
  assert.deepEqual(await chainOf(w, w.user(), x.body.item_id), [[v.item_id]]);
  assert.ok(flagsOf(w, x.body.item_id).includes("origin_suspect"));
});

test("P5 (s1): license_check agrees with the licence rule at publish (recorded sources), stored per question", async () => {
  // S1: CC0 original; Bob relabels verbatim to NC-SA; Cat read both and copies verbatim as CC0.
  {
    const w = new World();
    const A = w.user("teacher", "Ann"), B = w.user("teacher", "Bob"), C = w.user("teacher", "Cat"), R = w.user();
    const a = (await publish(w, A, bankOf([Qa()]), { license: "CC0-1.0" })).body;
    w.advance(DAY);
    const b = await publish(w, B, bankOf(served(await get(w, B, a.item_id as string))), { license: "CC-BY-NC-SA-4.0" });
    assert.equal(b.status, 201);
    w.advance(DAY);
    await get(w, C, a.item_id as string);
    await get(w, C, b.body.item_id as string);
    const c = await publish(w, C, bankOf([Qa()]), { license: "CC0-1.0" });
    assert.equal(c.status, 201, "identical sources are united: the CC0 original allows CC0");
    for (const id of [b.body.item_id, c.body.item_id]) assert.deepEqual((await get(w, R, id as string)).body.license_check, { status: "ok", basis: "recorded_sources" }, "no public accusation");
  }
  // S2: Ann relicenses BY -> NC; Cat read both versions; a verbatim copy as BY and as NC both read ok.
  {
    const w = new World();
    const A = w.user("teacher", "Ann"), C = w.user("teacher", "Cat"), R = w.user();
    const a = (await publish(w, A, bankOf([Qa()]), { license: "CC-BY-4.0" })).body;
    w.advance(DAY);
    await get(w, C, a.item_id as string);
    assert.equal((await publish(w, A, bankOf(served(await get(w, A, a.item_id as string))), { license: "CC-BY-NC-4.0", base: a.publish_id as string }, a.item_id as string)).status, 200);
    w.advance(DAY);
    await get(w, C, a.item_id as string);
    for (const L of ["CC-BY-4.0", "CC-BY-NC-4.0"]) {
      const c = await publish(w, C, bankOf([Qa()]), { license: L });
      assert.equal(c.status, 201, L);
      assert.deepEqual((await get(w, R, c.body.item_id as string)).body.license_check, { status: "ok", basis: "recorded_sources" }, L);
    }
  }
  // The recorded set is stored per question.
  const w = new World();
  const A = w.user(), B = w.user();
  const a = (await publish(w, A, bankOf([Qa()]), { license: "CC-BY-NC-4.0" })).body;
  w.advance(DAY);
  const b = (await publish(w, B, bankOf(withOrigin(await imp(w, B, a.item_id as string))), { license: "CC-BY-NC-4.0" })).body;
  const st = JSON.parse(w.q<{ allowed_json: string }>(`SELECT allowed_json FROM item_questions WHERE publish_id = ?`, b.publish_id)[0]!.allowed_json);
  assert.deepEqual(st, { allowed: ["CC-BY-NC-4.0"], binders: [a.item_id] });
});

test("P6 (s3a): a short statement is short in its KEY too; Matching never; Mc only with stem and choices within 12 words", async () => {
  const m = await import("../../src/pool/publish.ts");
  const isShort = (x: Q) => m.isShort(x as never);
  const long = "Osmosis is the net movement of water molecules across a selectively permeable membrane from a region of lower solute concentration to one of higher solute concentration, driven by the difference in water potential; it requires no cellular energy and continues until equilibrium.";
  assert.equal(isShort(q("Define osmosis in your own words.", { kind: { Short: { sample: long } } })), false, "a long model answer is not short");
  assert.equal(isShort(q("Define osmosis in your own words.", { kind: { Short: { sample: "Water moves across a membrane." } } })), true);
  assert.equal(isShort(tf("The sun is a star.")), true);
  assert.equal(isShort({ ...tf("The sun is a star."), explanation: "Because it fuses hydrogen." }), false, "an explanation is never short");
  assert.equal(isShort(tf("One two three four five six seven eight nine.")), false, "a stem of 9 words");
  assert.equal(isShort(q("Pair them.", { kind: { Matching: { pairs: [{ left: "a", right: "b" }, { left: "c", right: "d" }], distractors: [] } } })), false, "Matching never");
  assert.equal(isShort(mcq("Largest planet?", [["Jupiter", true], ["Mars", false], ["Venus", false]])), true, "Mc: 2 + 3 words");
  assert.equal(isShort(mcq("Which organelle is the main site of aerobic respiration?", [["Mitochondrion", true], ["Ribosome", false], ["Golgi body", false]])), false, "Mc: 9 + 4 words > 12");
  assert.equal(isShort(q("Fill in: the ___ is red.", { kind: { Blank: { accept: [["apple"]] } } }) ), true);
  assert.equal(isShort(q("Fill in: the ___ is red.", { kind: { Blank: { accept: [["a very long accepted answer that goes on and on for many words"]] } } })), false, "a long accepted answer");
  // Through the API: the osmosis question read and relabelled CC0 is refused (it binds).
  const w = new World();
  const A = w.user("teacher", "Ann"), X = w.user("teacher", "Xav");
  const a = (await publish(w, A, bankOf([q("Define osmosis in your own words.", { kind: { Short: { sample: long } } })]), { license: "CC-BY-NC-SA-4.0" })).body;
  w.advance(DAY);
  const x = await publish(w, X, bankOf(served(await get(w, X, a.item_id as string))), { license: "CC0-1.0" });
  assert.equal(out(x), `422${JSON.stringify(["CC-BY-NC-SA-4.0"])}`);
  // ... while a genuinely short statement read is common knowledge (not bound).
  const b = (await publish(w, A, bankOf([tf("The sun is a star.")]), { license: "CC-BY-NC-SA-4.0" })).body;
  w.advance(DAY);
  assert.equal((await publish(w, X, bankOf(served(await get(w, X, b.item_id as string))), { license: "CC0-1.0" })).status, 201);
});

test("P7 (s3c): 'common' needs K independent roots that published BEFORE the source; sock puppets after it never count", async () => {
  for (const n of [2, 3, 5]) {
    const w = new World();
    const A = w.user("teacher", "Ann"), X = w.user("teacher", "Xav");
    const a = (await publish(w, A, bankOf([Qa()]), { license: "CC-BY-NC-SA-4.0" })).body;
    w.advance(DAY);
    const ra = await get(w, X, a.item_id as string);
    for (let k = 0; k < n; k++) assert.equal((await publish(w, w.user("teacher", `Puppet${k}`), bankOf(served(ra)), { license: "CC-BY-NC-SA-4.0" })).status, 201);
    const x = await publish(w, X, bankOf(served(ra)), { license: "CC0-1.0" });
    assert.equal(out(x), `422${JSON.stringify(["CC-BY-NC-SA-4.0"])}`, `${n} puppets after Ann never make Ann's question common`);
  }
  // A legitimately common question: three independent teachers had it BEFORE Ann.
  const w = new World();
  const A = w.user("teacher", "Ann"), X = w.user("teacher", "Xav");
  for (let k = 0; k < 3; k++) {
    assert.equal((await publish(w, w.user("teacher", `Older${k}`), bankOf([Qa()]), { license: "CC0-1.0" })).status, 201);
    w.advance(DAY);
  }
  const a = (await publish(w, A, bankOf([Qa()]), { license: "CC-BY-NC-SA-4.0" })).body;
  w.advance(DAY);
  const x = await publish(w, X, bankOf(served(await get(w, X, a.item_id as string))), { license: "CC0-1.0" });
  assert.equal(x.status, 201, "a read of a question three earlier independent teachers published is common");
  // Two older ones are not enough.
  const w2 = new World();
  const A2 = w2.user("teacher", "Ann"), X2 = w2.user("teacher", "Xav");
  for (let k = 0; k < 2; k++) {
    await publish(w2, w2.user("teacher", `Older${k}`), bankOf([Qa()]), { license: "CC0-1.0" });
    w2.advance(DAY);
  }
  const a2 = (await publish(w2, A2, bankOf([Qa()]), { license: "CC-BY-NC-SA-4.0" })).body;
  w2.advance(DAY);
  await get(w2, X2, a2.item_id as string);
  // X2 read Ann only (not the older ones): Ann binds.
  assert.equal(out(await publish(w2, X2, bankOf([Qa()]), { license: "CC0-1.0" })), `422${JSON.stringify(["CC-BY-NC-SA-4.0"])}`);
});

test("hidden-key source: adding the key AND your own explanation is an adaptation; adding only the key is a copy", async () => {
  const base = mcq("Which organelle is the main site of aerobic respiration in eukaryotic cells?", [["Mitochondrion", true], ["Ribosome", false], ["Golgi body", false]]);
  const w = new World();
  const A = w.user("teacher", "Ann"), B = w.user("teacher", "Bob");
  const a = (await publish(w, A, bankOf([base]), { license: "CC-BY-4.0", hideKey: true })).body;
  w.advance(DAY);
  const sb = served(await imp(w, B, a.item_id as string))[0]!;
  const keyed = { ...sb, keyless: undefined, kind: { Mc: { answers: sb.kind.Mc.answers.map((x: Q, i: number) => ({ ...x, correct: i === 0 })) } } };
  assert.equal(out(await publish(w, B, bankOf([keyed]), { license: "CC-BY-NC-SA-4.0" })), `422${JSON.stringify(["CC-BY-4.0"])}`, "the key alone: a copy keeps CC-BY");
  const adapted = await publish(w, B, bankOf([{ ...keyed, explanation: "Bob's own explanation: the Krebs cycle and the electron transport chain run there." }]), { license: "CC-BY-NC-SA-4.0" });
  assert.equal(adapted.status, 201, "the key and Bob's own explanation: an adaptation, any BY-compatible licence");
});

test("P1: the qid's lineage survives retention: an author's drifting edits stay theirs against a copier in the middle", async () => {
  // Token sets: Q0 = t1..t12; the copier X = Q0 with t12 -> x; V's edits Q1 = Q0 with t11 -> y, Q2 = Q1 with
  // t12 -> x. Each edit continues the previous (Jaccard 11/13); Q2 is near X (11/13) but not Q0 (10/14).
  const t = Array.from({ length: 12 }, (_, i) => `word${String.fromCharCode(97 + i)}${i}`);
  const st = (xs: string[]) => xs.join(" ") + "?";
  const Q0 = st(t), X = st([...t.slice(0, 11), "xenon"]), Q1 = st([...t.slice(0, 10), "yttrium", t[11]!]), Q2 = st([...t.slice(0, 10), "yttrium", "xenon"]);
  const w = new World();
  const V = w.user("teacher", "Original"), C = w.user("teacher", "Copier"), R = w.user();
  const v = (await publish(w, V, bankOf([q(Q0)]), { license: "CC-BY-4.0" })).body;
  w.advance(DAY);
  await get(w, C, v.item_id as string);
  // The copier attests independence (so its chain does not lead back to the original).
  const x = (await publish(w, C, bankOf([q(X)]), { license: "CC-BY-NC-SA-4.0", independent: [v.item_id as string] })).body;
  // Round 16 (B2): the earlier visible original stays credited on it as a possible source.
  assert.deepEqual(await chainOf(w, R, x.item_id), [[v.item_id]]);
  w.advance(DAY);
  await get(w, V, x.item_id as string);
  let cur: any = v;
  for (const s of [Q1, Q2]) {
    const r = await publish(w, V, bankOf([{ ...q(s), published: { item_id: v.item_id, qid: (v.qids as string[])[0], publish_id: cur.publish_id, version: cur.version } }]), { base: cur.publish_id, license: "CC-BY-4.0" }, v.item_id as string);
    assert.equal(r.status, 200, s);
    cur = r.body;
    w.advance(DAY);
  }
  assert.deepEqual(await chainOf(w, R, v.item_id), [[]], "the copier is never credited on the original's lineage");
  // Round 14 (B6) flagged a lineage ending near a served source; round 16 (B2): the copy now credits
  // the original (possible source), so it is the original's own derivative: no suspicion either way.
  // (The B6 walk itself: round14.test.ts.)
  assert.ok(!flagsOf(w, v.item_id).includes("origin_suspect"));
  assert.deepEqual((await get(w, R, v.item_id as string)).body.license_check, { status: "ok", basis: "recorded_sources" });
  assert.ok(w.q<{ ls: number; seq: number }>(`SELECT lineage_seq AS ls, seq FROM item_questions WHERE publish_id = ?`, cur.publish_id)[0]!.ls < w.q<{ s: number }>(`SELECT MIN(seq) AS s FROM item_questions WHERE publish_id = ?`, x.publish_id)[0]!.s);
});
