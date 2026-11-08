use super::*;

fn defaults() -> Params { Params::defaults(&params()) }

fn with(over: &[(&str, Value)]) -> Params {
    let mut p = defaults();
    for (k, v) in over {
        assert!(p.0.contains_key(*k), "unknown param {k}");
        p.0.insert((*k).to_string(), v.clone());
    }
    p
}

fn s(v: &str) -> Value { Value::S(v.into()) }
fn b(v: bool) -> Value { Value::B(v) }

/// Every `<text>` element's content, in order.
fn texts(svg: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = svg;
    while let Some(i) = rest.find("<text") {
        rest = &rest[i..];
        let Some(gt) = rest.find('>') else { break };
        let Some(end) = rest.find("</text>") else { break };
        out.push(rest[gt + 1..end].replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">"));
        rest = &rest[end..];
    }
    out
}

/// The standard genetic code written out by amino acid (the degeneracy table), independent of `CODE`.
const BY_AMINO: [(&str, &[&str]); 21] = [
    ("Ala", &["GCU", "GCC", "GCA", "GCG"]),
    ("Arg", &["CGU", "CGC", "CGA", "CGG", "AGA", "AGG"]),
    ("Asn", &["AAU", "AAC"]),
    ("Asp", &["GAU", "GAC"]),
    ("Cys", &["UGU", "UGC"]),
    ("Gln", &["CAA", "CAG"]),
    ("Glu", &["GAA", "GAG"]),
    ("Gly", &["GGU", "GGC", "GGA", "GGG"]),
    ("His", &["CAU", "CAC"]),
    ("Ile", &["AUU", "AUC", "AUA"]),
    ("Leu", &["UUA", "UUG", "CUU", "CUC", "CUA", "CUG"]),
    ("Lys", &["AAA", "AAG"]),
    ("Met", &["AUG"]),
    ("Phe", &["UUU", "UUC"]),
    ("Pro", &["CCU", "CCC", "CCA", "CCG"]),
    ("Ser", &["UCU", "UCC", "UCA", "UCG", "AGU", "AGC"]),
    ("Thr", &["ACU", "ACC", "ACA", "ACG"]),
    ("Trp", &["UGG"]),
    ("Tyr", &["UAU", "UAC"]),
    ("Val", &["GUU", "GUC", "GUA", "GUG"]),
    ("Stop", &["UAA", "UAG", "UGA"]),
];

#[test]
fn all_64_codons_follow_the_standard_code() {
    let mut seen = std::collections::BTreeSet::new();
    for (aa, codons) in BY_AMINO {
        for c in codons {
            assert!(seen.insert(*c), "{c} listed twice");
            let one = translate(c.as_bytes());
            assert_eq!(aa_name(one, Naming::Three), aa, "{c}");
        }
    }
    assert_eq!(seen.len(), 64, "every codon exactly once");
    // and every one of the 64 is covered by enumerating the bases
    let mut n = 0;
    for &a in b"UCAG" {
        for &b2 in b"UCAG" {
            for &c in b"UCAG" {
                let codon = [a, b2, c];
                assert!(seen.contains(std::str::from_utf8(&codon).unwrap()));
                n += 1;
            }
        }
    }
    assert_eq!(n, 64);
    // start and the three stops
    assert_eq!(translate(b"AUG"), b'M');
    let stops: Vec<String> = BY_AMINO.iter().flat_map(|(_, c)| c.iter()).filter(|c| translate(c.as_bytes()) == b'*').map(|c| c.to_string()).collect();
    assert_eq!(stops, vec!["UAA", "UAG", "UGA"]);
    // the one-letter codes and the full names agree with the three-letter ones
    for (one, three, full) in [(b'W', "Trp", "Tryptophan"), (b'K', "Lys", "Lysine"), (b'D', "Asp", "Aspartic acid"), (b'Q', "Gln", "Glutamine"), (b'N', "Asn", "Asparagine")] {
        assert_eq!(aa_name(one, Naming::Three), three);
        assert_eq!(aa_name(one, Naming::Full), full);
        assert_eq!(aa_name(one, Naming::One), (one as char).to_string());
    }
    assert_eq!(aa_name(b'*', Naming::One), "Stop");
}

#[test]
fn the_codon_chart_lists_all_64_with_start_and_stops_in_bold() {
    let svg = render(&with(&[("chart", s("Codon chart only"))]));
    let t = texts(&svg);
    for (aa, codons) in BY_AMINO {
        for c in codons {
            let want = if *c == "AUG" { format!("{c} {aa} (Start)") } else { format!("{c} {aa}") };
            assert!(t.contains(&want), "chart is missing {want}");
        }
    }
    for c in ["AUG Met (Start)", "UAA Stop", "UAG Stop", "UGA Stop"] {
        assert!(svg.contains(&format!(r#"font-weight="bold">{c}</text>"#)), "{c} not bold");
    }
    assert!(!svg.contains(r#"font-weight="bold">UUU Phe<"#));
    assert!(t.iter().any(|x| x == "Second base") && t.iter().any(|x| x == "First base") && t.iter().any(|x| x == "Third base"));
    // the strands are not drawn with the chart alone
    assert!(!t.iter().any(|x| x == "DNA coding strand"));
    // one-letter names in the chart
    let one = texts(&render(&with(&[("chart", s("Codon chart only")), ("aa_names", s("M"))])));
    assert!(one.contains(&"UGG W".to_string()) && one.contains(&"AUG M (Start)".to_string()));
}

#[test]
fn transcription_is_template_complementary_and_mrna_is_coding_with_u() {
    // every base, and a sequence typed each of the three ways
    let mut rng = Rng::new(42);
    for _ in 0..200 {
        let n = 1 + rng.below(36);
        let seq: String = (0..n).map(|_| ['A', 'C', 'G', 'T'][rng.below(4)]).collect();
        for typed in ["Template", "Coding", "mRNA"] {
            let text = if typed == "mRNA" { seq.replace('T', "U") } else { seq.clone() };
            let m = Model::new(&text, typed, true);
            assert_eq!(m.len(), n);
            for i in 0..n {
                // A–T and G–C between the DNA strands
                assert_eq!(dna_pair(m.coding[i]), m.template[i]);
                assert_eq!(dna_pair(m.template[i]), m.coding[i]);
                // the mRNA pairs with the template (A–U, G–C) ...
                assert_eq!(rna_pair(m.template[i]), m.mrna[i]);
                // ... so it is the coding strand with U for T
                assert_eq!(m.mrna[i], if m.coding[i] == b'T' { b'U' } else { m.coding[i] });
                assert!(!m.mrna.contains(&b'T') && !m.coding.contains(&b'U') && !m.template.contains(&b'U'));
            }
            // the strand typed is drawn exactly as typed
            let typed_row = match typed { "Template" => &m.template, "Coding" => &m.coding, _ => &m.mrna };
            assert_eq!(String::from_utf8_lossy(typed_row), text);
        }
    }
}

#[test]
fn the_opening_sequence_reads_met_glu_pro_ala_phe_stop() {
    let m = Model::from_params(&defaults());
    assert_eq!(String::from_utf8_lossy(&m.template), "TACCTTGGACGTAAAATT");
    assert_eq!(String::from_utf8_lossy(&m.coding), "ATGGAACCTGCATTTTAA");
    assert_eq!(String::from_utf8_lossy(&m.mrna), "AUGGAACCUGCAUUUUAA");
    assert_eq!((m.start, m.codons, m.read, m.stopped, m.trnas()), (Some(0), 6, 6, true, 5));
    assert_eq!(m.chain(Naming::Three), "Met\u{2013}Glu\u{2013}Pro\u{2013}Ala\u{2013}Phe, then Stop");
    let anti: Vec<String> = (0..m.trnas()).map(|k| String::from_utf8_lossy(&m.anticodon(k)).into_owned()).collect();
    assert_eq!(anti, ["UAC", "CUU", "GGA", "CGU", "AAA"]);
    let t = texts(&render(&defaults()));
    for name in ["DNA coding", "strand", "mRNA", "tRNA anticodons", "Amino acids", "Met", "Glu", "Pro", "Ala", "Phe", "Stop"] {
        assert!(t.iter().any(|x| x == name), "missing {name}");
    }
}

#[test]
fn anticodons_are_complementary_and_antiparallel() {
    for seq in ["AUGUUUGGCACCUAG", "AUGGCAUUCCGAUAA", "AUGUGGAAACCCGGGUAA"] {
        let m = Model::new(seq, "mRNA", true);
        for k in 0..m.trnas() {
            let c = m.codon(k).to_vec();
            let a = m.anticodon(k);
            for j in 0..3 {
                assert_eq!(rna_pair(c[j]), a[j], "{seq} codon {k} base {j}");
            }
            // read 5′→3′ (right to left as drawn) the anticodon is the reverse complement of the codon
            let five_to_three: Vec<u8> = a.iter().rev().copied().collect();
            let revcomp: Vec<u8> = c.iter().rev().map(|&x| rna_pair(x)).collect();
            assert_eq!(five_to_three, revcomp);
        }
    }
    // the tRNA row is written 3′ on the left and 5′ on the right, the mRNA 5′ to 3′, the template 3′ to 5′
    assert_eq!(Row::Trna.ends(), ("3\u{2032}", "5\u{2032}"));
    assert_eq!(Row::Mrna.ends(), ("5\u{2032}", "3\u{2032}"));
    assert_eq!(Row::Template.ends(), ("3\u{2032}", "5\u{2032}"));
    assert_eq!(Row::Coding.ends(), ("5\u{2032}", "3\u{2032}"));
    // a stop codon has no tRNA
    let m = Model::new("AUGUAA", "mRNA", true);
    assert_eq!((m.read, m.trnas()), (2, 1));
}

#[test]
fn end_to_end_sequences() {
    // (typed, sequence, from AUG, expected protein, untranslated leader)
    let cases: [(&str, &str, bool, &str, usize); 9] = [
        ("Template", "TACGGTCTTAAACCG", true, "Met\u{2013}Pro\u{2013}Glu\u{2013}Phe\u{2013}Gly", 0),
        ("Coding", "GCTAATGTCCAAGGGCTAGTA", true, "Met\u{2013}Ser\u{2013}Lys\u{2013}Gly, then Stop", 4),
        ("mRNA", "AUGGCAUUCCGAUAA", true, "Met\u{2013}Ala\u{2013}Phe\u{2013}Arg, then Stop", 0),
        ("Template", "TACAAACGGTTGACT", true, "Met\u{2013}Phe\u{2013}Ala\u{2013}Asn, then Stop", 0),
        ("Coding", BETA_NORMAL, true, "Met\u{2013}Val\u{2013}His\u{2013}Leu\u{2013}Thr\u{2013}Pro\u{2013}Glu\u{2013}Glu\u{2013}Lys", 0),
        // a frame read from the first base, not from an AUG
        ("mRNA", "GGCUUUAAA", false, "Gly\u{2013}Phe\u{2013}Lys", 0),
        // translation stops at the first stop; the codons after it are not read
        ("mRNA", "AUGUGACCCGGG", true, "Met, then Stop", 0),
        // the first AUG wins, even out of frame with an earlier base
        ("mRNA", "CAUGCAUUGA", true, "Met\u{2013}His, then Stop", 1),
        ("Template", "tac-ccg  att", true, "Met\u{2013}Gly, then Stop", 0),
    ];
    for (typed, seq, aug, want, lead) in cases {
        let m = Model::new(seq, typed, aug);
        assert_eq!(m.chain(Naming::Three), want, "{typed} {seq}");
        assert_eq!(m.start, Some(lead), "{seq}");
    }
    // codon groups: leader, codons, leftover
    let m = Model::new("GCTAATGTCCAAGGGCTAGTA", "Coding", true);
    assert_eq!(m.groups(), vec![(0, 4), (4, 3), (7, 3), (10, 3), (13, 3), (16, 3), (19, 2)]);
    let key = key_lines(&with(&[("typed", s("Coding")), ("seq", s("GCTAATGTCCAAGGGCTAGTA"))]), &m);
    assert_eq!(key[0], "DNA coding strand: 5\u{2032}-GCTA ATG TCC AAG GGC TAG TA-3\u{2032}");
    assert_eq!(key[1], "DNA template strand: 3\u{2032}-CGAT TAC AGG TTC CCG ATC AT-5\u{2032}");
    assert_eq!(key[2], "mRNA: 5\u{2032}-GCUA AUG UCC AAG GGC UAG UA-3\u{2032}");
    assert_eq!(key[3], "tRNA anticodons (each 3\u{2032} to 5\u{2032}): UAC AGG UUC CCG");
    assert_eq!(key[4], "Amino acids: Met\u{2013}Ser\u{2013}Lys\u{2013}Gly, then Stop");
}

#[test]
fn point_mutations_change_the_protein_as_expected() {
    let prot = |seq: &str| Model::new(seq, "Coding", true).protein();
    let normal = prot(BETA_NORMAL);
    // sickle cell: GAG to GTG in codon 7, Glu to Val, nothing else changes
    let sickle = prot(BETA_SICKLE);
    let diff: Vec<usize> = (0..normal.len()).filter(|&k| normal[k] != sickle[k]).collect();
    assert_eq!(diff, vec![6]);
    assert_eq!((normal[6], sickle[6]), (b'E', b'V'));
    // silent: CTG to CTC is still Leu
    assert_eq!(prot(BETA_SILENT), normal);
    // nonsense: GAG to TAG stops after seven amino acids
    let non = prot(BETA_NONSENSE);
    assert_eq!(non.len(), 8);
    assert_eq!(*non.last().unwrap(), b'*');
    assert_eq!(&non[..7], &normal[..7]);
    // frameshift: every codon after the deletion is read differently, here to an early UGA
    let fs = Model::new(BETA_FRAMESHIFT, "Coding", true);
    assert_eq!(fs.chain(Naming::Three), "Met\u{2013}Val\u{2013}Ile, then Stop");
    // the examples carry these sequences and render
    for e in examples(ID).unwrap() {
        let mut p = defaults();
        for (k, v) in &e.params {
            p.0.insert(k.to_string(), v.clone());
        }
        assert!(texts(&render(&p)).len() > 10, "{}", e.name);
    }
}

#[test]
fn typing_is_forgiving() {
    let m = Model::new("5′-atg ccT-aag-3′", "Coding", true);
    assert_eq!(String::from_utf8_lossy(&m.coding), "ATGCCTAAG");
    assert!(m.ignored.is_empty() && m.cut == 0 && !m.swapped);
    let m = Model::new("5'-AUG-3'", "Coding", true);
    assert!(m.swapped);
    assert_eq!(String::from_utf8_lossy(&m.coding), "ATG");
    let m = Model::new("ATGXN", "Coding", true);
    assert_eq!(m.ignored, vec!['X', 'N']);
    let long: String = "ATG".repeat(14);
    let m = Model::new(&long, "Coding", true);
    assert_eq!((m.len(), m.cut), (36, 6));
    let p = with(&[("seq", s(&long)), ("typed", s("Coding"))]);
    assert!(seq_note(&p).unwrap().contains("first 36 bases"));
    // no AUG: nothing is read, and the readout says why
    let p = with(&[("seq", s("GGGCCCTTT")), ("typed", s("Coding"))]);
    assert_eq!(Model::from_params(&p).read, 0);
    assert!(readout(&p).starts_with("No AUG"));
    let p2 = with(&[("seq", s("GGGCCCTTT")), ("typed", s("Coding")), ("frame", s("From the first base"))]);
    assert_eq!(readout(&p2), "Gly\u{2013}Pro\u{2013}Phe (no stop codon yet)");
    // empty and odd input still render
    for t in ["", "   ", "xyz", "A", "AU", "AUG"] {
        let svg = render(&with(&[("seq", s(t)), ("typed", s("mRNA"))]));
        assert!(svg.starts_with("<svg") && svg.ends_with("</svg>"), "{t:?}");
    }
}

#[test]
fn blank_and_some_rows_hide_bases_and_the_key_fills_them() {
    let m = Model::from_params(&defaults());
    let letters = |svg: &str| texts(svg).into_iter().filter(|t| t.len() == 1 && "ACGTU".contains(t.as_str())).count();
    let full = letters(&render(&defaults()));
    // 18 coding + 18 template + 18 mRNA + 15 anticodon letters
    assert_eq!(full, 18 * 3 + 15);
    let p = with(&[("row_mrna", s("Blank"))]);
    assert_eq!(letters(&render(&p)), full - 18);
    // the key writes them back in, in the answer style
    let mut k = p.clone();
    k.set_b(SHOW_ANSWERS, true);
    let ks = render(&k);
    assert_eq!(letters(&ks), full);
    assert!(ks.contains(r##"fill="#1d5fb8" font-style="italic">U</text>"##));
    // Some leaves the share blank, picked by the pattern, and Shuffle picks others
    let p = with(&[("row_mrna", s("Some")), ("some_share", Value::I(50))]);
    let bl = blanks(&p, &m, Row::Mrna);
    assert_eq!(bl.len(), 9);
    assert_eq!(letters(&render(&p)), full - 9);
    let mut q = p.clone();
    q.set_i("blank_seed", 77);
    assert_ne!(blanks(&q, &m, Row::Mrna), bl);
    assert_ne!(render(&q), render(&p));
    // amino acids blank: the beads stay, the names go
    let p = with(&[("row_amino", s("Blank"))]);
    let t = texts(&render(&p));
    assert!(!t.iter().any(|x| x == "Met") && !t.iter().any(|x| x == "Stop"));
    assert_eq!(render(&p).matches(r#"rx="#).count(), 6, "five beads and a detached one for the stop");
    // row names blank: lines to write on, no names
    let t = texts(&render(&with(&[("row_names", s("Blank"))])));
    assert!(!t.iter().any(|x| x.contains("strand")));
    // ends blank and off
    let t = texts(&render(&with(&[("ends", s("Blank"))])));
    assert!(!t.iter().any(|x| x == "5\u{2032}"));
    let t = texts(&render(&defaults()));
    assert_eq!(t.iter().filter(|x| *x == "5\u{2032}").count(), 4);
}

#[test]
fn mistakes_change_only_the_chosen_row_and_are_listed_in_the_key() {
    for n in 1..=4 {
        let p = with(&[("mistakes", s(&n.to_string())), ("mistake_row", s("mRNA"))]);
        let m = Model::from_params(&p);
        let xs = mistakes(&p, &m);
        assert_eq!(xs.len(), n);
        for x in &xs {
            assert_eq!(x.row, Row::Mrna);
            assert_eq!(x.right, m.mrna[x.at]);
            assert_ne!(x.drawn, x.right);
            assert!(b"ACGU".contains(&x.drawn), "an RNA base");
        }
        let mut ats: Vec<usize> = xs.iter().map(|x| x.at).collect();
        ats.dedup();
        assert_eq!(ats.len(), n, "different bases");
        // the drawn mRNA differs in exactly n places; the other rows are as worked out
        let mut key = p.clone();
        key.set_b(SHOW_ANSWERS, true);
        let lines = key_lines(&key, &m);
        assert!(lines.last().unwrap().starts_with(if n == 1 { "Mistake: mRNA base" } else { "Mistakes: mRNA base" }));
        assert!(render(&key).contains(r##"stroke="#c62828""##));
        assert!(!render(&p).contains(r##"stroke="#c62828""##), "the student copy does not ring the mistakes");
    }
    // DNA rows get DNA bases, the tRNA row RNA bases, numbered by anticodon
    let p = with(&[("mistakes", s("2")), ("mistake_row", s("DNA template strand"))]);
    let m = Model::from_params(&p);
    assert!(mistakes(&p, &m).iter().all(|x| b"ACGT".contains(&x.drawn)));
    let p = with(&[("mistakes", s("1")), ("mistake_row", s("tRNA anticodons"))]);
    let m = Model::from_params(&p);
    let x = &mistakes(&p, &m)[0];
    assert!(x.at < 15);
    assert!(mistake_text(&p, &m, x).starts_with("tRNA anticodon "));
    // a blank row has no base to change; the shuffle moves them
    let p = with(&[("mistakes", s("1")), ("row_mrna", s("Blank"))]);
    assert!(mistakes(&p, &Model::from_params(&p)).is_empty());
    let a = with(&[("mistakes", s("3"))]);
    let mut b2 = a.clone();
    b2.set_i("mistake_seed", 9);
    assert_ne!(mistakes(&a, &Model::from_params(&a)), mistakes(&b2, &Model::from_params(&b2)));
    // the multiple-choice classics: A right, B to D with 1 to 3 wrong mRNA bases
    let cl = classics();
    for (i, want) in [(5, 0), (6, 1), (7, 2), (8, 3)] {
        let mut p = defaults();
        for (k, v) in &cl[i].params {
            p.0.insert(k.to_string(), v.clone());
        }
        assert_eq!(mistakes(&p, &Model::from_params(&p)).len(), want, "{}", cl[i].name);
    }
}

#[test]
fn options_change_the_drawing() {
    let base = render(&defaults());
    let cases: Vec<(&str, Value)> = vec![
        ("typed", s("Coding")),
        ("row_coding", s("Off")),
        ("row_template", s("Blank")),
        ("row_trna", s("Some")),
        ("row_amino", s("Off")),
        ("row_names", s("Off")),
        ("name_mrna", s("messenger RNA")),
        ("aa_names", s("Methionine")),
        ("aa_names", s("M")),
        ("aa_style", s("Words")),
        ("ends", s("Off")),
        ("gaps", b(false)),
        ("brackets", b(true)),
        ("hbonds", b(true)),
        ("color_bases", b(true)),
        ("mistakes", s("2")),
        ("chart", s("Strands and codon chart")),
        ("chart", s("Codon chart only")),
        ("title", s("Protein synthesis")),
        (SHOW_ANSWERS, b(true)),
        ("label_size", s("Large")),
        ("label_size", s("Small")),
    ];
    for (k, v) in cases {
        assert_ne!(render(&with(&[(k, v.clone())])), base, "{k} = {v:?} changed nothing");
    }
    // the reading frame matters once there are bases before the first AUG
    let lead = [("typed", s("Coding")), ("seq", s("GCTAATGTCCAAGGGCTAGTA"))];
    let mut first = lead.to_vec();
    first.push(("frame", s("From the first base")));
    assert_ne!(render(&with(&first)), render(&with(&lead)));
    assert!(texts(&render(&with(&first))).iter().any(|t| t == "Ala"), "GCT read as Ala from the first base");
    // chart below vs beside
    let beside = render(&with(&[("chart", s("Strands and codon chart"))]));
    let below = render(&with(&[("chart", s("Strands and codon chart")), ("chart_pos", s("Below"))]));
    assert!(svg_size(&below).1 > svg_size(&beside).1 && svg_size(&below).0 < svg_size(&beside).0);
    // hydrogen bonds: 2 for each A–T pair, 3 for each G–C
    let m = Model::from_params(&defaults());
    let want: usize = m.coding.iter().map(|&x| if matches!(x, b'G' | b'C') { 3 } else { 2 }).sum();
    assert_eq!(render(&with(&[("hbonds", b(true))])).matches(r#"stroke-dasharray="2.2 1.8""#).count(), want);
    // a bracket under each codon read
    assert_eq!(render(&with(&[("brackets", b(true))])).matches("<path").count(), 6);
    // the answer key prints only with Show answers on
    let key = texts(&render(&with(&[(SHOW_ANSWERS, b(true))])));
    assert!(key.iter().any(|t| t == "mRNA: 5\u{2032}-AUG GAA CCU GCA UUU UAA-3\u{2032}"));
    assert!(!texts(&base).iter().any(|t| t.starts_with("mRNA: ")));
}

#[test]
fn print_style_keeps_coloured_bases_grey_and_distinct() {
    let p = with(&[("style", s("Print")), ("color_bases", b(true)), (SHOW_ANSWERS, b(true)), ("row_mrna", s("Blank")), ("mistakes", s("1")), ("mistake_row", s("DNA coding strand"))]);
    let svg = render(&p);
    for fill in ["#b4b4b4", "#8a8a8a", "#dcdcdc"] {
        assert!(svg.contains(fill));
    }
    for hue in ["#f4a3a3", "#f6dc78", "#96b6ee", "#a3dcae", "#1d5fb8", "#c62828"] {
        assert!(!svg.contains(hue), "Print has {hue}");
    }
    let lum: Vec<u32> = [b'A', b'T', b'G', b'C'].iter().map(|&x| u32::from_str_radix(&Look { print: true, key: false, colour: true, lab: 1.0 }.base_fill(x)[1..3], 16).unwrap()).collect();
    let mut sorted = lum.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.len(), 4, "four different greys");
    assert!(sorted.windows(2).all(|w| w[1] - w[0] >= 30), "greys far enough apart for a copier: {sorted:?}");
}

#[test]
fn panel_relevance_and_notes() {
    let p = defaults();
    assert!(!relevant("some_share", &p));
    assert!(relevant("some_share", &with(&[("row_trna", s("Some"))])));
    assert!(!relevant("mistake_row", &p));
    assert!(relevant("mistake_row", &with(&[("mistakes", s("1"))])));
    assert!(!relevant("chart_pos", &p));
    assert!(relevant("chart_pos", &with(&[("chart", s("Strands and codon chart"))])));
    assert!(!relevant("hbonds", &with(&[("row_template", s("Off"))])));
    assert!(!relevant("blank_seed", &p) && !relevant("mistake_seed", &p));
    assert_eq!(readout(&p), "Met\u{2013}Glu\u{2013}Pro\u{2013}Ala\u{2013}Phe, then Stop");
    assert!(mistakes_note(&p).is_none());
    assert!(mistakes_note(&with(&[("mistakes", s("1"))])).unwrap().starts_with("mRNA base "));
    assert!(seq_note(&p).is_none());
}

#[test]
fn renders_png_for_review() {
    for (name, over) in [
        ("default", vec![]),
        ("key", vec![(SHOW_ANSWERS, b(true)), ("row_mrna", s("Some")), ("row_amino", s("Blank")), ("mistakes", s("2"))]),
        ("colour_hbonds", vec![("color_bases", b(true)), ("hbonds", b(true)), ("brackets", b(true)), ("aa_names", s("Methionine")), ("aa_style", s("Words"))]),
        ("chart_beside", vec![("chart", s("Strands and codon chart")), (SHOW_ANSWERS, b(true))]),
        ("print", vec![("style", s("Print")), ("color_bases", b(true)), ("hbonds", b(true)), ("row_names", s("Blank")), ("ends", s("Blank"))]),
    ] {
        let svg = render(&with(&over));
        assert!(to_png(&svg, 1.0).is_ok(), "{name}");
    }
}
