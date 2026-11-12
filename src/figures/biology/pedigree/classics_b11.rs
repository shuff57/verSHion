//! The six classic families of the pedigree's Classic setups list (batch 11), each a fixed family whose affected and
//! carrier marks follow its inheritance, stored as the `family` param (so a loaded family is changed by hand like any
//! other). In the family text (see `family.rs`) a person is `m`/`f`, then `A` affected or `C` carrier, `d` deceased,
//! and the disease alleles as a digit (1 first allele, 2 second, 3 both; X-linked: a man's one X is the first, a
//! woman's are the one from her mother then the one from her father). Daughters of a carrier who may or may not be
//! carriers themselves are drawn unmarked, as a pedigree draws them.

/// (setup name, inheritance, allele letter, title, carrier style, family).
pub(super) const FAMILIES: [(&str, &str, &str, &str, &str, &str); 6] = [
    // Hemophilia in Queen Victoria's descendants, simplified: four of her nine children and the lines that carried
    // it. Victoria (I-2) was a carrier; her son Leopold had hemophilia, her daughters Alice and Beatrice were
    // carriers. Alice's son Friedrich had it and her daughters Irene (sons Waldemar and Heinrich affected) and Alix
    // (Empress Alexandra; her son Tsarevich Alexei affected) were carriers. Leopold's daughter Alice of Albany was an
    // obligate carrier (her son Rupert affected); Beatrice's daughter Victoria Eugenie (Queen Ena of Spain; sons
    // Alfonso and Gonzalo affected) carried it and her son Leopold had it. Sources: D. M. Potts and W. T. W. Potts,
    // "Queen Victoria's Gene" (1995); R. F. Stevens, "The history of haemophilia in the royal families of Europe",
    // Br J Haematol 105 (1999) 25-32; E. I. Rogaev et al., "Genotype analysis identifies the cause of the 'royal
    // disease'", Science 326 (2009) 817 (hemophilia B, an F9 mutation: X-linked recessive).
    (
        "Hemophilia, royal family",
        "X-linked recessive",
        "H",
        "Hemophilia in Queen Victoria’s family",
        "Half filled",
        "1:m-fC1(m,fC1-m(fC1-m(mA1,m,mA1),mA1,fC1-m(f,f,f,f,mA1)),mA1-f(fC2-m(f,mA1),m),fC1-m(m,fC1-m(mA1,m,mA1),mA1,m))",
    ),
    // Autosomal dominant: every affected child has an affected parent, and an affected parent's children are
    // affected or not; it shows in every generation.
    ("Huntington’s disease", "Autosomal dominant", "H", "Huntington’s disease", "Automatic", "1:mAd1-f(fA1-m(mA1,f,fA1),m,mA1-f(fA1,m),f)"),
    // Autosomal recessive: two carrier parents with an affected daughter; her children with a non-carrier are all
    // carriers; a carrier son and a carrier wife have an affected son.
    ("Cystic fibrosis", "Autosomal recessive", "F", "Cystic fibrosis", "Center dot", "1:mC1-fC2(mC1-fC1(mA3,fC1,m),fA3-m(fC1,mC1),m)"),
    // Autosomal recessive skipping a generation: a carrier grandfather, a carrier daughter who marries a carrier, two
    // affected grandchildren.
    ("Albinism", "Autosomal recessive", "A", "Albinism", "Center dot", "1:mC1-f(fC1-mC1(fA3,m,mC1,fA3),m-f(m,f),f)"),
    // X-linked recessive: an affected grandfather's daughters are carriers; a carrier's sons are affected or not; a
    // carrier and an affected man can have an affected daughter.
    ("Red–green color blindness", "X-linked recessive", "B", "Red–green color blindness", "Center dot", "1:mA1-f(fC2-m(mA1,m,f),m,fC2-mA1(fA3,fC2,mA1,m))"),
    // X-linked dominant (hypophosphatemic rickets): an affected father's daughters are all affected and his sons
    // never; an affected mother's sons and daughters are affected or not.
    ("Vitamin D–resistant rickets", "X-linked dominant", "R", "Vitamin D–resistant rickets", "Automatic", "1:mA1-f(fA2-m(mA1,f,fA1,m),m-f(f,m),fA2-m(f,mA1),m)"),
];

#[cfg(test)]
mod tests {
    use super::super::*;
    use super::FAMILIES;

    fn mode(s: &str) -> Mode {
        match s {
            "Autosomal dominant" => Mode::AD,
            "X-linked recessive" => Mode::XR,
            "X-linked dominant" => Mode::XD,
            "Y-linked" => Mode::Y,
            _ => Mode::AR,
        }
    }

    /// Every way a child of `mom` and `dad` can inherit (the child's alleles as a sorted pair).
    fn possible(mom: [u8; 2], dad: [u8; 2], male: bool, m: Mode) -> Vec<[u8; 2]> {
        let mut v = Vec::new();
        for &x in &mom {
            match m {
                Mode::AD | Mode::AR => for &y in &dad { v.push(if x <= y { [x, y] } else { [y, x] }); },
                Mode::XR | Mode::XD => v.push(if male { [x, 0] } else if x <= dad[0] { [x, dad[0]] } else { [dad[0], x] }),
                Mode::Y => v.push(if male { [0, dad[1]] } else { [0, 0] }),
            }
        }
        v
    }

    fn sorted(a: [u8; 2], male: bool, m: Mode) -> [u8; 2] {
        if male && matches!(m, Mode::XR | Mode::XD) { return a; }
        if a[0] <= a[1] { a } else { [a[1], a[0]] }
    }

    /// Every parent-child trio in each classic family is possible under its inheritance, and every person's affected
    /// and carrier mark is the one their alleles give under it (no carriers under a dominant mode, no carrier men
    /// under an X-linked one); each family has affected and unaffected people.
    #[test]
    fn every_classic_family_follows_its_inheritance() {
        assert_eq!(FAMILIES.len(), 6);
        for (name, inherit, _, _, _, text) in FAMILIES {
            let m = mode(inherit);
            let f = family::Family::parse(text).unwrap_or_else(|| panic!("{name}: the family reads"));
            assert_eq!(f.to_text(), text, "{name}: written back as typed");
            let mut trios = 0;
            let mut affected_gens = std::collections::BTreeSet::new();
            let mut unaffected = 0;
            fn walk(n: &family::Node, g_n: usize, m: Mode, name: &str, trios: &mut usize, ag: &mut std::collections::BTreeSet<usize>, un: &mut usize) {
                let mut people = vec![&n.me];
                if let Some(q) = &n.mate { people.push(q); }
                for q in people {
                    let male = q.sex == 0;
                    assert!(q.sex < 2, "{name}: everyone is a man or a woman");
                    let want = st_u8(status_from(q.a, male, m));
                    assert_eq!(q.status, want, "{name}: generation {} person {q:?}: marked {} but the alleles give {want}", g_n + 1, q.status);
                    if matches!(m, Mode::AD | Mode::XD) { assert_ne!(q.status, 1, "{name}: no carriers under a dominant mode"); }
                    if male && matches!(m, Mode::XR | Mode::XD) { assert_eq!(q.a[1], 0, "{name}: a man has one X"); assert_ne!(q.status, 1, "{name}: no carrier men"); }
                    if q.status == 2 { ag.insert(g_n); } else { *un += 1; }
                }
                if !n.kids.is_empty() {
                    let mate = n.mate.as_ref().unwrap_or_else(|| panic!("{name}: children have two parents"));
                    let (mom, dad) = if n.me.sex == 1 { (&n.me, mate) } else { (mate, &n.me) };
                    assert!(mom.sex == 1 && dad.sex == 0, "{name}: a mother and a father");
                    for k in &n.kids {
                        let male = k.me.sex == 0;
                        let ways = possible(mom.a, dad.a, male, m);
                        assert!(ways.contains(&sorted(k.me.a, male, m)), "{name}: generation {} child {:?} of mother {:?} and father {:?} cannot inherit that", g_n + 2, k.me, mom, dad);
                        *trios += 1;
                        walk(k, g_n + 1, m, name, trios, ag, un);
                    }
                }
            }
            walk(&f.root, 0, m, name, &mut trios, &mut affected_gens, &mut unaffected);
            assert!(trios >= 8, "{name}: {trios} parent-child trios");
            assert!(!affected_gens.is_empty() && unaffected > 0, "{name}: affected in generations {affected_gens:?}");
        }
    }

    /// The trio check itself catches what each mode forbids.
    #[test]
    fn the_trio_check_refuses_impossible_children() {
        // autosomal recessive: two clear parents cannot have an affected child
        assert!(!possible([0, 0], [0, 0], true, Mode::AR).contains(&[1, 1]));
        // autosomal dominant: an affected child needs an affected parent
        assert!(!possible([0, 0], [0, 0], false, Mode::AD).iter().any(|a| a[0] + a[1] > 0));
        // X-linked recessive: an affected son needs a carrier or affected mother, whatever the father
        assert!(!possible([0, 0], [1, 0], true, Mode::XR).contains(&[1, 0]));
        // X-linked dominant: an affected father's daughters all inherit it
        assert!(possible([0, 0], [1, 0], false, Mode::XD).iter().all(|a| a[0] + a[1] == 1));
    }

    /// The royal family: the people the history names are where they should be (Victoria a carrier, Leopold
    /// affected, Alexei affected, Alice of Albany carrying her father's X).
    #[test]
    fn the_royal_family_follows_queen_victorias_descent() {
        let f = family::Family::parse(FAMILIES[0].5).unwrap();
        assert_eq!(f.depth(), 4, "four generations, Victoria to Alexei");
        let victoria = f.root.mate.as_ref().unwrap();
        assert_eq!((victoria.sex, victoria.status, victoria.a), (1, 1, [1, 0]), "Victoria, a carrier");
        let kids = &f.root.kids;
        assert_eq!(kids.len(), 4, "Edward, Alice, Leopold, Beatrice");
        assert_eq!((kids[0].me.sex, kids[0].me.status), (0, 0), "Edward VII, unaffected");
        assert_eq!((kids[1].me.sex, kids[1].me.status), (1, 1), "Alice, a carrier");
        assert_eq!((kids[2].me.sex, kids[2].me.status), (0, 2), "Leopold, hemophilia");
        assert_eq!((kids[3].me.sex, kids[3].me.status), (1, 1), "Beatrice, a carrier");
        let alix = &kids[1].kids[2];
        assert_eq!(alix.kids.len(), 5, "Olga, Tatiana, Maria, Anastasia, Alexei");
        assert_eq!((alix.kids[4].me.sex, alix.kids[4].me.status), (0, 2), "Alexei, hemophilia");
        let alice_albany = &kids[2].kids[0];
        assert_eq!((alice_albany.me.status, alice_albany.me.a), (1, [0, 1]), "Alice of Albany carries her father's X");
    }
}
