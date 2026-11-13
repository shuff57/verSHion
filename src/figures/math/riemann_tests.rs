//! Tests for the Riemann sums figure: hand-worked values, the label sweep, the student copy, Print, handles, links.
use super::*;

pub(super) fn with(set: &[(&str, Value)]) -> Params {
    let defs = params();
    let mut p = Params::defaults(&defs);
    for (k, v) in set { p.0.insert(k.to_string(), v.clone()); }
    p.normalize(&defs, false);
    p
}
pub(super) fn s(v: &str) -> Value { Value::S(v.to_string()) }

