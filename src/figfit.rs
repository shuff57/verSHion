//! Figure fitting: a picture cropped from an imported page becomes an editable Figures-tab
//! figure when one of the generators can reproduce it. Same shape as viSHual's generation
//! loop: the model proposes a structured spec, the app validates and renders it, the model
//! checks the render against the original, and one repair round fixes what it flagged.
//! When nothing fits, the caller keeps the plain cropped image.
use crate::ai::{ask_images, AiCfg};
use crate::figures::{self, FigureDef, Kind, Value as PV};
use crate::model::FigureSpec;
use serde_json::Value;

pub struct Fit {
    pub spec: FigureSpec,
    pub png: Vec<u8>,
}

fn catalog() -> String {
    figures::registry().iter().map(|d| format!("- {}: {} — {}", d.id, d.title, d.blurb)).collect::<Vec<_>>().join("\n")
}

fn param_help(def: &FigureDef) -> String {
    (def.params)()
        .iter()
        .map(|p| {
            let default = match &p.default {
                PV::F(v) => v.to_string(),
                PV::I(v) => v.to_string(),
                PV::B(v) => v.to_string(),
                PV::S(v) => format!("{v:?}"),
            };
            let kind = match &p.kind {
                Kind::Float { min, max, .. } => format!("number {min}..{max}"),
                Kind::Int { min, max } => format!("integer {min}..{max}"),
                Kind::Bool => "true/false".into(),
                Kind::Choice(o) => format!("one of {}", o.iter().map(|s| format!("{s:?}")).collect::<Vec<_>>().join(", ")),
                Kind::Text => "text".into(),
                Kind::Long => "multi-line text".into(),
                Kind::List => "list as text".into(),
                Kind::Data { columns: 1 } => "numbers as text, comma separated".into(),
                Kind::Data { .. } => "x, y rows as text, one pair per line".into(),
                Kind::Color(o) => format!("colour, one of {}", o.join(", ")),
            };
            format!("- {} ({kind}; default {default}): {}", p.key, p.label)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// `VERSHION_FIT_DEBUG=1` prints each step of the fitting loop to stderr.
fn debug(msg: &str) {
    if crate::env_var("VERSHION_FIT_DEBUG").is_ok() {
        eprintln!("[figfit] {msg}");
    }
}

fn ask_json(images: &[&[u8]], cfg: &AiCfg, prompt: &str) -> Result<Value, String> {
    let reply = ask_images(images, cfg, prompt, 0.0)?;
    crate::ditto::json_object(&reply).ok_or_else(|| "the model did not answer with JSON".to_string())
}

fn render(def: &FigureDef, params: &Value) -> Result<(FigureSpec, Vec<u8>), String> {
    let mut spec = FigureSpec { id: def.id.to_string(), params: Default::default() };
    for (k, v) in params.as_object().into_iter().flatten() {
        spec.params.insert(k.clone(), v.clone());
    }
    // params_from_spec coerces by declared kind, drops unknown keys, clamps to ranges.
    let p = crate::figures::spec::params_from_spec(def, &spec);
    let png = figures::to_png(&(def.render)(&p), 2.0)?;
    let png = crate::figures::spec::flatten_white(&png)?;
    Ok((crate::figures::spec::spec_for(def.id, &p), png))
}

/// What one fitting attempt produced.
enum Fitted {
    Found(Fit),
    /// The model answered, but no generator could reproduce the picture. The reason is fed to the
    /// next attempt.
    Miss(String),
}

/// One fitting attempt. `feedback` is what earlier attempts got wrong (empty on the first try).
fn fit_once(crop: &[u8], context: &str, cfg: &AiCfg, feedback: &str) -> Result<Fitted, String> {
    let pick = ask_json(
        &[crop],
        cfg,
        &format!(
            "This picture was cut from a test question: \"{}\".\nWhich generator can redraw THIS picture closely (same kind of drawing and same subject, not just the same subject area)?\n{}{feedback}\n\nAnswer JSON only: {{\"id\": \"<generator id>\" or null, \"reason\": \"...\"}}. Use null unless one clearly fits.",
            context.chars().take(200).collect::<String>(),
            catalog()
        ),
    )?;
    debug(&format!("pick: {pick}"));
    let Some(id) = pick.get("id").and_then(Value::as_str) else {
        return Ok(Fitted::Miss("the model named no generator".to_string()));
    };
    let Some(def) = figures::registry().into_iter().find(|d| d.id == id) else {
        return Ok(Fitted::Miss(format!("the model named a generator that does not exist ({id})")));
    };

    let mut feedback = feedback.to_string();
    let mut reason = String::new();
    for _round in 0..3 {
        let spec = ask_json(
            &[crop],
            cfg,
            &format!(
                "Set the \"{}\" generator so it redraws this picture. Question text: \"{}\".\nParameters:\n{}\n\nAnswer JSON only: {{\"reasoning\": \"two short sentences working out which part of the picture each parameter describes (for a triangle: which corner is the right angle, which side is the longest)\", \"params\": {{...}}}}, listing only the parameters that must differ from their defaults, with values of the stated kind. Every parameter you leave out keeps the default shown above, so when a default does not fit this picture (a preset angle or length it does not have), set it. Set a text parameter to \"\" (empty) to clear its default, which is how a measure the picture leaves out is \"worked out\" from the others. Check your values agree before answering: angles of a triangle add to 180, and the side opposite the largest angle (the hypotenuse, opposite a right angle) is the longest, so a 3 and a 4 are the two legs and the hypotenuse is the side left blank for the generator to work out. Match the picture's own labels exactly, units included (a side printed as \"3 cm\" needs the label text \"3 cm\"), and switch off every label, corner letter or measure the picture does not show. If the redraw shows a sentence of text instead of a drawing, the parameters contradict each other. When a parameter offers a list of options and one option names what the picture shows, choose that option instead of typing free text.{feedback}",
                def.title,
                context.chars().take(200).collect::<String>(),
                param_help(&def)
            ),
        )?;
        debug(&format!("params: {spec}"));
        let spec_json = spec.get("params").cloned().unwrap_or(Value::Null).to_string();
        let Ok((spec, png)) = render(&def, spec.get("params").unwrap_or(&Value::Null)) else {
            return Ok(Fitted::Miss(format!("the \"{}\" generator could not draw the parameters it proposed", def.title)));
        };
        let verdict = ask_json(
            &[crop, &png],
            cfg,
            "Image 1 is the original picture. Image 2 is a redraw made by a generator. Do they show the same thing: same molecule or structure, same labels and values, same layout? Ignore line thickness, font and exact position.\nAnswer JSON only: {\"same\": true or false, \"differences\": \"what is different, as a short instruction for fixing the parameters\"}",
        )?;
        debug(&format!("verdict: {verdict}"));
        if verdict.get("same").and_then(Value::as_bool) == Some(true) {
            return Ok(Fitted::Found(Fit { spec, png }));
        }
        reason = verdict.get("differences").and_then(Value::as_str).unwrap_or("it did not match").to_string();
        feedback.push_str(&format!(
            "\nAn earlier attempt with parameters {} was compared with the original and differed: {reason}",
            spec_json,
        ));
    }
    Ok(Fitted::Miss(format!("the \"{}\" redraw still did not match after 3 parameter repairs: {reason}", def.title)))
}

/// One fitting attempt; `Ok(None)` when no generator reproduces the picture closely enough.
pub fn fit(crop: &[u8], context: &str, cfg: &AiCfg) -> Result<Option<Fit>, String> {
    if crop.is_empty() {
        return Ok(None);
    }
    Ok(match fit_once(crop, context, cfg, "")? {
        Fitted::Found(f) => Some(f),
        Fitted::Miss(_) => None,
    })
}

/// Up to `tries` fitting attempts, each told why the ones before it failed. The import path asks
/// for 3, so a picture the first generator misses still gets a chance with another one.
pub fn fit_with_retries(crop: &[u8], context: &str, cfg: &AiCfg, tries: usize) -> Result<Option<Fit>, String> {
    if crop.is_empty() {
        return Ok(None);
    }
    retry_fit(tries, |feedback| fit_once(crop, context, cfg, feedback))
}

fn retry_fit(tries: usize, mut attempt: impl FnMut(&str) -> Result<Fitted, String>) -> Result<Option<Fit>, String> {
    let tries = tries.max(1);
    let mut feedback = String::new();
    let mut last_error: Option<String> = None;
    let mut answered = false;
    for _ in 0..tries {
        match attempt(&feedback) {
            Ok(Fitted::Found(f)) => return Ok(Some(f)),
            Ok(Fitted::Miss(reason)) => {
                answered = true;
                feedback.push_str(&format!(
                    "\nAn earlier attempt with this same picture failed: {reason}. Try a different generator, or parameters that match the picture more closely."
                ));
            }
            Err(e) => {
                last_error = Some(e);
                feedback.push_str("\nAn earlier attempt failed before it could draw anything.");
            }
        }
    }
    // A model that answered but found nothing is a real answer; only an all-error run is an error.
    if answered { Ok(None) } else { last_error.map_or(Ok(None), Err) }
}

/// Browser version of [`fit`], awaiting each model round trip instead of blocking.
#[cfg(target_arch = "wasm32")]
async fn ask_json_async(images: &[&[u8]], cfg: &AiCfg, prompt: &str) -> Result<Value, String> {
    let reply = crate::ai::ask_images_async(images, cfg, prompt, 0.0).await?;
    crate::ditto::json_object(&reply).ok_or_else(|| "the model did not answer with JSON".to_string())
}

/// One fitting attempt on the web: the same prompts and feedback as [`fit_once`], awaited.
#[cfg(target_arch = "wasm32")]
async fn fit_once_async(crop: &[u8], context: &str, cfg: &AiCfg, feedback: &str) -> Result<Fitted, String> {
    let pick = ask_json_async(
        &[crop],
        cfg,
        &format!(
            "This picture was cut from a test question: \"{}\".\nWhich generator can redraw THIS picture closely (same kind of drawing and same subject, not just the same subject area)?\n{}{feedback}\n\nAnswer JSON only: {{\"id\": \"<generator id>\" or null, \"reason\": \"...\"}}. Use null unless one clearly fits.",
            context.chars().take(200).collect::<String>(),
            catalog()
        ),
    )
    .await?;
    debug(&format!("pick: {pick}"));
    let Some(id) = pick.get("id").and_then(Value::as_str) else {
        return Ok(Fitted::Miss("the model named no generator".to_string()));
    };
    let Some(def) = figures::registry().into_iter().find(|d| d.id == id) else {
        return Ok(Fitted::Miss(format!("the model named a generator that does not exist ({id})")));
    };

    let mut feedback = feedback.to_string();
    let mut reason = String::new();
    for _round in 0..3 {
        let spec = ask_json_async(
            &[crop],
            cfg,
            &format!(
                "Set the \"{}\" generator so it redraws this picture. Question text: \"{}\".\nParameters:\n{}\n\nAnswer JSON only: {{\"reasoning\": \"two short sentences working out which part of the picture each parameter describes (for a triangle: which corner is the right angle, which side is the longest)\", \"params\": {{...}}}}, listing only the parameters that must differ from their defaults, with values of the stated kind. Every parameter you leave out keeps the default shown above, so when a default does not fit this picture (a preset angle or length it does not have), set it. Set a text parameter to \"\" (empty) to clear its default, which is how a measure the picture leaves out is \"worked out\" from the others. Check your values agree before answering: angles of a triangle add to 180, and the side opposite the largest angle (the hypotenuse, opposite a right angle) is the longest, so a 3 and a 4 are the two legs and the hypotenuse is the side left blank for the generator to work out. Match the picture's own labels exactly, units included (a side printed as \"3 cm\" needs the label text \"3 cm\"), and switch off every label, corner letter or measure the picture does not show. If the redraw shows a sentence of text instead of a drawing, the parameters contradict each other. When a parameter offers a list of options and one option names what the picture shows, choose that option instead of typing free text.{feedback}",
                def.title,
                context.chars().take(200).collect::<String>(),
                param_help(&def)
            ),
        )
        .await?;
        debug(&format!("params: {spec}"));
        let spec_json = spec.get("params").cloned().unwrap_or(Value::Null).to_string();
        let Ok((spec, png)) = render(&def, spec.get("params").unwrap_or(&Value::Null)) else {
            return Ok(Fitted::Miss(format!("the \"{}\" generator could not draw the parameters it proposed", def.title)));
        };
        let verdict = ask_json_async(
            &[crop, &png],
            cfg,
            "Image 1 is the original picture. Image 2 is a redraw made by a generator. Do they show the same thing: same molecule or structure, same labels and values, same layout? Ignore line thickness, font and exact position.\nAnswer JSON only: {\"same\": true or false, \"differences\": \"what is different, as a short instruction for fixing the parameters\"}",
        )
        .await?;
        debug(&format!("verdict: {verdict}"));
        if verdict.get("same").and_then(Value::as_bool) == Some(true) {
            return Ok(Fitted::Found(Fit { spec, png }));
        }
        reason = verdict.get("differences").and_then(Value::as_str).unwrap_or("it did not match").to_string();
        feedback.push_str(&format!(
            "\nAn earlier attempt with parameters {} was compared with the original and differed: {reason}",
            spec_json,
        ));
    }
    Ok(Fitted::Miss(format!("the \"{}\" redraw still did not match after 3 parameter repairs: {reason}", def.title)))
}

/// Browser version of [`fit`], awaiting each model round trip instead of blocking.
#[cfg(target_arch = "wasm32")]
pub async fn fit_async(crop: &[u8], context: &str, cfg: &AiCfg) -> Result<Option<Fit>, String> {
    if crop.is_empty() {
        return Ok(None);
    }
    Ok(match fit_once_async(crop, context, cfg, "").await? {
        Fitted::Found(f) => Some(f),
        Fitted::Miss(_) => None,
    })
}

/// Browser version of [`fit_with_retries`]: up to `tries` attempts, each told what the last missed.
#[cfg(target_arch = "wasm32")]
pub async fn fit_async_with_retries(crop: &[u8], context: &str, cfg: &AiCfg, tries: usize) -> Result<Option<Fit>, String> {
    if crop.is_empty() {
        return Ok(None);
    }
    let tries = tries.max(1);
    let mut feedback = String::new();
    let mut last_error: Option<String> = None;
    let mut answered = false;
    for _ in 0..tries {
        match fit_once_async(crop, context, cfg, &feedback).await {
            Ok(Fitted::Found(f)) => return Ok(Some(f)),
            Ok(Fitted::Miss(reason)) => {
                answered = true;
                feedback.push_str(&format!(
                    "\nAn earlier attempt with this same picture failed: {reason}. Try a different generator, or parameters that match the picture more closely."
                ));
            }
            Err(e) => {
                last_error = Some(e);
                feedback.push_str("\nAn earlier attempt failed before it could draw anything.");
            }
        }
    }
    if answered { Ok(None) } else { last_error.map_or(Ok(None), Err) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_lists_every_figure_and_lewis_exposes_its_presets() {
        assert_eq!(catalog().lines().count(), figures::registry().len());
        let lewis = figures::registry().into_iter().find(|d| d.id == "lewis-structures").unwrap();
        assert!(param_help(&lewis).contains("CH₃COCl, acetyl chloride"));
    }

    #[test]
    fn a_spec_renders_and_unknown_or_out_of_range_params_are_tolerated() {
        let lewis = figures::registry().into_iter().find(|d| d.id == "lewis-structures").unwrap();
        let params = serde_json::json!({"preset": "CH₃COCl, acetyl chloride", "bogus": 3, "lone": "yes"});
        let (spec, png) = render(&lewis, &params).unwrap();
        assert_eq!(spec.id, "lewis-structures");
        assert!(png.starts_with(b"\x89PNG"));
        assert!(!spec.params.contains_key("bogus"));
        assert_eq!(spec.params["preset"], "CH₃COCl, acetyl chloride");
    }

    /// A real, renderable figure, for the retry loop's success case.
    fn a_fit() -> Fit {
        let lewis = figures::registry().into_iter().find(|d| d.id == "lewis-structures").unwrap();
        let (spec, png) = render(&lewis, &serde_json::json!({"preset": "CH₃COCl, acetyl chloride"})).unwrap();
        Fit { spec, png }
    }

    #[test]
    fn retries_stop_at_the_first_editable_figure_and_feed_each_miss_forward() {
        let mut prompts: Vec<String> = Vec::new();
        let got = retry_fit(3, |feedback| {
            prompts.push(feedback.to_string());
            if prompts.len() < 3 { Ok(Fitted::Miss(format!("miss {}", prompts.len()))) } else { Ok(Fitted::Found(a_fit())) }
        })
        .unwrap()
        .expect("the third attempt made a figure");
        assert_eq!(got.spec.id, "lewis-structures");
        assert_eq!(prompts.len(), 3);
        assert_eq!(prompts[0], "", "the first attempt is told nothing");
        assert!(prompts[1].contains("miss 1"));
        assert!(prompts[2].contains("miss 1") && prompts[2].contains("miss 2"), "feedback accumulates");
    }

    #[test]
    fn retries_give_up_after_the_number_of_tries() {
        let mut calls = 0;
        for tries in [1, 2, 3] {
            calls = 0;
            let r = retry_fit(tries, |_| {
                calls += 1;
                Ok(Fitted::Miss("no generator fits".to_string()))
            })
            .unwrap();
            assert!(r.is_none());
            assert_eq!(calls, tries);
        }
    }

    #[test]
    fn an_all_error_run_is_an_error_but_a_model_that_answered_is_not() {
        let e = match retry_fit(2, |_| Err("no model".to_string())) {
            Err(e) => e,
            Ok(_) => panic!("an all-error run must be an error"),
        };
        assert_eq!(e, "no model");
        let mut n = 0;
        let r = retry_fit(3, |_| {
            n += 1;
            if n == 1 { Err("flaky".to_string()) } else { Ok(Fitted::Miss("nothing fits".to_string())) }
        })
        .unwrap();
        assert!(r.is_none(), "one real 'nothing fits' answer is not an error");
    }
}
