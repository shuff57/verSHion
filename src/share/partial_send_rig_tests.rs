//! The plan's one unproven D3 check (docs/circles-phaseD-plan.md section 10, phase D3): against the
//! LOCAL rig only, an all-but-one-unticked send reports `changed == 1`, because the raw base JSON
//! round-trips as unchanged.
//!
//! Two `#[ignore]`d tests: [`partial_send_rig_round_trip`] needs the local UI rig answering on
//! <http://localhost:18791> (see `server/worker/scripts/local-ui-rig.sh start`), and
//! [`partial_send_staging_round_trip`] needs a local `wrangler dev --remote` session proxying the
//! staging bindings on <http://localhost:18793>. Each refuses to run unless its base URL is exactly
//! its own address; neither talks to production. The throwaway account's password is made from
//! random bytes in memory and never printed, written to a file or put in a command; the only thing
//! handed to the rig script or the staging D1 command is the throwaway address.

use std::collections::{BTreeSet, HashMap};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

use super::api::Profile;
use super::circle_links::CircleLink;
use super::circles_api::{CircleSend, MESSAGE_MAX};
use super::client::Client;
use super::merge::{BaseQuestion, base_of};
use super::partial_send::{changes_against_body, compose_partial};
use super::pool::{ItemMeta, License, Pool, RetryPolicy};
use super::secret::Secret;
use super::transport::{block_on, Mode, NativeTransport};
use super::vault::MemoryVault;
use crate::model::{Answer, Kind, Question, QuestionBank};

/// The only server the local test may talk to.
const RIG_BASE: &str = "http://localhost:18791";
/// A local `wrangler dev --remote` proxying STAGING bindings: the staging test's only server.
const STAGING_BASE: &str = "http://localhost:18793";
/// The rig's mail log, unless `VERSHION_RIG_MAIL_LOG` names another (the repo's path by default).
const DEFAULT_MAIL_LOG: &str = "server/worker/.wrangler-ui/dev.log";
/// The staging remote dev session's log, unless `VERSHION_STAGING_MAIL_LOG` names another.
const DEFAULT_STAGING_MAIL_LOG: &str = "/tmp/staging-dev.log";

fn random_bytes(n: usize) -> Vec<u8> {
    let mut b = vec![0u8; n];
    std::fs::File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut b))
        .expect("this test needs /dev/urandom for a throwaway password and address");
    b
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn mail_log() -> PathBuf {
    std::env::var_os("VERSHION_RIG_MAIL_LOG").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(DEFAULT_MAIL_LOG))
}

fn staging_mail_log() -> PathBuf {
    std::env::var_os("VERSHION_STAGING_MAIL_LOG").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(DEFAULT_STAGING_MAIL_LOG))
}

/// The newest confirm link's token for `email` in `log`, kept in memory only.
fn confirm_token(email: &str, log: &Path) -> Option<Secret> {
    let log = std::fs::read_to_string(log).ok()?;
    let mut to = String::new();
    let mut found = None;
    for line in log.lines() {
        if let Some(rest) = line.split_once("[dev mail] to=").map(|(_, r)| r) {
            to = rest.split(' ').next().unwrap_or_default().to_ascii_lowercase();
            continue;
        }
        if to != email.to_ascii_lowercase() {
            continue;
        }
        let Some((_, rest)) = line.split_once("#token=") else { continue };
        if !line.contains("/account/verify") {
            continue;
        }
        let token: String = rest.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-' || *c == '.').collect();
        if !token.is_empty() {
            found = Some(Secret::new(token));
        }
    }
    found
}

fn question(id: &str, stem: &str, right: &str, wrong: &str, at: i64) -> Question {
    Question {
        id: id.to_string(),
        stem: stem.to_string(),
        kind: Kind::Mc { answers: vec![Answer { text: right.to_string(), correct: true, locked: false }, Answer { text: wrong.to_string(), correct: false, locked: false }] },
        points: Some(1.0),
        difficulty: None,
        topics: Vec::new(),
        image: None,
        explanation: None,
        figure: None,
        created: at,
        modified: at,
    }
}

fn bank() -> QuestionBank {
    let t = super::browse::now_ms();
    let id = crate::model::new_id();
    QuestionBank {
        id: id.clone(),
        name: "Round-trip partial send".to_string(),
        questions: vec![
            question(&format!("{id}a"), "What is 1 + 1?", "2", "3", t),
            question(&format!("{id}b"), "What is 2 + 2?", "4", "5", t),
            question(&format!("{id}c"), "What is 3 + 3?", "6", "7", t),
        ],
        created: t,
        modified: t,
        details: Default::default(),
    }
}

fn bank_body(bank: &QuestionBank) -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({ "schema_version": 2, "bank": bank })).expect("the bank serialises")
}

fn meta() -> ItemMeta {
    ItemMeta { title: "Round-trip partial send".to_string(), subject: "Mathematics".to_string(), course: String::new(), grade: "8".to_string(), tags: vec!["roundtrip".to_string()], standards: Vec::new() }
}

/// The questions of a version body by their circle id, raw.
fn questions_by_id(payload: &[u8]) -> std::collections::BTreeMap<String, Value> {
    let v: Value = serde_json::from_slice(payload).expect("a version body is JSON");
    v.pointer("/bank/questions")
        .and_then(Value::as_array)
        .expect("a version body has a bank's questions")
        .iter()
        .filter_map(|q| q.get("id").and_then(Value::as_str).map(|id| (id.to_string(), q.clone())))
        .collect()
}

#[test]
#[ignore = "needs the local rig on 18791"]
fn partial_send_rig_round_trip() {
    assert_eq!(RIG_BASE, "http://localhost:18791", "this test may only talk to the local rig");
    if std::env::var_os("VERSHION_POOL_URL").is_some_and(|v| v != RIG_BASE) {
        eprintln!("partial_send_rig_round_trip: VERSHION_POOL_URL is not the local rig; refusing to run");
        return;
    }

    // ---- a throwaway account, its password made here and never leaving memory -------------------
    let email = format!("roundtrip-{}@example.test", hex(&random_bytes(8)));
    let password = Secret::new(hex(&random_bytes(24)));
    let profile = Profile { display_name: "Round-trip Test".to_string(), school: "Example Middle School".to_string(), adult_school_employee: true };
    eprintln!("partial_send_rig_round_trip: throwaway account {email} on {RIG_BASE}");

    let client = Client::new(RIG_BASE, Mode::Native, Box::new(NativeTransport::default()), Box::new(MemoryVault::default()));

    block_on(client.sign_up(&email, &password, &profile)).expect("sign-up reaches the local rig");

    let token = {
        let mut got = None;
        for _ in 0..60 {
            got = confirm_token(&email, &mail_log());
            if got.is_some() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(250));
        }
        got.unwrap_or_else(|| panic!("no confirm link for {email} appeared in {}", mail_log().display()))
    };
    let confirmed = block_on(client.confirm(&token, &password, &profile)).expect("the confirm link works");
    assert!(confirmed.password_set, "the confirm link set the password");

    // Only the throwaway address goes into this command; no secret does.
    let rig = PathBuf::from("server/worker/scripts/local-ui-rig.sh");
    let out = Command::new(&rig).arg("approve").arg(&email).output().expect("the rig script runs");
    assert!(
        out.status.success(),
        "approving {email} refused: {}",
        String::from_utf8_lossy(&out.stderr).trim()
    );
    eprintln!("partial_send_rig_round_trip: {}", String::from_utf8_lossy(&out.stdout).trim().lines().last().unwrap_or_default());

    block_on(client.sign_in(&email, &password)).expect("the approved throwaway account signs in");

    let pool = Pool::new(&client).with_retry(RetryPolicy::instant(2));
    let media: HashMap<String, Vec<u8>> = HashMap::new();
    let mut no_progress = |_p: super::pool::PublishProgress| {};

    let cid = block_on(pool.create_circle("Partial send round-trip", "A local rig check for docs/circles-phaseD-plan.md section 10."))
        .expect("the circle is created")
        .id;

    let mut local = bank();
    let first = CircleSend::new(bank_body(&local), License::CcBy, meta());
    let sent1 = block_on(pool.send_new_circle_bank(&cid, &first, &media, &mut no_progress)).expect("version 1 is sent");
    assert_eq!(sent1.version, 1);
    let bid = sent1.bank_id.clone();
    let qids = sent1.qids.clone();
    assert_eq!(qids.len(), local.questions.len());

    // ---- version 1 as the circle serves it: the base body and a link from it ---------------------
    let doc = block_on(pool.circle_version(&cid, &bid, 1)).expect("version 1 is read");
    // The circle ids are the payload's own question ids, in the bank's order (a circle body has no
    // `published.qid`).
    let served: Value = serde_json::from_slice(&doc.payload).expect("version 1 is JSON");
    let served_ids: Vec<String> = served
        .pointer("/bank/questions")
        .and_then(Value::as_array)
        .expect("version 1 has questions")
        .iter()
        .map(|q| q.get("id").and_then(Value::as_str).unwrap_or_default().to_string())
        .collect();
    assert_eq!(served_ids, qids, "the version's question ids are the send's qids");
    let base: Vec<BaseQuestion> = local
        .questions
        .iter()
        .zip(&served_ids)
        .map(|(q, qid)| base_of(q, qid, "", None))
        .collect();
    let link = CircleLink::from_head(&doc.circle_id, &doc.circle_name, &doc.bank, &local.id, base, super::browse::now_ms());
    assert_eq!(link.base_version, 1);
    assert_eq!(link.qid_of(&local.questions[0].id), Some(qids[0].as_str()));

    // ---- two changes to distinct questions, A ticked and B left unticked -------------------------
    local.questions[0].stem = "A, changed: what is 1 + 1?".to_string();
    local.questions[0].modified += 1;
    local.questions[1].stem = "B, changed: what is 2 + 2?".to_string();
    local.questions[1].modified += 1;
    let (a_local, b_local) = (local.questions[0].id.clone(), local.questions[1].id.clone());
    let (a_qid, b_qid) = (qids[0].clone(), qids[1].clone());

    let changes = changes_against_body(&link, &local, &doc.payload).expect("the changes are worked out");
    assert!(changes.changed.contains(&a_local), "A is a change");
    assert!(changes.changed.contains(&b_local), "B is a change");
    assert!(changes.added.is_empty() && changes.removed.is_empty(), "nothing was added or removed");

    let all_changes = changes.ids();
    let mut ticked = BTreeSet::new();
    ticked.insert(a_local.clone());
    let composed = compose_partial(&link, &local, &doc.payload, &all_changes, &ticked).expect("the partial payload is composed");

    let mut second = CircleSend::new(composed, License::CcBy, meta());
    second.base_version = Some(1);
    second.message = "A only; B left as the circle has it.".to_string();
    assert!(second.message.chars().count() <= MESSAGE_MAX);
    let sent2 = block_on(pool.send_circle_version(&cid, &bid, &second, &media, &mut no_progress)).expect("version 2 is sent");

    eprintln!("partial_send_rig_round_trip: server reported version {} added={} changed={} removed={}", sent2.version, sent2.added, sent2.changed, sent2.removed);

    // ---- what the server counted: exactly one change, B unchanged ---------------------------------
    assert_eq!(sent2.version, 2);
    assert_eq!(sent2.added, 0, "added");
    assert_eq!(sent2.removed, 0, "removed");
    assert_eq!(sent2.changed, 1, "the server must count exactly the ticked change A");

    let v1 = questions_by_id(&doc.payload);
    let doc2 = block_on(pool.circle_version(&cid, &bid, 2)).expect("version 2 is read");
    let v2 = questions_by_id(&doc2.payload);
    let a1 = v1.get(&a_qid).unwrap_or_else(|| panic!("version 1 has A's question"));
    let a2 = v2.get(&a_qid).unwrap_or_else(|| panic!("version 2 has A's question"));
    let b1 = v1.get(&b_qid).unwrap_or_else(|| panic!("version 1 has B's question"));
    let b2 = v2.get(&b_qid).unwrap_or_else(|| panic!("version 2 has B's question"));
    assert_ne!(a1, a2, "A's question changed");
    assert_eq!(b2, b1, "B's raw base JSON round-tripped as unchanged");

    // ---- the throwaway session ends; the password and token are never written or printed ---------
    let sign_out = client.begin_sign_out(true);
    let _ = client.wipe_signed_out(&sign_out, false);
    eprintln!("partial_send_rig_round_trip: PASS (changed == 1; B identical to version 1)");
}

/// Gives the throwaway staging account teacher trust and pool access, the way the local rig's
/// `approve` does. Only the address goes into the command; no secret does.
fn staging_approve(email: &str) -> Result<(), String> {
    // The address is this test's own hex-made string: no quotes or escapes can reach the SQL.
    assert!(email.bytes().all(|b| b.is_ascii_alphanumeric() || b"-_@. ".contains(&b)), "the staging address is SQL-safe");
    let now_ms = super::browse::now_ms();
    let sql = format!(
        "UPDATE users SET trust='teacher' WHERE email='{email}'; \
         INSERT OR IGNORE INTO pool_invite_redemptions (user_id, invite_id, at) \
         SELECT id, NULL, {now_ms} FROM users WHERE email='{email}';"
    );
    // The coordinator's staging approve command, run from the Worker's package directory so
    // `wrangler.staging.toml` resolves. Read-only against nothing; only the address is passed.
    let out = Command::new("npx")
        .current_dir("server/worker")
        .args(["wrangler", "d1", "execute", "vershion-pool-staging", "--remote", "--config", "wrangler.staging.toml", "--command", &sql])
        .output()
        .map_err(|e| format!("the staging approve command could not run: {e}"))?;
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    if !out.status.success() {
        return Err(format!("the staging approve command failed ({}): {}", out.status, stderr.trim()));
    }
    if !stdout.contains("\"success\": true") {
        return Err(format!("the staging approve command did not report success: {}", stdout.trim()));
    }
    Ok(())
}

/// The same round-trip as [`partial_send_rig_round_trip`], against a local `wrangler dev --remote`
/// session proxying the STAGING bindings (18793). Nothing it makes is deleted here: it prints the
/// email, circle, bank and version ids so the staging rows and R2 objects can be cleaned up after.
#[test]
#[ignore = "needs the staging remote dev on 18793"]
fn partial_send_staging_round_trip() {
    assert_eq!(STAGING_BASE, "http://localhost:18793", "the staging test may only talk to the local staging proxy");
    assert_ne!(STAGING_BASE, RIG_BASE, "the staging test never touches the local rig");
    if std::env::var_os("VERSHION_POOL_URL").is_some_and(|v| v != STAGING_BASE) {
        eprintln!("partial_send_staging_round_trip: VERSHION_POOL_URL is not the staging proxy; refusing to run");
        return;
    }

    // ---- a throwaway account, its password made here and never leaving memory -------------------
    let email = format!("roundtrip-{}@example.test", hex(&random_bytes(8)));
    let password = Secret::new(hex(&random_bytes(24)));
    let profile = Profile { display_name: "Round-trip Test".to_string(), school: "Example Middle School".to_string(), adult_school_employee: true };
    eprintln!("partial_send_staging_round_trip: throwaway account {email} on {STAGING_BASE}");

    let client = Client::new(STAGING_BASE, Mode::Native, Box::new(NativeTransport::default()), Box::new(MemoryVault::default()));

    block_on(client.sign_up(&email, &password, &profile)).expect("sign-up reaches the staging remote dev session");

    let token = {
        let mut got = None;
        for _ in 0..120 {
            got = confirm_token(&email, &staging_mail_log());
            if got.is_some() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(250));
        }
        got.unwrap_or_else(|| panic!("no confirm link for {email} appeared in {}", staging_mail_log().display()))
    };
    let confirmed = block_on(client.confirm(&token, &password, &profile)).expect("the staging confirm link works");
    assert!(confirmed.password_set, "the staging confirm link set the password");

    staging_approve(&email).unwrap_or_else(|e| panic!("approving {email} on staging: {e}"));
    eprintln!("partial_send_staging_round_trip: approved {email} in the staging D1 (teacher + pool access)");

    block_on(client.sign_in(&email, &password)).expect("the approved staging throwaway account signs in");

    let pool = Pool::new(&client).with_retry(RetryPolicy::instant(2));
    let media: HashMap<String, Vec<u8>> = HashMap::new();
    let mut no_progress = |_p: super::pool::PublishProgress| {};

    let cid = block_on(pool.create_circle("Partial send staging round-trip", "A staging check for docs/circles-phaseD-plan.md section 10."))
        .expect("the staging circle is created")
        .id;
    eprintln!("partial_send_staging_round_trip: circle id {cid}");

    let mut local = bank();
    let first = CircleSend::new(bank_body(&local), License::CcBy, meta());
    let sent1 = block_on(pool.send_new_circle_bank(&cid, &first, &media, &mut no_progress)).expect("staging version 1 is sent");
    assert_eq!(sent1.version, 1);
    let bid = sent1.bank_id.clone();
    let qids = sent1.qids.clone();
    assert_eq!(qids.len(), local.questions.len());
    eprintln!("partial_send_staging_round_trip: bank id {bid}, version 1 id {}", sent1.version_id);

    // ---- version 1 as staging serves it: the base body and a link from it ------------------------
    let doc = block_on(pool.circle_version(&cid, &bid, 1)).expect("staging version 1 is read");
    let served: Value = serde_json::from_slice(&doc.payload).expect("staging version 1 is JSON");
    let served_ids: Vec<String> = served
        .pointer("/bank/questions")
        .and_then(Value::as_array)
        .expect("staging version 1 has questions")
        .iter()
        .map(|q| q.get("id").and_then(Value::as_str).unwrap_or_default().to_string())
        .collect();
    assert_eq!(served_ids, qids, "the staging version's question ids are the send's qids");
    let base: Vec<BaseQuestion> = local.questions.iter().zip(&served_ids).map(|(q, qid)| base_of(q, qid, "", None)).collect();
    let link = CircleLink::from_head(&doc.circle_id, &doc.circle_name, &doc.bank, &local.id, base, super::browse::now_ms());
    assert_eq!(link.base_version, 1);

    // ---- two changes to distinct questions, A ticked and B left unticked -------------------------
    local.questions[0].stem = "A, changed: what is 1 + 1?".to_string();
    local.questions[0].modified += 1;
    local.questions[1].stem = "B, changed: what is 2 + 2?".to_string();
    local.questions[1].modified += 1;
    let (a_local, b_local) = (local.questions[0].id.clone(), local.questions[1].id.clone());
    let (a_qid, b_qid) = (served_ids[0].clone(), served_ids[1].clone());

    let changes = changes_against_body(&link, &local, &doc.payload).expect("the staging changes are worked out");
    assert!(changes.changed.contains(&a_local), "A is a change");
    assert!(changes.changed.contains(&b_local), "B is a change");
    assert!(changes.added.is_empty() && changes.removed.is_empty(), "nothing was added or removed");

    let all_changes = changes.ids();
    let mut ticked = BTreeSet::new();
    ticked.insert(a_local.clone());
    let composed = compose_partial(&link, &local, &doc.payload, &all_changes, &ticked).expect("the staging partial payload is composed");

    let mut second = CircleSend::new(composed, License::CcBy, meta());
    second.base_version = Some(1);
    second.message = "A only; B left as the circle has it.".to_string();
    assert!(second.message.chars().count() <= MESSAGE_MAX);
    let sent2 = block_on(pool.send_circle_version(&cid, &bid, &second, &media, &mut no_progress)).expect("staging version 2 is sent");

    eprintln!(
        "partial_send_staging_round_trip: server reported version {} (id {}) added={} changed={} removed={}",
        sent2.version, sent2.version_id, sent2.added, sent2.changed, sent2.removed
    );

    // ---- what staging counted: exactly one change, B unchanged -----------------------------------
    assert_eq!(sent2.version, 2);
    assert_eq!(sent2.added, 0, "added");
    assert_eq!(sent2.removed, 0, "removed");
    assert_eq!(sent2.changed, 1, "the server must count exactly the ticked change A");

    let v1 = questions_by_id(&doc.payload);
    let doc2 = block_on(pool.circle_version(&cid, &bid, 2)).expect("staging version 2 is read");
    let v2 = questions_by_id(&doc2.payload);
    let a1 = v1.get(&a_qid).unwrap_or_else(|| panic!("staging version 1 has A's question"));
    let a2 = v2.get(&a_qid).unwrap_or_else(|| panic!("staging version 2 has A's question"));
    let b1 = v1.get(&b_qid).unwrap_or_else(|| panic!("staging version 1 has B's question"));
    let b2 = v2.get(&b_qid).unwrap_or_else(|| panic!("staging version 2 has B's question"));
    assert_ne!(a1, a2, "A's question changed");
    assert_eq!(b2, b1, "B's raw base JSON round-tripped as unchanged");

    // The rows are left for cleanup: this test deletes nothing on staging.
    eprintln!(
        "partial_send_staging_round_trip: CLEANUP account={email} circle={cid} bank={bid} version1={} version2={}",
        sent1.version_id, sent2.version_id
    );

    // ---- the throwaway session ends; the password and token are never written or printed ---------
    let sign_out = client.begin_sign_out(true);
    let _ = client.wipe_signed_out(&sign_out, false);
    eprintln!("partial_send_staging_round_trip: PASS (changed == 1; B identical to version 1)");
}
