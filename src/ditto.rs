//! verSHion Ditto package (`.ditto.json`, see `docs/ditto-format.md`): what the AI conversion
//! prompt (`prompts/ditto-convert.md`) produces. Import is tolerant: each question is checked
//! alone, a bad one is skipped with a warning, and figure boxes are cropped out of the page
//! image the model was shown.
use crate::formats::ImportDoc;
use crate::model::{Answer, Difficulty, Exam, ExamItem, Kind, Pair, Part, PartKind, Question};
use serde_json::Value;
use std::collections::HashMap;

/// Stable data-format identifier, deliberately unchanged by the verSHion rename: existing
/// `.ditto.json` files and the AI conversion prompt all carry `queshion/ditto`.
pub const FORMAT: &str = "queshion/ditto";

#[derive(Debug)]
pub struct DittoImport {
    pub doc: ImportDoc,
    /// One human sentence per skipped question, dropped figure or repaired field.
    pub warnings: Vec<String>,
    /// How many figures were cropped and attached.
    pub figures: usize,
    /// One entry per raw item of the reply, including the ones that were dropped.
    pub items: Vec<SourceItem>,
}

/// The JSON object inside a model reply, tolerating a code fence or chatter around it.
pub fn json_object(text: &str) -> Option<Value> {
    let (a, b) = (text.find('{')?, text.rfind('}')?);
    if b <= a {
        return None;
    }
    if let Ok(v) = serde_json::from_str(&text[a..=b]) {
        return Some(v);
    }
    first_value(&text[a..]).or_else(|| salvage(&text[a..]))
}

/// A reply that starts well and then breaks (a repetition loop until the token limit, a cut-off): the root object's members
/// that were completed before the break. The text is cut after the last member of the root that closed, and the root closed.
fn salvage(text: &str) -> Option<Value> {
    // `open` holds the closers still owed; `last` is the end of the latest bracket that closed, with the closers owed then.
    let (mut open, mut in_str, mut esc, mut last): (Vec<char>, bool, bool, Option<(usize, String)>) = (Vec::new(), false, false, None);
    for (i, c) in text.char_indices() {
        if in_str {
            if esc {
                esc = false;
            } else if c == '\\' {
                esc = true;
            } else if c == '"' {
                in_str = false;
            }
            continue;
        }
        match c {
            '"' => in_str = true,
            '{' => open.push('}'),
            '[' => open.push(']'),
            '}' | ']' => {
                if open.pop() != Some(c) {
                    break;
                }
                if open.is_empty() {
                    break;
                }
                last = Some((i + c.len_utf8(), open.iter().rev().collect()));
            }
            _ => {}
        }
    }
    let (end, owed) = last?;
    let cut = text[..end].trim_end().trim_end_matches(',');
    serde_json::from_str::<Value>(&format!("{cut}{owed}")).ok().filter(Value::is_object)
}

/// The first complete JSON value in `text`, ignoring whatever follows it. A model sometimes closes the root
/// object one brace early (before `"report"`); everything that matters is already inside by then.
fn first_value(text: &str) -> Option<Value> {
    let mut values = serde_json::Deserializer::from_str(text).into_iter::<Value>();
    let mut root = values.next()?.ok().filter(Value::is_object)?;
    // A root closed early may still have `"exams"` and `"report"` after it, as members that now sit outside
    // the object (a worksheet's directions live in `exams`). Read them as members of that root, never over
    // a member the root already has.
    let tail = text[values.byte_offset()..].trim_start().trim_start_matches(',').trim_start();
    if tail.starts_with('"')
        && let Some(Value::Object(extra)) = serde_json::Deserializer::from_str(&format!("{{{tail}")).into_iter::<Value>().next().and_then(Result::ok)
        && let Some(members) = root.as_object_mut()
    {
        for (key, value) in extra {
            members.entry(key).or_insert(value);
        }
    }
    Some(root)
}

/// Why a reply isn't usable JSON, or None when it parses (used to ask the model to retry).
pub fn json_error(text: &str) -> Option<String> {
    let (Some(a), Some(b)) = (text.find('{'), text.rfind('}')) else {
        return Some("there was no JSON object in the reply".into());
    };
    if b <= a {
        return Some("the JSON object was not closed".into());
    }
    match serde_json::from_str::<Value>(&text[a..=b]) {
        Ok(_) => None,
        Err(_) if first_value(&text[a..]).is_some() || salvage(&text[a..]).is_some() => None,
        Err(e) => Some(e.to_string()),
    }
}

pub fn is_ditto(text: &str) -> bool {
    json_object(text).is_some_and(|v| v.get("format").and_then(Value::as_str) == Some(FORMAT))
}

/// Shown as the task of a source figure whose printed task directions are not visible anywhere the
/// importer can see (a figure-only item on a continuation page imported alone). It is an explicit
/// review state, never a guessed task, and it deliberately names no mathematical verb.
pub const UNRESOLVED_TASK: &str = "[Source task not visible on this page. Review before using this question.]";
/// Topic tag that marks a question imported with [`UNRESOLVED_TASK`].
pub const UNRESOLVED_TOPIC: &str = "needs review: source task not visible";

/// What the importer knows about one raw item of the model's reply: stable provenance that survives the
/// items that were dropped, so a source number is never inferred by zipping lists of different lengths.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
pub struct SourceItem {
    /// Position of the item in the reply's `bank.questions`.
    pub index: usize,
    pub id: Option<String>,
    /// The printed label exactly as the model wrote it.
    pub label: Option<String>,
    /// The printed question number when the label is a plain number.
    pub number: Option<u32>,
    /// `saved`, `saved_figure_only`, `saved_figure_only_unresolved`,
    /// `saved_figure_box_unresolved`, `saved_figure_ownership_unresolved` or `dropped`.
    pub status: String,
    /// Index into the parsed question list of this page when it was saved.
    pub question_index: Option<usize>,
    pub reason: Option<String>,
    /// `none`, `attached`, `attached_needs_review`, `unsupported_needs_review` or `dropped`.
    pub figure: String,
    /// The raw owner of the one source figure attached to the parent image slot.
    pub figure_source: Option<FigureSource>,
    /// Explicit unsupported/invalid ownership or crop diagnostics; never a successful redraw.
    pub figure_review: Vec<String>,
    /// Literal section headings mapped to this item, distinct from its task directions.
    pub headings: Vec<String>,
    /// The task directions applied to this item and where they came from.
    pub directions: Option<DirectionSource>,
}

/// Source ownership is kept independently of the parent image slot used by stored questions.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct FigureSource {
    /// `question` or `multipart_part`.
    pub origin: String,
    /// JSON pointer within the original raw question, for example `/parts/0/figure`.
    pub path: String,
    /// Zero-based index of the immediate raw multipart part, when that part owns the figure.
    pub part_index: Option<usize>,
}

pub struct SelectedFigure<'a> {
    pub figure: &'a Value,
    pub source: FigureSource,
}

fn picture_markers(stem: &str) -> usize {
    crate::bankops::split_pictures(stem).iter().filter(|p| matches!(p, crate::bankops::StemPart::Picture(_))).count()
}

/// The one unambiguous figure this storage format can retain. Multiple sources, including
/// identical boxes assigned to different parts, require review: their ownership is not guessed.
/// This selector never mutates source text or unions boxes and is shared by preflight/import.
pub fn source_figure(item: &Value) -> Result<Option<SelectedFigure<'_>>, String> {
    fn children<'a>(v: &'a Value, prefix: &str, out: &mut Vec<(String, usize, &'a Value)>) {
        for (i, p) in v.get("parts").and_then(Value::as_array).into_iter().flatten().enumerate() {
            let path = format!("{prefix}/parts/{i}");
            if let Some(fig) = p.get("figure").filter(|f| !f.is_null()) { out.push((format!("{path}/figure"), i, fig)); }
            children(p, &path, out);
        }
    }
    let parent = item.get("figure").filter(|f| !f.is_null());
    let mut child = Vec::new();
    children(item, "", &mut child);
    if child.is_empty() {
        return Ok(parent.map(|figure| SelectedFigure { figure, source: FigureSource { origin: "question".into(), path: "/figure".into(), part_index: None } }));
    }
    let paths = child.iter().map(|x| x.0.as_str()).collect::<Vec<_>>().join(", ");
    if parent.is_some() || child.len() != 1 {
        return Err(format!("source figure ownership requires review: this question has {} parent and {} child figures ({paths}); one stored image cannot safely represent all of them", usize::from(parent.is_some()), child.len()));
    }
    let (path, index, figure) = child.pop().expect("one child");
    if text(item, "type") != "multipart" || path != format!("/parts/{index}/figure") {
        return Err(format!("source figure ownership requires review: {path} is not an immediate supported multipart part"));
    }
    let parts = item["parts"].as_array().expect("child exists");
    if part(&parts[index]).is_none() {
        return Err(format!("source figure ownership requires review: part {index} at {path} cannot be retained as a supported populated part"));
    }
    if picture_markers(text(item, "stem")) != 0 || parts.iter().enumerate().any(|(i,p)| picture_markers(text(p, "stem")) > usize::from(i == index)) {
        return Err(format!("source figure ownership requires review: a picture marker outside the sole owning part {index}, or multiple markers in that part, conflicts with {path}"));
    }
    Ok(Some(SelectedFigure { figure, source: FigureSource { origin: "multipart_part".into(), path, part_index: Some(index) } }))
}

/// Normalize only the uniquely owned child's figure into the parent's existing image slot.
/// The raw owner path stays in SourceItem; its part marker survives ordinary bank serialization.
fn item_with_source_figure(item: &Value, selected: Option<&SelectedFigure<'_>>) -> Value {
    let mut normalized = item.clone();
    if let Some(selected) = selected {
        normalized["figure"] = selected.figure.clone();
        if let Some(index) = selected.source.part_index {
            let stem = text(&item["parts"][index], "stem");
            if picture_markers(stem) == 0 {
                normalized["parts"][index]["stem"] = Value::String(format!("{stem}\n[picture]"));
            }
        }
    }
    normalized
}

/// Where a mapped task direction came from.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct DirectionSource {
    pub text: String,
    /// `printed_on_page`, or `inherited_from_document_context` for a direction carried from an earlier page of the same document.
    pub origin: String,
    pub from_page: Option<usize>,
    pub document: Option<String>,
}

/// One task direction an earlier page of the same document printed (or itself inherited).
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct ContextDirection {
    pub text: String,
    /// The page that printed it.
    pub page: usize,
    pub labels: Vec<String>,
    /// Literal section qualifiers printed with this task on its source page.
    pub headings: Vec<String>,
    pub heading_page: Option<usize>,
}

/// Explicit source-derived context for one page of a multi-page document: the directions earlier pages of
/// the SAME document printed, oldest first. It is empty for the first page and is rebuilt per document, so
/// nothing crosses a document boundary.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
pub struct DocContext {
    pub document: String,
    pub directions: Vec<ContextDirection>,
}

impl DocContext {
    pub fn is_empty(&self) -> bool {
        self.directions.is_empty()
    }

    /// The reading instructions appended to the conversion prompt; empty when there is nothing to carry.
    pub fn prompt_block(&self, page_no: usize) -> String {
        if self.directions.is_empty() {
            return String::new();
        }
        let mut s = format!("DOCUMENT CONTEXT: this image is page {page_no} of one source document. Earlier pages of THIS document (and no other document) printed these task directions, oldest first:\n");
        for d in &self.directions {
            let labels = if d.labels.is_empty() { String::new() } else { format!(", questions {}", range_text(&d.labels)) };
            s.push_str(&format!("- printed on page {}{labels}: {}\n", d.page, serde_json::Value::String(d.text.clone())));
            if !d.headings.is_empty() {s.push_str(&format!("  source section headings (printed on page {}): {}\n",d.heading_page.unwrap_or(d.page),serde_json::json!(d.headings)));}
        }
        s.push_str("Rules for this page:\n\
- Directions printed on THIS page always win. Questions under a heading or directions printed on this page use those, never the list above.\n\
- If this page begins numbered problems with no directions printed above them, they continue the LAST direction listed above. Put that exact text in `exams[].sections[].instructions`, map the questions with `positions`, preserve its listed source headings in `title`, and set that section's `\"inheritedFromPage\"` to the page number shown for it.\n\
- Never use a listed direction for a problem type it does not describe. If the task is still unclear, convert what is printed and say so in `report.notes`; do not guess.\n\
- Never invent directions that are neither printed on this page nor listed above.");
        s
    }

    /// The context for the next page: what this one learned, most recent last, at most four distinct directions.
    pub fn after_page(&self, page_no: usize, items: &[SourceItem]) -> DocContext {
        let mut next = self.clone();
        let mut order: Vec<(String, usize, Vec<String>, Vec<String>, Option<usize>)> = Vec::new();
        for item in items.iter().filter(|i| i.question_index.is_some()) {
            let Some(d) = &item.directions else { continue };
            let page = d.from_page.unwrap_or(page_no);
            // A continuation's labels were not printed on the originating page. Keep the original
            // labels rather than attributing the continuation's question numbers to that earlier page.
            let (labels, headings, heading_page) = if d.origin == "inherited_from_document_context" {
                let Some(original) = self.directions.iter().find(|old| old.page == page && old.text.trim() == d.text.trim()) else { continue };
                let same=item.headings.is_empty()||item.headings.join("\n")==original.headings.join("\n");
                (original.labels.clone(),if same{original.headings.clone()}else{item.headings.clone()},if same{original.heading_page}else{Some(page_no)})
            } else {
                (item.label.iter().cloned().collect(),item.headings.clone(),(!item.headings.is_empty()).then_some(page_no))
            };
            if let Some(at)=order.iter().position(|(t,p,_,h,hp)|*p==page&&t.trim()==d.text.trim()&&*h==headings&&*hp==heading_page) {
                let mut group=order.remove(at);
                for label in labels {if !group.2.contains(&label) {group.2.push(label);}}
                order.push(group);
            } else {order.push((d.text.clone(),page,labels,headings,heading_page));}

        }
        for (text, page, labels, headings, heading_page) in order {
            next.directions.retain(|d| d.text.trim() != text.trim());
            let labels = labels.into_iter().filter(|l| !l.is_empty()).collect();
            next.directions.push(ContextDirection { text, page, labels, headings, heading_page });
        }
        let extra = next.directions.len().saturating_sub(4);
        next.directions.drain(..extra);
        next
    }
}

/// Existing questions without any usable or conflicting explicit section mapping. The model
/// may confirm continuation for these indices; this function never assigns a task itself.
pub fn continuation_targets(reply: &str, context: &DocContext, page_no: usize) -> Vec<usize> {
    if context.directions.is_empty() {return Vec::new();}
    unmapped_direction_targets(reply, page_no, Some(context))
}

fn unmapped_direction_targets(reply: &str, page_no: usize, context: Option<&DocContext>) -> Vec<usize> {
    let Some(mut v)=json_object(reply) else {return Vec::new()};
    if v.get("exams").is_none() {if let Some(exams)=v.pointer("/bank/exams").cloned() {v["exams"]=exams;}}
    let mapped=Directions::read(&v,page_no,context,&mut Vec::new());
    let Some(questions)=v.pointer("/bank/questions").and_then(Value::as_array) else {return Vec::new()};
    questions.iter().enumerate().filter_map(|(index,q)| {
        let id=q.get("id").and_then(Value::as_str).filter(|s|!s.is_empty())?;
        let unique=questions.iter().filter(|q|q.get("id").and_then(Value::as_str)==Some(id)).count()==1;
        (unique && !mapped.by_source_id.contains_key(id) && !mapped.invalid.iter().any(|s|s==id)).then_some(index)
    }).collect()
}

/// Every otherwise eligible unmapped item may have lost a printed current-page
/// task, including bare expressions or prose. Existing/invalid/conflicting
/// mappings remain protected; no task is inferred from question or figure type.
pub fn current_page_direction_targets(reply: &str, page_no: usize, context: Option<&DocContext>) -> Vec<usize> {
    if page_no == 0 { return Vec::new(); }
    unmapped_direction_targets(reply,page_no,context)
}

/// Append only reviewed literal current-page task mappings. Question payloads,
/// answers, figures, headings and existing exam entries are never rewritten.
pub fn recover_printed_direction_mappings(reply: &str, page_no: usize, context: Option<&DocContext>, directions: &[String], mappings: &[(usize,usize)]) -> Result<String,String> {
    let allowed=current_page_direction_targets(reply,page_no,context);
    if directions.iter().any(|s|s.trim().is_empty()) {return Err("Printed directions must contain literal nonempty source text.".into());}
    let mut v=json_object(reply).ok_or("No source object.")?;
    let questions=v.pointer("/bank/questions").and_then(Value::as_array).ok_or("No source questions.")?;
    let mut additions=Vec::new();let mut seen=Vec::new();
    for &(index,direction) in mappings {
        if !allowed.contains(&index)||seen.contains(&index) {return Err("Current-page repair must identify an eligible unmapped item once only.".into());}seen.push(index);
        let text=directions.get(direction).ok_or("Unknown current-page direction index.")?;
        let id=questions[index]["id"].as_str().expect("validated unique source id");
        additions.push(serde_json::json!({"name":"","sections":[{"title":"","instructions":text,"inheritedFromPage":null}],"positions":[{"question":id,"section":0}]}));
    }
    if additions.is_empty() {return Ok(reply.into());}
    if v.get("exams").is_none() {v["exams"]=v.pointer("/bank/exams").cloned().unwrap_or_else(||serde_json::json!([]));}
    v["exams"].as_array_mut().ok_or("Source exams are not a list.")?.extend(additions);
    serde_json::to_string(&v).map_err(|e|e.to_string())
}

/// Application-owned merge: a reviewed item may select only one direction already printed
/// earlier in this document. No source stem, answer, figure, identity or existing mapping changes.
pub fn inherit_direction_mappings(reply: &str, context: &DocContext, page_no: usize, mappings: &[(usize,usize)]) -> Result<String,String> {
    let allowed=continuation_targets(reply,context,page_no);
    let mut v=json_object(reply).ok_or("No source object.")?;
    if v.get("exams").is_none() {if let Some(exams)=v.pointer("/bank/exams").cloned() {v["exams"]=exams;}}
    let current=Directions::read(&v,page_no,Some(context),&mut Vec::new());
    let questions=v.pointer("/bank/questions").and_then(Value::as_array).ok_or("No source questions.")?;
    let mut exams=Vec::new();let mut seen=Vec::new();
    for &(index,direction) in mappings {
        if !allowed.contains(&index)||seen.contains(&index) {return Err("Direction repair does not identify one unmapped source item.".into());}seen.push(index);
        let d=context.directions.get(direction).filter(|d|d.page>0&&d.page<page_no&&!d.text.trim().is_empty()).ok_or("Direction is not from an earlier page of this document.")?;
        let id=questions[index]["id"].as_str().expect("validated source id");
        let title=if current.headings.contains_key(id) {String::new()} else {d.headings.join("\n")};
        exams.push(serde_json::json!({"name":"","sections":[{"title":title,"instructions":d.text,"inheritedFromPage":d.page}],"positions":[{"question":id,"section":0}]}));
    }
    if exams.is_empty() {return Ok(reply.into());}
    if v.get("exams").is_none() {v["exams"]=v.pointer("/bank/exams").cloned().unwrap_or_else(||serde_json::json!([]));}
    v["exams"].as_array_mut().ok_or("Source exams are not a list.")?.extend(exams);
    serde_json::to_string(&v).map_err(|e|e.to_string())
}

fn range_text(labels: &[String]) -> String {
    let numbers:Option<Vec<_>>=labels.iter().map(|s|label_number(s)).collect();
    if labels.len()>1 && numbers.is_some_and(|n|n.windows(2).all(|w|w[0].checked_add(1)==Some(w[1]))) {
        format!("{}-{}",labels.first().unwrap(),labels.last().unwrap())
    } else {labels.join(", ")}
}

/// The printed question number in a label such as `13`, `13.`, `13)` or `Question 13`.
pub fn label_number(label: &str) -> Option<u32> {
    let t = label.trim();
    let t = t.get(..9).filter(|p| p.eq_ignore_ascii_case("question ")).map_or(t, |_| t[9..].trim_start());
    t.trim_end_matches(['.', ')']).trim().parse().ok()
}

fn raw_label(item: &Value, index: usize) -> (String, Option<String>) {
    let printed = match item.get("label") {
        Some(Value::String(s)) if !s.trim().is_empty() => Some(s.trim().to_string()),
        Some(Value::Number(n)) => Some(n.to_string()),
        _ => None,
    };
    (printed.clone().unwrap_or_else(|| format!("#{}", index + 1)), printed)
}

/// Parses ditto JSON. `page_png` is the page raster the figure boxes refer to (`page_no`
/// is its 1-based number); with no raster, figures are dropped with a warning.
pub fn import_text(text: &str, page_png: Option<&[u8]>, page_no: usize, fallback_name: &str) -> Result<DittoImport, String> {
    import_text_in(text, page_png, page_no, fallback_name, None)
}

/// [`import_text`] with the directions earlier pages of the same document printed. A direction the reply
/// marks as inherited is accepted only when it is exactly one the context lists.
pub fn import_text_in(reply: &str, page_png: Option<&[u8]>, page_no: usize, fallback_name: &str, context: Option<&DocContext>) -> Result<DittoImport, String> {
    let mut v = json_object(reply).ok_or("The reply was not a JSON object.")?;
    if v.get("format").and_then(Value::as_str) != Some(FORMAT) {
        return Err(format!("Not a verSHion Ditto file (format should be \"{FORMAT}\")."));
    }
    // `bank` left open by the model (its `}` missing before `"exams"`) holds the exams and report as its own members.
    for key in ["exams", "report"] {
        if v.get(key).is_none()
            && let Some(inner) = v.pointer(&format!("/bank/{key}")).cloned()
        {
            v[key] = inner;
        }
    }
    let bank = v.get("bank").ok_or("The file has no \"bank\".")?;
    let name = bank.get("name").and_then(Value::as_str).filter(|n| !n.trim().is_empty()).unwrap_or(fallback_name).to_string();
    let raw = bank.get("questions").and_then(Value::as_array).ok_or("The bank has no \"questions\" list.")?;

    let page = page_png.and_then(|b| image::load_from_memory(b).ok());
    let box_issues = figure_box_issues_in(&v, page.as_ref(), page_no);
    let mut warnings = Vec::new();
    let mut directions = Directions::read(&v, page_no, context, &mut warnings);
    let mut questions = Vec::new();
    let mut ids: HashMap<String, String> = HashMap::new();
    let mut items = Vec::new();
    let mut figures = 0;
    for (i, item) in raw.iter().enumerate() {
        let (label, printed) = raw_label(item, i);
        let source_id = item.get("id").and_then(Value::as_str).map(str::to_string);
        let mut info = SourceItem { index: i, id: source_id.clone(), number: printed.as_deref().and_then(label_number), label: printed, figure: "none".into(), status: "dropped".into(), ..Default::default() };
        let selection = source_figure(item);
        let ownership_review = selection.as_ref().err().cloned();
        let selected = selection.as_ref().ok().and_then(Option::as_ref);
        info.figure_source = selected.map(|s| s.source.clone());
        let normalized = item_with_source_figure(item, selected);
        let item = &normalized;
        let box_review: Vec<_> = box_issues.iter().filter(|issue| issue.kind == "overlap" && issue.indices.contains(&i)).collect();
        info.figure_review = box_issues.iter().filter(|issue| issue.indices.contains(&i)).map(|issue| issue.message.clone()).collect();
        if let Some(reason) = &ownership_review {
            warnings.push(format!("Question {label}: {reason}; retained its text and parts for review without choosing a figure."));
            info.figure = "unsupported_needs_review".into();
            info.reason = Some(reason.clone());
        }
        let cropped = selected.map(|_| crop(&item["figure"], page.as_ref(), page_no));
        if let Some(Err(e)) = &cropped {
            warnings.push(format!("Question {label}: figure dropped ({e})."));
            info.figure = "dropped".into();
        }
        let applied = source_id.as_deref().and_then(|id| directions.single(id));
        // A figure-only item is a real question when its figure is a real figure: its task is the section's
        // printed direction, or an explicit unresolved-review state. Without a valid figure it stays dropped.
        let mut unresolved = false;
        let mut built = if text(item, "stem").is_empty() && matches!(&cropped, Some(Ok(_))) {
            let mut filled = item.clone();
            let stem = match &applied {
                Some(d) => d.text.clone(),
                None => {
                    unresolved = true;
                    warnings.push(format!("Question {label}: only its figure is printed and no task directions are visible; kept for review without inventing a task."));
                    UNRESOLVED_TASK.to_string()
                }
            };
            filled["stem"] = Value::String(stem);
            // Some readers label a figure-only problem multipart even though they explicitly
            // supply zero parts. Preserve the real diagram as one written-response problem;
            // never discard populated parts or manufacture subquestions/answers.
            if text(item, "type") == "multipart"
                && item.get("parts").and_then(Value::as_array).is_some_and(Vec::is_empty)
                && ["choices", "pairs", "blanks"].iter().all(|key| item.get(*key).is_none_or(|v| v.is_null() || v.as_array().is_some_and(Vec::is_empty)))
            {
                filled["type"] = Value::String("short-answer".into());
                warnings.push(format!("Question {label}: empty multipart wrapper repaired to a single figure-only written-response question; no parts or answers were invented."));
            }
            question(&filled)
        } else {
            question(item)
        };
        // Promotion cannot conceal an earlier invalid part and shift the raw owner index.
        // Keep a malformed multipart as an explicit dropped raw item instead of a truncated bank item.
        if selected.is_some_and(|s| s.source.part_index.is_some())
            && let Ok(q) = &built
            && let Kind::Multipart { parts } = &q.kind
            && item.get("parts").and_then(Value::as_array).is_some_and(|raw| raw.len() != parts.len())
        {
            built = Err("cannot preserve all multipart parts while promoting a child figure; review the original raw item instead of using a truncated question.".into());
        }
        if text(item, "stem").is_empty() && !matches!(&cropped, Some(Ok(_))) && built.is_err() {
            built = Err("it has no stem and no valid source figure.".into());
        }
        match built {
            Ok(mut q) => {
                if unresolved {
                    q.topics.push(UNRESOLVED_TOPIC.to_string());
                }
                if ownership_review.is_some() {
                    q.topics.push("needs review: source figure ownership".into());
                }
                if let Some(Ok(png)) = cropped {
                    match crate::store::put_image(&png) {
                        Ok(img) => {
                            q.image = Some(img);
                            figures += 1;
                            info.figure = "attached".into();
                        }
                        Err(e) => {
                            warnings.push(format!("Question {label}: couldn't save its figure ({e})."));
                            info.figure = "dropped".into();
                        }
                    }
                }
                if let Some(id) = &source_id {
                    ids.insert(id.clone(), q.id.clone());
                }
                info.status = if ownership_review.is_some() {
                    "saved_figure_ownership_unresolved"
                } else if !box_review.is_empty() && q.image.is_some() {
                    let reason = box_review.iter().map(|issue| issue.message.as_str()).collect::<Vec<_>>().join("; ");
                    warnings.push(format!("Question {label}: source figure box requires review ({reason}); kept the source crop without certifying its scope."));
                    info.figure = "attached_needs_review".into();
                    info.reason = Some(reason);
                    "saved_figure_box_unresolved"
                } else if unresolved { "saved_figure_only_unresolved" } else if text(item, "stem").is_empty() { "saved_figure_only" } else { "saved" }.into();
                info.question_index = Some(questions.len());
                info.directions = applied;
                info.headings = source_id.as_ref().and_then(|id| directions.headings.get(id)).cloned().unwrap_or_default();
                questions.push(q);
            }
            Err(e) => {
                warnings.push(format!("Question {label} skipped: {e}"));
                info.reason = Some(e);
            }
        }
        items.push(info);
    }
    if let Some(rep) = v.pointer("/report/unconverted").and_then(Value::as_array) {
        for u in rep {
            let l = u.get("label").map(|l| l.as_str().map(str::to_string).unwrap_or_else(|| l.to_string())).unwrap_or_default();
            let why = u.get("reason").and_then(Value::as_str).unwrap_or("unreadable");
            warnings.push(format!("The model left question {l} out: {why}."));
        }
    }
    directions.apply(&ids, &mut questions, &mut warnings);
    let exams = exams(&v, &ids);
    Ok(DittoImport { doc: ImportDoc { name, questions, exams }, warnings, figures, items })
}

fn instruction_key(s: &str) -> String {
    s.replace("**", "").split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}

/// The words of a text with math markup set aside: LaTeX command names (`\begin`, `\frac`) and
/// single letters are not words, so a plain-text and a typeset copy of one sentence read alike.
fn prose_words(s: &str) -> Vec<String> {
    let (mut out, mut word, mut command) = (Vec::new(), String::new(), false);
    for c in s.chars().chain(std::iter::once(' ')) {
        if c.is_alphabetic() {
            word.extend(c.to_lowercase());
            continue;
        }
        if word.chars().count() > 1 && !command {
            out.push(std::mem::take(&mut word));
        }
        word.clear();
        command = c == '\\';
    }
    out
}

/// The direction is already in the question (its stem or a part): an exact copy, or, for a
/// direction of at least four words, every one of its words in order. The second catches a reader
/// that returns a question's own sentence, in plain text, as the page's direction for it.
fn already_stated(instruction: &str, q: &Question) -> bool {
    let mut text = q.stem.clone();
    if let Kind::Multipart { parts } = &q.kind {
        for p in parts {
            text.push('\n');
            text.push_str(&p.stem);
        }
    }
    if instruction_key(&text).contains(&instruction_key(instruction)) {
        return true;
    }
    let want = prose_words(instruction);
    let mut have = prose_words(&text).into_iter();
    want.len() >= 4 && want.iter().all(|w| have.any(|h| &h == w))
}

/// The task directions a reply maps to its questions through `exams[].sections[].instructions` and
/// `positions`. Only explicit source section mappings supply directions; a title or a neighboring
/// question is never used to guess an instruction.
struct Directions {
    by_source_id: HashMap<String, Vec<DirectionSource>>,
    headings: HashMap<String, Vec<String>>,
    invalid: Vec<String>,
}

impl Directions {
    fn read(v: &Value, page_no: usize, context: Option<&DocContext>, warnings: &mut Vec<String>) -> Directions {
        let mut by_source_id: HashMap<String, Vec<DirectionSource>> = HashMap::new();
        let mut headings: HashMap<String, Vec<String>> = HashMap::new();
        let mut inherited_headings: HashMap<String, Vec<String>> = HashMap::new();
        let mut invalid = Vec::new();
        let document = context.map(|c| c.document.clone()).filter(|d| !d.is_empty());
        for exam in v.get("exams").and_then(Value::as_array).into_iter().flatten() {
            let Some(sections) = exam.get("sections").and_then(Value::as_array) else { continue };
            for position in exam.get("positions").and_then(Value::as_array).into_iter().flatten() {
                let Some(source_id) = position.get("question").and_then(Value::as_str) else { continue };
                // Old packages sometimes omit section for a single-section exam.
                let section = match position.get("section") {
                    None if sections.len() == 1 => Some(0),
                    Some(value) => value.as_u64().and_then(|n| usize::try_from(n).ok()),
                    _ => None,
                };
                let Some(section) = section.and_then(|i| sections.get(i)) else {
                    invalid.push(source_id.to_string());
                    continue;
                };
                let heading = text(section, "title");
                let inherited=section.get("inheritedFromPage").is_some_and(|p|!p.is_null());
                if !inherited && !heading.is_empty() {
                    let titles = headings.entry(source_id.to_string()).or_default();
                    if !titles.iter().any(|title| title == heading) { titles.push(heading.to_string()); }
                }
                let instruction = text(section, "instructions");
                if instruction.is_empty() {
                    continue;
                }
                let source = if let Some(claimed) = section.get("inheritedFromPage").filter(|p|!p.is_null()) {
                    let claimed = claimed.as_u64().and_then(|p| usize::try_from(p).ok());
                    let known = context.and_then(|c| c.directions.iter().rev().find(|d| {
                        Some(d.page) == claimed && d.page > 0 && d.page < page_no && d.text.trim() == instruction
                    }));
                    match known {
                        Some(d) => {
                            DirectionSource { text: instruction.to_string(), origin: "inherited_from_document_context".into(), from_page: Some(d.page), document: document.clone() }
                        },
                        None => {
                        warnings.push(format!("Question {source_id}: directions marked as inherited are not in the document context; not applied."));
                        continue;
                        }
                    }
                } else {
                    DirectionSource { text: instruction.to_string(), origin: "printed_on_page".into(), from_page: Some(page_no), document: document.clone() }
                };
                if inherited && !heading.is_empty() {
                    let titles=inherited_headings.entry(source_id.to_string()).or_default();
                    if !titles.iter().any(|t|t==heading) {titles.push(heading.to_string());}
                }
                let choices = by_source_id.entry(source_id.to_string()).or_default();
                if !choices.iter().any(|d| d.text.trim() == source.text.trim()) {
                    choices.push(source);
                }
            }
        }
        // Supplement only after every explicit heading has been read, so a later source
        // mapping on this page wins regardless of exam/position iteration order.
        for (id,choices) in &by_source_id {
            if headings.contains_key(id)||choices.len()!=1 {continue;}
            let d=&choices[0];
            if let Some(titles)=inherited_headings.get(id) {headings.insert(id.clone(),titles.clone());continue;}
            if d.origin=="inherited_from_document_context" && let Some(original)=context.and_then(|c|c.directions.iter().find(|old|Some(old.page)==d.from_page&&old.text.trim()==d.text.trim())) && !original.headings.is_empty() {
                headings.insert(id.clone(),original.headings.clone());
            }
        }
        Directions { by_source_id, headings, invalid }
    }

    /// The one direction mapped to a source item; none when there is none or the mapping conflicts.
    fn single(&self, source_id: &str) -> Option<DirectionSource> {
        let choices = self.by_source_id.get(source_id)?;
        (choices.len() == 1).then(|| choices[0].clone())
    }

    /// A bank item must retain the section task when reused outside the source exam.
    fn apply(&mut self, ids: &HashMap<String, String>, questions: &mut [Question], warnings: &mut Vec<String>) {
        for source_id in &self.invalid {
            if ids.contains_key(source_id) {
                warnings.push(format!("Question {source_id}: invalid source section mapping; review its directions."));
            }
        }
        let by_question: HashMap<&String, &String> = ids.iter().map(|(s, q)| (q, s)).collect();
        for q in questions {
            let Some(source_id) = by_question.get(&q.id) else { continue };
            if let Some(choices) = self.by_source_id.get(*source_id) {
                if choices.len() != 1 {
                    warnings.push("A question has conflicting source section directions; review it before reusing it in a bank.".into());
                } else {
                    let instruction = &choices[0].text;
                    if !already_stated(instruction, q) {
                        if matches!(q.kind, Kind::Blank { .. }) && instruction.contains("___") {
                            warnings.push("A source direction contains an example blank; review its context before reusing this fill-in question in a bank.".into());
                        } else {
                            q.stem = format!("{instruction}\n\n{}", q.stem);
                        }
                    }
                }
            }
            // A literal heading preserves the source's scope/category; it is never used to guess
            // a missing task. Empty instructions still leave a figure-only item unresolved.
            if let Some(headings) = self.headings.get(*source_id) {
                let missing: Vec<_> = headings.iter().filter(|heading| !q.stem.lines().any(|line| line.trim().trim_matches('*').trim() == heading.trim())).collect();
                if !missing.is_empty() {
                    q.stem = format!("{}\n\n{}", missing.iter().map(|s| s.as_str()).collect::<Vec<_>>().join("\n"), q.stem);
                }
            }
        }
    }
}

fn text<'a>(v: &'a Value, key: &str) -> &'a str {
    v.get(key).and_then(Value::as_str).unwrap_or("").trim()
}

fn question(item: &Value) -> Result<Question, String> {
    let stem = text(item, "stem").to_string();
    let ty = text(item, "type");
    if stem.is_empty() && !(ty == "multipart" && parts_carry_task(item)) {
        return Err("it has no stem.".into());
    }
    // Tables of blanks are cleaned for every type (blank questions also realign their answers).
    let stem = if ty == "fill-in-the-blank" { stem } else { normalize_tables(&stem, &[]).stem };
    let (stem, kind) = match ty {
        "multiple-choice" => (stem, Kind::Mc { answers: choices(item)? }),
        "true-false" => (stem, Kind::Tf { correct: item.get("answer").and_then(Value::as_bool).unwrap_or(true) }),
        "matching" => {
            let pairs: Vec<Pair> = item
                .get("pairs")
                .and_then(Value::as_array)
                .map(|a| a.iter().map(|p| Pair { left: text(p, "left").into(), right: text(p, "right").into() }).filter(|p| !p.left.is_empty() && !p.right.is_empty()).collect())
                .unwrap_or_default();
            if pairs.len() < 2 {
                return Err("a matching question needs at least two pairs.".into());
            }
            let distractors = strings(item.get("distractors"));
            (stem, Kind::Matching { pairs, distractors })
        }
        "fill-in-the-blank" => blank(stem, item),
        "short-answer" => (stem, Kind::Short { sample: text(item, "suggestedAnswer").to_string() }),
        "multipart" => {
            let parts: Vec<Part> = item.get("parts").and_then(Value::as_array).map(|a| a.iter().filter_map(part).collect()).unwrap_or_default();
            if parts.is_empty() {
                return Err("a multipart question needs at least one part.".into());
            }
            (stem, Kind::Multipart { parts })
        }
        other => return Err(format!("unknown type \"{other}\".")),
    };
    let mut q = crate::formats::make_question(stem, kind);
    q.points = item.get("points").and_then(Value::as_f64).filter(|p| *p > 0.0);
    q.difficulty = match text(item, "difficulty") {
        "easy" => Some(Difficulty::Low),
        "medium" => Some(Difficulty::Medium),
        "hard" => Some(Difficulty::High),
        _ => None,
    };
    q.topics = strings(item.get("topics"));
    q.explanation = Some(text(item, "explanation").to_string()).filter(|e| !e.is_empty());
    Ok(q)
}

/// A problem printed as "1) (a) ... (b) ..." has no shared stem: its parts carry the whole task.
/// That holds only when every raw part survives, so an empty stem never hides a lost part.
fn parts_carry_task(item: &Value) -> bool {
    item.get("parts").and_then(Value::as_array).is_some_and(|raw| !raw.is_empty() && raw.iter().all(|p| part(p).is_some()))
}

fn strings(v: Option<&Value>) -> Vec<String> {
    v.and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_str).map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()).unwrap_or_default()
}

fn choices(item: &Value) -> Result<Vec<Answer>, String> {
    let answers: Vec<Answer> = item
        .get("choices")
        .and_then(Value::as_array)
        .map(|a| a.iter().map(|c| Answer { text: text(c, "text").to_string(), correct: c.get("correct").and_then(Value::as_bool).unwrap_or(false), locked: false }).filter(|a| !a.text.is_empty()).collect())
        .unwrap_or_default();
    if answers.len() < 2 {
        return Err("a multiple-choice question needs at least two choices.".into());
    }
    Ok(answers)
}

/// A blank question: one accepted-answer list per `___` in the stem. A stem with no marker
/// can't be a blank question, so it degrades to short answer with the answers as the sample.
fn blank(stem: String, item: &Value) -> (String, Kind) {
    let mut accept: Vec<Vec<String>> = item.get("blanks").and_then(Value::as_array).map(|a| a.iter().map(|b| match b {
        Value::String(s) => vec![s.trim().to_string()],
        other => strings(Some(other)),
    }).collect()).unwrap_or_default();
    let before = stem.matches("___").count();
    let fix = normalize_tables(&stem, &[accept.iter().filter(|a| !a.is_empty()).count(), accept.iter().map(Vec::len).sum()]);
    let stem = fix.stem;
    let markers = stem.matches("___").count();
    if markers == 0 {
        let sample = accept.iter().filter(|a| !a.is_empty()).map(|a| a.join(" OR ")).collect::<Vec<_>>().join("; ");
        return (stem, Kind::Short { sample });
    }
    let basic = |mut acc: Vec<Vec<String>>| {
        // Trailing empty entries are padding, not answers (resize pads them back).
        while acc.last().is_some_and(Vec::is_empty) {
            acc.pop();
        }
        // Repair: the model grouped each row's answers into one entry ([[a, b], [c, d]] for
        // four blanks). When flattening them gives exactly one answer per blank, that was it.
        if acc.len() < markers && acc.iter().map(Vec::len).sum::<usize>() == markers {
            acc = acc.into_iter().flatten().map(|a| vec![a]).collect();
        }
        acc
    };
    let fitted = basic(accept.clone());
    accept = if fitted.len() == markers || accept.len() != before || fix.dropped.is_empty() {
        fitted
    } else {
        // Repair: the header row lost its blanks and the model gave one entry per ORIGINAL
        // blank (the header's ones empty or junk): drop exactly those entries.
        basic(accept.into_iter().enumerate().filter(|(i, _)| !fix.dropped.contains(i)).map(|(_, a)| a).collect())
    };
    accept.resize(markers, Vec::new());
    // A model sometimes repeats an alternative ([["Ionic", "Ionic"]]): keep each once.
    for alts in &mut accept {
        let mut seen: Vec<String> = Vec::new();
        alts.retain(|a| !seen.contains(a) && {
            seen.push(a.clone());
            true
        });
    }
    (stem, Kind::Blank { accept })
}

/// A stem after [`normalize_tables`], and which `___` markers of the original stem (0-based,
/// reading order) were removed because they sat in a header row.
#[derive(Debug, PartialEq)]
pub(crate) struct TableFix {
    pub stem: String,
    pub dropped: Vec<usize>,
}

#[derive(Clone, Debug, PartialEq)]
enum Cell {
    Blank,
    Labeled(String),
    Other(String),
}

fn classify(c: &str) -> Cell {
    let t = c.trim();
    let core = t.trim_end_matches('_');
    if t.len() - core.len() >= 3 {
        let label = core.trim().trim_end_matches(':').trim();
        if label.is_empty() {
            return Cell::Blank;
        }
        if !label.contains("___") {
            return Cell::Labeled(label.to_string());
        }
    }
    Cell::Other(t.to_string())
}

/// Cells of a pipe row (the same reading as `richtext`): None for a line with < 2 cells.
fn pipe_cells(line: &str) -> Option<Vec<String>> {
    if !line.contains('|') {
        return None;
    }
    let t = line.trim();
    let t = t.strip_prefix('|').unwrap_or(t);
    let t = t.strip_suffix('|').unwrap_or(t);
    let c: Vec<String> = t.split('|').map(|x| x.trim().to_string()).collect();
    (c.len() >= 2).then_some(c)
}

fn is_separator(c: &[String]) -> bool {
    c.iter().all(|x| !x.is_empty() && x.contains('-') && x.chars().all(|ch| matches!(ch, '-' | ':' | ' ')))
}

fn pipe_row(cells: &[String]) -> String {
    if cells.first().is_some_and(String::is_empty) || cells.last().is_some_and(String::is_empty) {
        format!("| {} |", cells.join(" | "))
    } else {
        cells.join(" | ")
    }
}

/// Words a first column's header cell typically says (used only when nothing else decides
/// whether the first row of a label-repeating table is a header).
fn looks_like_title(s: &str) -> bool {
    const TITLES: [&str; 24] = [
        "formula", "compound", "name", "symbol", "element", "term", "word", "item", "molecule", "substance", "ion", "question",
        "problem", "number", "#", "equation", "reaction", "sample", "object", "material", "quantity", "value", "unit", "chemical formula",
    ];
    let l = s.trim().trim_end_matches(':').to_lowercase();
    let l = l.strip_suffix('s').filter(|x| TITLES.contains(x)).map(str::to_string).unwrap_or(l);
    TITLES.contains(&l.as_str())
}

/// The common model slip in a pipe table of blanks: every row repeats the column's header
/// label in front of its blank (`KI | Ionic or Covalent? ___ | Name ___`), and the header row
/// itself may carry blanks (`Formula | Ionic or Covalent? ___ | Name ___`). Rewrites the table
/// to the clean shape — a header row WITHOUT blanks, body rows with a bare `___` per answer
/// cell — synthesising the header (first cells empty) when the source rows have none.
/// `answer_counts` are the model's answer counts (entries, flattened answers) used to tell a
/// header row from a data row when the rows look alike. Stems without such a table come back
/// unchanged.
pub(crate) fn normalize_tables(stem: &str, answer_counts: &[usize]) -> TableFix {
    let lines: Vec<&str> = stem.split('\n').collect();
    let total = stem.matches("___").count();
    let mut out: Vec<String> = Vec::new();
    let mut dropped: Vec<usize> = Vec::new();
    let mut markers_before = 0usize;
    let mut i = 0;
    while i < lines.len() {
        let Some(cols) = pipe_cells(lines[i]).map(|c| c.len()) else {
            markers_before += lines[i].matches("___").count();
            out.push(lines[i].to_string());
            i += 1;
            continue;
        };
        let mut j = i;
        while j < lines.len() && pipe_cells(lines[j]).is_some_and(|c| c.len() == cols || is_separator(&c)) {
            j += 1;
        }
        let run = &lines[i..j];
        let base = markers_before;
        markers_before += run.iter().map(|l| l.matches("___").count()).sum::<usize>();
        i = j;
        match fix_table(run, cols, base, total, answer_counts) {
            Some((rows, gone)) => {
                out.extend(rows);
                dropped.extend(gone);
            }
            None => out.extend(run.iter().map(|l| l.to_string())),
        }
    }
    TableFix { stem: out.join("\n"), dropped }
}

/// One table run → its clean lines and the dropped header markers, or None when the table is
/// not the label-repeating shape (left exactly as written).
fn fix_table(run: &[&str], cols: usize, base: usize, total: usize, answer_counts: &[usize]) -> Option<(Vec<String>, Vec<usize>)> {
    let parsed: Vec<Vec<String>> = run.iter().filter_map(|l| pipe_cells(l)).collect();
    let data_idx: Vec<usize> = (0..parsed.len()).filter(|&r| !is_separator(&parsed[r])).collect();
    if data_idx.len() < 2 || data_idx[0] != 0 {
        return None;
    }
    let sep_after_first = parsed.get(1).is_some_and(|r| is_separator(r));
    let grid: Vec<Vec<Cell>> = data_idx.iter().map(|&r| parsed[r].iter().map(|c| classify(c)).collect()).collect();
    // Label columns: every row after the first is a blank (bare or labelled), the labels agree.
    let mut labels: Vec<Option<String>> = vec![None; cols];
    let mut any_repeat = false;
    for (c, slot) in labels.iter_mut().enumerate() {
        let rest: Vec<&Cell> = grid[1..].iter().map(|r| &r[c]).collect();
        if !rest.iter().all(|x| matches!(x, Cell::Blank | Cell::Labeled(_))) {
            continue;
        }
        let mut seen: Option<String> = None;
        let mut agree = true;
        for x in &rest {
            if let Cell::Labeled(l) = x {
                match &seen {
                    Some(s) if !s.eq_ignore_ascii_case(l) => agree = false,
                    Some(_) => {}
                    None => seen = Some(l.clone()),
                }
            }
        }
        if !agree {
            continue;
        }
        if seen.is_some() {
            any_repeat = true;
        }
        let first = match &grid[0][c] {
            Cell::Labeled(l) => Some(l.clone()),
            _ => None,
        };
        *slot = seen.or(first);
    }
    let label_cols: Vec<usize> = (0..cols).filter(|&c| labels[c].is_some()).collect();
    if label_cols.is_empty() {
        return None;
    }
    let first_labeled = label_cols.iter().any(|&c| matches!(grid[0][c], Cell::Labeled(_)));
    if !any_repeat && !first_labeled {
        return None;
    }
    // Is the first row the header?
    let k = label_cols.iter().filter(|&&c| matches!(grid[0][c], Cell::Labeled(_) | Cell::Blank)).count();
    let header = if sep_after_first || label_cols.iter().any(|&c| matches!(grid[0][c], Cell::Other(_))) || !any_repeat {
        true
    } else {
        let as_header = answer_counts.iter().any(|&a| a > 0 && a == total - k);
        let as_data = answer_counts.iter().any(|&a| a > 0 && a == total);
        match (as_header, as_data) {
            (true, false) => true,
            (false, true) => false,
            _ => {
                let others: Vec<&Cell> = (0..cols).filter(|c| !label_cols.contains(c)).map(|c| &grid[0][c]).collect();
                !others.is_empty() && others.iter().all(|x| matches!(x, Cell::Other(t) if looks_like_title(t)))
            }
        }
    };
    let mut lines: Vec<String> = Vec::new();
    let mut gone: Vec<usize> = Vec::new();
    let mut seen_markers = base;
    for (r, raw) in parsed.iter().enumerate() {
        if is_separator(raw) {
            lines.push(run[r].to_string());
            continue;
        }
        let is_head = header && r == 0;
        let mut cells: Vec<String> = Vec::new();
        for (c, text) in raw.iter().enumerate() {
            let n = text.matches("___").count();
            if let Some(label) = labels[c].as_ref() {
                if is_head {
                    gone.extend(seen_markers..seen_markers + n);
                    cells.push(match &grid[0][c] {
                        Cell::Other(t) => t.clone(),
                        _ => label.clone(),
                    });
                } else {
                    cells.push("___".to_string());
                }
            } else {
                cells.push(text.clone());
            }
            seen_markers += n;
        }
        if r == 0 && !header {
            let head: Vec<String> = (0..cols).map(|c| labels[c].clone().unwrap_or_default()).collect();
            lines.push(pipe_row(&head));
        }
        lines.push(pipe_row(&cells));
    }
    Some((lines, gone))
}

fn part(p: &Value) -> Option<Part> {
    let stem = normalize_tables(text(p, "stem"), &[]).stem;
    if stem.is_empty() {
        return None;
    }
    let kind = match text(p, "type") {
        "multiple-choice" => PartKind::Mc { answers: choices(p).ok()? },
        "true-false" => PartKind::Tf { correct: p.get("answer").and_then(Value::as_bool).unwrap_or(true) },
        // A part can't hold blanks; keep the key as its sample answer.
        "fill-in-the-blank" => {
            let key: Vec<String> = p.get("blanks").and_then(Value::as_array).map(|a| a.iter().map(|b| match b {
                Value::String(s) => s.trim().to_string(),
                other => strings(Some(other)).join(" OR "),
            }).collect()).unwrap_or_default();
            PartKind::Short { sample: key.join("; ") }
        }
        _ => PartKind::Short { sample: text(p, "suggestedAnswer").to_string() },
    };
    Some(Part { stem, kind })
}

/// A source-box problem detected before asking a figure tool to redraw the picture. Overlap is
/// evidence of uncertain scope, not permission to choose new bounds or split the source's ink.
#[derive(Clone, Debug, serde::Serialize)]
pub struct FigureBoxIssue {
    pub kind: String,
    pub indices: Vec<usize>,
    pub message: String,
}

/// Validate source boxes without saving media or invoking a model. A nonblank crop can still be
/// wrong: distinct boxes that materially overlap may include a neighboring problem. Exactly shared
/// boxes are allowed, since several questions can legitimately refer to one supplied diagram.
pub fn figure_box_issues(reply: &str, page_png: Option<&[u8]>, page_no: usize) -> Vec<FigureBoxIssue> {
    let Some(v) = json_object(reply) else { return Vec::new() };
    let page = page_png.and_then(|b| image::load_from_memory(b).ok());
    figure_box_issues_in(&v, page.as_ref(), page_no)
}

fn figure_box_issues_in(v: &Value, page: Option<&image::DynamicImage>, page_no: usize) -> Vec<FigureBoxIssue> {
    let Some(raw) = v.pointer("/bank/questions").and_then(Value::as_array) else { return Vec::new() };
    let mut issues = Vec::new();
    let mut boxes = Vec::new();
    for (i, item) in raw.iter().enumerate() {
        let (label, _) = raw_label(item, i);
        let fig = match source_figure(item) {
            Ok(Some(selected)) => selected.figure,
            Ok(None) => continue,
            Err(reason) => {
                issues.push(FigureBoxIssue { kind: "figure_ownership".into(), indices: vec![i], message: format!("Question {label}: {reason}") });
                continue;
            }
        };
        match crop_piece(fig, page, page_no) {
            Ok(_) => if let Ok(bounds) = figure_bounds(fig, page_no) { boxes.push((i, label, bounds)); },
            Err(e) => issues.push(FigureBoxIssue { kind: "invalid_crop".into(), indices: vec![i], message: format!("Question {label}: {e}") }),
        }
    }
    for (at, (i, label, a)) in boxes.iter().enumerate() {
        for (j, other_label, b) in &boxes[at + 1..] {
            if a.iter().zip(b).all(|(x, y)| (x - y).abs() <= 0.000_001) { continue; }
            let width = (a[2].min(b[2]) - a[0].max(b[0])).max(0.0);
            let height = (a[3].min(b[3]) - a[1].max(b[1])).max(0.0);
            let smaller = ((a[2] - a[0]) * (a[3] - a[1])).min((b[2] - b[0]) * (b[3] - b[1]));
            let overlap = width * height / smaller;
            // Ignore touching edges and small rounding/padding slivers. This is deliberately a
            // review threshold, not a claim that boxes below it are source-faithful.
            if width >= 0.01 && height >= 0.01 && overlap >= 0.10 {
                issues.push(FigureBoxIssue { kind: "overlap".into(), indices: vec![*i, *j], message: format!(
                    "Question {label} figure box {a:?} overlaps question {other_label} figure box {b:?} by {:.1}% of the smaller box (normalized x0,y0,x1,y1). Distinct question boxes may include neighboring question text or diagrams", overlap * 100.0) });
            }
        }
    }
    issues
}

/// Read-only source thumbnails for the full-page visual preflight. Selection, ownership and
/// crop validation are identical to import; unsupported multi-figure items are left to issues.
pub fn source_figure_previews(reply: &str, page_png: &[u8], page_no: usize) -> Vec<(usize, String, Vec<u8>)> {
    let Some(v) = json_object(reply) else { return Vec::new() };
    let Ok(page) = image::load_from_memory(page_png) else { return Vec::new() };
    v.pointer("/bank/questions").and_then(Value::as_array).into_iter().flatten().enumerate().filter_map(|(i,item)| {
        let selected = source_figure(item).ok()??;
        let png = crop(selected.figure, Some(&page), page_no).ok()?;
        Some((i, raw_label(item, i).0, png))
    }).collect()
}

/// A crop retry may change only the suspect source box/page fields. All original stems, givens,
/// choices, keys, tasks, diagram descriptions and other questions survive byte-for-byte as JSON
/// values; a reordered or relabelled retry cannot silently attach a box to the wrong question.
pub fn corrected_figure_boxes(original: &str, correction: &str, affected: &[usize]) -> Option<String> {
    let mut before = json_object(original)?;
    let after = json_object(correction)?;
    let old = before.pointer_mut("/bank/questions")?.as_array_mut()?;
    let new = after.pointer("/bank/questions")?.as_array()?;
    if old.len() != new.len() { return None; }
    for &i in affected {
        let a = old.get(i)?;
        let b = new.get(i)?;
        let identified = ["id", "label"].iter().any(|key| a.get(*key).is_some_and(|v| v.is_number() || v.as_str().is_some_and(|s| !s.trim().is_empty())));
        let same_identity = |other: &&Value| a.get("id") == other.get("id") && a.get("label") == other.get("label");
        if !identified || a.get("id") != b.get("id") || a.get("label") != b.get("label")
            || old.iter().filter(same_identity).count() != 1 || new.iter().filter(same_identity).count() != 1 { return None; }
        let old_figure = source_figure(a).ok()??;
        let new_figure = source_figure(b).ok()??;
        if old_figure.source.path != new_figure.source.path
            && !(old_figure.source.part_index.is_some() && new_figure.source.origin == "question") { return None; }
        new_figure.figure.get("box")?.as_array()?;
    }
    for &i in affected {
        // A retry may use the normalized parent slot for a uniquely promoted child. The merge
        // writes only bounds/page back to the ORIGINAL child path, retaining its owner and text.
        let owner = source_figure(&old[i]).ok()??.source.path;
        let corrected = source_figure(&new[i]).ok()??.figure;
        let fig = old[i].pointer_mut(&owner)?.as_object_mut()?;
        fig.insert("box".into(), corrected["box"].clone());
        match corrected.get("page") {
            Some(page) => { fig.insert("page".into(), page.clone()); }
            None => { fig.remove("page"); }
        }
    }
    serde_json::to_string(&before).ok()
}

/// Crops `figure.box` (fractions of the page, top-left origin) out of the page raster.
/// A figure box as `[x0, y0, x1, y1]` fractions of the page, whichever of the two conventions it
/// was written in. The conversion prompt asks for the vision model's native one,
/// `[ymin, xmin, ymax, xmax]` on a 0..1000 grid: asked for x-first fractions instead, the model
/// put pictures near the top of a page in the wrong place (a blank region) about half the time.
/// Older files and hand-written ones use the fractions. Telling them apart is safe because a grid
/// box always has a value above 1, and a fraction box never does.
fn box_fractions(b: &[f64]) -> [f64; 4] {
    if b.iter().any(|v| *v > 1.0) {
        [b[1] / 1000.0, b[0] / 1000.0, b[3] / 1000.0, b[2] / 1000.0]
    } else {
        [b[0], b[1], b[2], b[3]]
    }
}

fn figure_bounds(fig: &Value, page_no: usize) -> Result<[f64; 4], String> {
    if let Some(p) = fig.get("page").and_then(Value::as_u64)
        && p as usize != page_no
    {
        return Err(format!("it points at page {p}, this is page {page_no}"));
    }
    let b: Vec<f64> = fig.get("box").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_f64).collect()).unwrap_or_default();
    if b.len() != 4 || b.iter().any(|n| !n.is_finite()) {
        return Err("its box needs four numbers".into());
    }
    Ok(box_fractions(&b).map(|n| n.clamp(0.0, 1.0)))
}

fn crop(fig: &Value, page: Option<&image::DynamicImage>, page_no: usize) -> Result<Vec<u8>, String> {
    let piece = crop_piece(fig, page, page_no)?;
    let mut out = Vec::new();
    piece.write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png).map_err(|e| e.to_string())?;
    Ok(out)
}

fn crop_piece(fig: &Value, page: Option<&image::DynamicImage>, page_no: usize) -> Result<image::DynamicImage, String> {
    let page = page.ok_or("no page image to crop from")?;
    let (w, h) = (page.width() as f64, page.height() as f64);
    let [bx0, by0, bx1, by1] = figure_bounds(fig, page_no)?;
    // The model's boxes run tight (on a real worksheet one clipped the edge atom of a structure), so
    // the crop may grow by up to 2.5% of the page on each side. It follows connected ink, including
    // a one-pixel axis, rather than treating a thin stroke as paper. A separate text line above (the question's own
    // last line) is a different object, and including it made the figure agent copy it as a title.
    let pad = 0.025;
    let (l, t) = (((bx0.clamp(0.0, 1.0)) * w) as i64, ((by0.clamp(0.0, 1.0)) * h) as i64);
    let (r, bt) = (((bx1.clamp(0.0, 1.0)) * w) as i64, ((by1.clamp(0.0, 1.0)) * h) as i64);
    let (px, py, pw, ph) = if r - l < 2 || bt - t < 2 {
        (0, 0, 0, 0)
    } else {
        let gray = page.to_luma8();
        let [x0, y0, x1, y1] = connected_crop_bounds(&gray,
            [l as u32, t as u32, r as u32, bt as u32],
            [(pad * w) as u32, (pad * h) as u32]);
        (x0, y0, x1 - x0, y1 - y0)
    };
    if pw < 24 || ph < 24 {
        return Err("its box is empty or off the page".into());
    }
    if pw as f64 > w * 0.98 && ph as f64 > h * 0.9 {
        return Err("its box covers the whole page".into());
    }
    let piece = page.crop_imm(px, py, pw, ph);
    // A box on bare paper is a wrong box, not a picture: on a real worksheet the model gave the
    // right box on one run and a box over the empty bottom of the page on the next three.
    if is_blank(&piece) {
        return Err("its box is over a blank part of the page".into());
    }
    Ok(piece)
}

/// Extend only ink that actually touches the box perimeter. Eight-neighbour connectivity
/// keeps diagonal and one-pixel strokes intact without jumping a paper gap to nearby text.
/// The original box is never shrunk; four clean paper pixels provide the final margin.
fn connected_crop_bounds(gray: &image::GrayImage, bounds: [u32; 4], pad: [u32; 2]) -> [u32; 4] {
    let [l, t, r, b] = bounds;
    if l >= r || t >= b || r > gray.width() || b > gray.height() { return bounds; }
    let ml = l.saturating_sub(pad[0]);
    let mt = t.saturating_sub(pad[1]);
    let mr = r.saturating_add(pad[0]).min(gray.width());
    let mb = b.saturating_add(pad[1]).min(gray.height());
    let stride = (mr - ml) as usize;
    let mut seen = vec![false; stride * (mb - mt) as usize];
    let mut stack = Vec::new();
    let enqueue = |x: u32, y: u32, seen: &mut Vec<bool>, stack: &mut Vec<(u32, u32)>| {
        if x < ml || x >= mr || y < mt || y >= mb { return; }
        let i = (y - mt) as usize * stride + (x - ml) as usize;
        if !seen[i] {
            seen[i] = true;
            if gray.get_pixel(x, y).0[0] < 200 { stack.push((x, y)); }
        }
    };
    for x in l..r { enqueue(x, t, &mut seen, &mut stack); enqueue(x, b - 1, &mut seen, &mut stack); }
    for y in t..b { enqueue(l, y, &mut seen, &mut stack); enqueue(r - 1, y, &mut seen, &mut stack); }
    let [mut x0, mut y0, mut x1, mut y1] = bounds;
    while let Some((x, y)) = stack.pop() {
        x0 = x0.min(x); y0 = y0.min(y); x1 = x1.max(x + 1); y1 = y1.max(y + 1);
        for dy in -1..=1i64 { for dx in -1..=1i64 {
            let (nx, ny) = (i64::from(x) + dx, i64::from(y) + dy);
            if nx >= i64::from(ml) && nx < i64::from(mr) && ny >= i64::from(mt) && ny < i64::from(mb)
                && (nx < i64::from(l) || nx >= i64::from(r) || ny < i64::from(t) || ny >= i64::from(b)) {
                enqueue(nx as u32, ny as u32, &mut seen, &mut stack);
            }
        } }
    }
    for _ in 0..4 { if x0 > ml && (y0..y1).all(|y| gray.get_pixel(x0 - 1, y).0[0] >= 200) { x0 -= 1; } else { break; } }
    for _ in 0..4 { if x1 < mr && (y0..y1).all(|y| gray.get_pixel(x1, y).0[0] >= 200) { x1 += 1; } else { break; } }
    for _ in 0..4 { if y0 > mt && (x0..x1).all(|x| gray.get_pixel(x, y0 - 1).0[0] >= 200) { y0 -= 1; } else { break; } }
    for _ in 0..4 { if y1 < mb && (x0..x1).all(|x| gray.get_pixel(x, y1).0[0] >= 200) { y1 += 1; } else { break; } }
    [x0, y0, x1, y1]
}

/// True when almost nothing in the picture is ink: under 0.2% of its pixels are darker than paper.
/// A thin line drawing still has more than that; a scanner's grey speckle does not reach it.
fn is_blank(img: &image::DynamicImage) -> bool {
    let g = img.to_luma8();
    let ink = g.pixels().filter(|p| p.0[0] < 200).count();
    (ink as f64) < 0.002 * (g.width() as f64 * g.height() as f64).max(1.0)
}

fn exams(v: &Value, ids: &HashMap<String, String>) -> Vec<Exam> {
    let mut out = Vec::new();
    for e in v.get("exams").and_then(Value::as_array).into_iter().flatten() {
        let name = text(e, "name");
        let items: Vec<ExamItem> = e
            .get("positions")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|p| p.get("question").and_then(Value::as_str).and_then(|q| ids.get(q)))
            .map(|q| ExamItem::new(String::new(), q.clone()))
            .collect();
        if name.is_empty() || items.is_empty() {
            continue;
        }
        out.push(Exam { paper_style: Default::default(), margins: None,
            id: crate::model::new_id(),
            name: name.to_string(),
            header: crate::formats::DEFAULT_HEADER.to_string(),
            title: None,
            items,
            heading_pt: 16.0,
            body_pt: 11.0,
            margin_mm: 18.0,
            ruled_blanks: 0,
            modified: 0,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn directions_package() -> Value {
        serde_json::json!({"format":FORMAT,"bank":{"name":"Worksheet","questions":[
            {"id":"q1","type":"short-answer","stem":"$5/4-3/4$","suggestedAnswer":""},
            {"id":"q2","type":"short-answer","stem":"$x^2+2x-3=0$","suggestedAnswer":""}
        ]},"exams":[{"name":"Worksheet","sections":[
            {"title":"A","instructions":"Evaluate each expression."},
            {"title":"B","instructions":"Solve with the quadratic formula."}
        ],"positions":[{"question":"q1","section":0},{"question":"q2","section":1}]}]})
    }

    #[test]
    fn bank_items_keep_their_own_source_section_instructions() {
        let got = import_text(&directions_package().to_string(),None,1,"worksheet").unwrap();
        assert_eq!(got.doc.questions[0].stem,"A\n\nEvaluate each expression.\n\n$5/4-3/4$");
        assert_eq!(got.doc.questions[1].stem,"B\n\nSolve with the quadratic formula.\n\n$x^2+2x-3=0$");
        assert!(got.warnings.is_empty());
    }

    #[test]
    fn already_inline_directions_are_not_duplicated() {
        let mut package = directions_package();
        package["bank"]["questions"][0]["stem"] = Value::String("Evaluate  each expression.\n$5/4-3/4$".into());
        package["exams"][0]["sections"][0]["instructions"] = Value::String("**Evaluate each expression.**".into());
        let got = import_text(&package.to_string(),None,1,"worksheet").unwrap();
        assert_eq!(got.doc.questions[0].stem,"A\n\nEvaluate  each expression.\n$5/4-3/4$");
    }

    /// Holdout v1, MIT 18.06 PS2: "1) (1a) ... (1b) ..." has no shared stem, and the reader
    /// returned each question's own sentence, in plain text, as its page direction.
    fn inline_parts_package() -> Value {
        serde_json::json!({"format":FORMAT,"formatVersion":"0.1.0","bank":{"name":"PS2","questions":[
            {"id":"q1","label":"1","type":"multipart","stem":"","parts":[
                {"type":"short-answer","stem":"(1a) Find a nonzero matrix $A$ for which $A^2 = 0$.","suggestedAnswer":""},
                {"type":"short-answer","stem":"(1b) Find a 2-by-2 matrix (of real numbers) such that $A^2 = -I$.","suggestedAnswer":""}]},
            {"id":"q2","label":"2","type":"short-answer","stem":"Find $A^{-1}$ with Gauss-Jordan for $A = \\begin{bmatrix}0&2\\\\2&2\\end{bmatrix}$. (Of course, you have to start with a row exchange.)","suggestedAnswer":""}]},
            "exams":[{"name":"PS2","sections":[
                {"title":"","instructions":"(1a) Find a nonzero matrix A for which A^2 = 0. (1b) Find a 2-by-2 matrix (of real numbers) such that A^2 = -I."},
                {"title":"","instructions":"Find A^{-1} with Gauss-Jordan for A = [[0, 2], [2, 2]]. (Of course, you have to start with a row exchange.)"}],
                "positions":[{"question":"q1","section":0},{"question":"q2","section":1}]}]})
    }

    #[test]
    fn inline_lettered_parts_are_kept_without_a_shared_stem_or_a_restated_copy() {
        let got = import_text(&inline_parts_package().to_string(),None,1,"worksheet").unwrap();
        assert_eq!(got.doc.questions.len(), 2, "{:?}", got.warnings);
        let q1 = &got.doc.questions[0];
        assert_eq!(q1.stem, "");
        assert!(matches!(&q1.kind, Kind::Multipart { parts } if parts.len() == 2 && parts[1].stem.starts_with("(1b)")));
        assert!(crate::qcheck::check(&q1.stem, &q1.kind).iter().all(|i| i.at != crate::qcheck::Where::Stem));
        // The plain-text restatement is the typeset stem again, not a direction.
        assert!(got.doc.questions[1].stem.starts_with("Find $A^{-1}$"), "{}", got.doc.questions[1].stem);
    }

    #[test]
    fn an_empty_stem_with_a_lost_part_is_still_dropped() {
        let mut package = inline_parts_package();
        package["bank"]["questions"][0]["parts"][1]["stem"] = Value::String(String::new());
        let got = import_text(&package.to_string(),None,1,"worksheet").unwrap();
        assert_eq!(got.doc.questions.len(), 1);
        assert!(got.doc.questions[0].stem.starts_with("Find $A^{-1}$"));
    }

    #[test]
    fn a_short_direction_or_new_words_are_still_prepended() {
        let mut package = inline_parts_package();
        package["exams"][0]["sections"][1]["instructions"] = Value::String("Find each inverse with Gauss-Jordan.".into());
        let got = import_text(&package.to_string(),None,1,"worksheet").unwrap();
        assert!(got.doc.questions[1].stem.starts_with("Find each inverse with Gauss-Jordan.\n\nFind $A^{-1}$"));
        assert!(prose_words("\\frac{a}{b} Find $A^{-1}$ x").eq(&["find".to_string()]));
    }

    #[test]
    fn invalid_section_mapping_warns_without_borrowing_another_task() {
        let mut package = directions_package();
        package["exams"][0]["positions"][0]["section"] = serde_json::json!(12);
        let got = import_text(&package.to_string(),None,1,"worksheet").unwrap();
        assert_eq!(got.doc.questions[0].stem,"$5/4-3/4$");
        assert!(got.warnings.iter().any(|w|w.contains("invalid source section")));
    }

    #[test]
    fn conflicting_exam_directions_are_flagged_instead_of_silently_combined() {
        let mut package = directions_package();
        let mut other = package["exams"][0].clone();
        other["sections"][0]["instructions"] = Value::String("Round each result to two decimals.".into());
        package["exams"].as_array_mut().unwrap().push(other);
        let got = import_text(&package.to_string(),None,1,"worksheet").unwrap();
        assert_eq!(got.doc.questions[0].stem,"A\n\n$5/4-3/4$");
        assert!(got.warnings.iter().any(|w|w.contains("conflicting source section")));
        assert_eq!(got.doc.questions[1].stem.matches("Solve with").count(),1);
    }

    #[test]
    fn unknown_question_positions_do_not_assign_instructions_by_array_order() {
        let mut package = directions_package();
        package["exams"][0]["positions"][0]["question"] = Value::String("unknown".into());
        let got = import_text(&package.to_string(),None,1,"worksheet").unwrap();
        assert_eq!(got.doc.questions[0].stem,"$5/4-3/4$");
        assert!(got.doc.questions[1].stem.starts_with("B\n\nSolve with"));
    }

    #[test]
    fn example_blanks_in_directions_do_not_add_unkeyed_answer_slots() {
        let mut package = directions_package();
        package["bank"]["questions"][0] = serde_json::json!({"id":"q1","type":"fill-in-the-blank","stem":"The value is ___.","blanks":[["2"]]});
        package["exams"][0]["sections"][0]["instructions"] = Value::String("Write the word in each ___ space.".into());
        let got = import_text(&package.to_string(),None,1,"worksheet").unwrap();
        assert_eq!(got.doc.questions[0].stem,"A\n\nThe value is ___.");
        assert!(matches!(&got.doc.questions[0].kind,Kind::Blank{accept} if accept.len()==1));
        assert!(got.warnings.iter().any(|w|w.contains("example blank")));
    }

    #[test]
    fn a_continuation_without_directions_does_not_invent_them_from_the_title() {
        let mut package = directions_package();
        package["bank"]["name"] = Value::String("Using the quadratic formula".into());
        package["exams"] = serde_json::json!([]);
        let got = import_text(&package.to_string(),None,1,"worksheet").unwrap();
        assert_eq!(got.doc.questions[0].stem,"$5/4-3/4$");
    }

    #[test]
    fn a_root_object_closed_one_brace_early_still_imports_its_questions() {
        // Seen from the hosted model: `]}]}` closes the root before `"report"`, so `"report"…}}` is extra data.
        let reply = r#"{"format":"queshion/ditto","formatVersion":"0.1.0","bank":{"name":"Q","questions":[{"id":"q1","type":"true-false","stem":"Water is wet.","answer":true}]},"exams":[{"name":"Q","sections":[{"title":"","instructions":""}],"positions":[{"question":"q1","section":0}]}]},"report":{"unconverted":[],"notes":[]}}"#;
        assert_eq!(json_error(reply), None);
        let got = import_text(reply, None, 1, "x").expect("the questions are all there");
        assert_eq!(got.doc.questions.len(), 1);
        assert!(json_object("no braces").is_none());
        assert!(json_error(r#"{"a": [1, 2"#).is_some(), "a genuinely broken reply still asks for a retry");
    }

    #[test]
    fn directions_after_a_root_closed_before_exams_are_not_lost() {
        // Seen from the hosted model on a figure-only worksheet: `]}}` closed the root before "exams", so the
        // printed task directions sat outside the object and every figure-only item looked task-less.
        store();
        let reply = r#"{"format":"queshion/ditto","formatVersion":"0.1.0","bank":{"name":"Polar","questions":[{"id":"q1","label":"1","type":"short-answer","stem":"","figure":{"box":[130, 100, 400, 400],"alt":"g"}}]}},"exams":[{"name":"Polar","sections":[{"title":"","instructions":"Classify the curve."}],"positions":[{"question":"q1","section":0}]}],"report":{"unconverted":[],"notes":[]}}"#;
        assert_eq!(json_error(reply), None);
        let got = import_text(reply, Some(&worksheet_page(false)), 1, "x").unwrap();
        assert_eq!(got.doc.questions[0].stem, "Classify the curve.");
        assert_eq!(got.items[0].status, "saved_figure_only");
    }

    #[test]
    fn directions_inside_a_bank_the_model_forgot_to_close_still_apply() {
        store();
        let reply = r#"{"format":"queshion/ditto","formatVersion":"0.1.0","bank":{"name":"Polar","questions":[{"id":"q1","label":"1","type":"short-answer","stem":"","figure":{"box":[130, 100, 400, 400],"alt":"g"}}],"exams":[{"name":"Polar","sections":[{"title":"","instructions":"Classify the curve."}],"positions":[{"question":"q1","section":0}]}],"report":{"unconverted":[],"notes":[]}}"#;
        let got = import_text(reply, Some(&worksheet_page(false)), 1, "x").unwrap();
        assert_eq!(got.doc.questions[0].stem, "Classify the curve.");
        assert_eq!(got.doc.exams.len(), 1);
    }

    #[test]
    fn a_reply_that_falls_into_a_loop_after_its_questions_still_imports_them() {
        // Seen from DeepSeek V4.1 Flash: after the bank the model repeated `"report:{" ,"unconverted:"…` until the token limit.
        let looped = r#""report:{" ,"unconverted:","[]," ,"notes:","[]}},","#.repeat(40);
        let reply = format!(r#"{{"format":"queshion/ditto","formatVersion":"0.1.0","bank":{{"name":"Q","questions":[{{"id":"q1","type":"true-false","stem":"Water is wet.","answer":true}}]}},"exams":[{{"name":"Q","positions":[{{"question":"q1","section":0}}]}},{looped}"#);
        assert_eq!(json_error(&reply), None);
        let got = import_text(&reply, None, 1, "x").expect("the bank closed before the loop began");
        assert_eq!(got.doc.questions.len(), 1);
        assert!(json_error(r#"{"bank":{"questions":[{"id":"q1""#).is_some(), "a cut inside the first question is still a retry");
        // The same loop when the model also forgot to close "bank" before "exams" (the exam then sits inside the bank).
        let open_bank = format!(r#"{{"format":"queshion/ditto","formatVersion":"0.1.0","bank":{{"name":"Q","questions":[{{"id":"q1","type":"true-false","stem":"Water is wet.","answer":true}}],"exams":[{{"name":"Q","positions":[{{"question":"q1","section":0}}]}},{looped}"#);
        assert_eq!(json_error(&open_bank), None);
        assert_eq!(import_text(&open_bank, None, 1, "x").expect("the questions closed before the loop").doc.questions.len(), 1);
    }

    fn png_of(img: image::RgbaImage) -> Vec<u8> {
        let mut out = Vec::new();
        image::DynamicImage::ImageRgba8(img).write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png).unwrap();
        out
    }

    fn ink(img: &mut image::RgbaImage, xs: std::ops::Range<u32>, ys: std::ops::Range<u32>) {
        for y in ys {
            for x in xs.clone() {
                img.put_pixel(x, y, image::Rgba([0, 0, 0, 255]));
            }
        }
    }

    /// A 1000 x 1000 page: the question's last text line (rows 100..112) sits 18 blank rows above a
    /// hollow square figure (rows 130..400) like a graph under its inequalities.
    fn worksheet_page(clipped_arrow: bool) -> Vec<u8> {
        let mut img = image::RgbaImage::from_pixel(1000, 1000, image::Rgba([255, 255, 255, 255]));
        ink(&mut img, 100..400, 100..112);
        ink(&mut img, 100..400, 130..133);
        ink(&mut img, 100..400, 397..400);
        ink(&mut img, 100..103, 130..400);
        ink(&mut img, 397..400, 130..400);
        if clipped_arrow {
            ink(&mut img, 248..252, 120..130);
        }
        png_of(img)
    }

    fn crop_dims(page: &[u8], box_json: &str) -> (u32, u32) {
        let page = image::load_from_memory(page).unwrap();
        let fig: Value = serde_json::from_str(&format!(r#"{{"box":{box_json}}}"#)).unwrap();
        let img = image::load_from_memory(&crop(&fig, Some(&page), 1).unwrap()).unwrap();
        (img.width(), img.height())
    }

    #[test]
    fn a_crop_never_grows_into_the_question_text_above_its_figure() {
        // Box [ymin,xmin,ymax,xmax] on the 0..1000 grid = the square exactly. The old 2.5%-of-page pad
        // (25 px) reached row 105 and pulled the last text line into the picture.
        let (w, h) = crop_dims(&worksheet_page(false), "[130, 100, 400, 400]");
        assert_eq!((w, h), (300 + 8, 270 + 8), "only a clean few-pixel margin is added, no text rows");
    }

    #[test]
    fn a_crop_still_includes_ink_that_continues_from_the_box_edge() {
        // An arrowhead that pokes out of a tight box is part of the figure and joins it without a gap.
        let (_, h) = crop_dims(&worksheet_page(true), "[130, 100, 400, 400]");
        assert_eq!(h, 270 + 10 + 4 + 4, "the 10 px arrow is kept; the separate text line 18 rows up is not");
    }

    #[test]
    fn one_pixel_axis_bridges_reach_their_arrowheads_on_every_side() {
        let mut image = image::GrayImage::from_pixel(400, 400, image::Luma([255]));
        for k in 150..=220 { image.put_pixel(k, 150, image::Luma([0])); }
        for k in 210..=220 {
            let d = (220 - k) / 2;
            image.put_pixel(k, 150 - d, image::Luma([0]));
            image.put_pixel(k, 150 + d, image::Luma([0]));
        }
        let rotations = [image.clone(), image::imageops::rotate90(&image), image::imageops::rotate180(&image), image::imageops::rotate270(&image)];
        let boxes = [[100,100,200,200], [200,100,300,200], [200,200,300,300], [100,200,200,300]];
        for (image, bounds) in rotations.iter().zip(boxes) {
            let crop = connected_crop_bounds(image, bounds, [40,40]);
            for (x,y,p) in image.enumerate_pixels().filter(|(_,_,p)|p.0[0] < 200) {
                assert!(crop[0] <= x && x < crop[2] && crop[1] <= y && y < crop[3], "thin connected ink at {x},{y} was clipped by {crop:?}: {p:?}");
            }
        }
    }

    #[test]
    fn connected_crop_does_not_jump_to_nearby_text_or_single_pixel_noise() {
        let mut image = image::GrayImage::from_pixel(400, 400, image::Luma([255]));
        for x in 150..=220 { image.put_pixel(x, 150, image::Luma([0])); }
        // A separate speck is not an axis continuation, even on the same row.
        image.put_pixel(223, 150, image::Luma([0]));
        for x in 100..230 { for y in 88..95 { image.put_pixel(x, y, image::Luma([0])); } }
        assert_eq!(connected_crop_bounds(&image, [100,100,200,200], [40,40]), [96,96,223,204]);
        // Following a diagonal must not convert an actual paper gap into a join.
        image.put_pixel(222, 149, image::Luma([0]));
        let crop = connected_crop_bounds(&image, [100,100,200,200], [40,40]);
        assert!(crop[2] <= 222 && crop[1] > 94);
    }

    #[test]
    fn connected_crop_keeps_diagonals_and_obeys_page_and_padding_limits() {
        let mut image = image::GrayImage::from_pixel(100, 100, image::Luma([255]));
        for k in 0..=45 { image.put_pixel(k, k, image::Luma([0])); }
        assert_eq!(connected_crop_bounds(&image, [20,20,50,50], [7,7]), [13,13,54,54]);
        assert_eq!(connected_crop_bounds(&image, [0,0,50,50], [7,7]), [0,0,54,54]);
        assert_eq!(connected_crop_bounds(&image, [20,20,20,50], [7,7]), [20,20,20,50]);
    }

    #[test]
    #[ignore = "explicit hash-pinned source fixture; no model calls or bank writes"]
    fn pinned_thin_axis_crop_replay_keeps_the_complete_original_arrow() {
        use sha2::{Digest, Sha256};
        let fixture: Value = serde_json::from_slice(&std::fs::read(std::env::var("VERSHION_CROP_REPLAY_FIXTURE").expect("explicit fixture path")).unwrap()).unwrap();
        let bytes = std::fs::read(fixture["source"].as_str().unwrap()).unwrap();
        assert_eq!(format!("{:x}", Sha256::digest(&bytes)), fixture["source_sha256"].as_str().unwrap());
        let page = image::load_from_memory(&bytes).unwrap();
        let fig = serde_json::json!({"box":fixture["box_1000"]});
        let piece = crop_piece(&fig, Some(&page), 1).unwrap();
        let xy: Vec<u32> = fixture["expected_pixel_rect"].as_array().unwrap().iter().map(|n|n.as_u64().unwrap() as u32).collect();
        assert_eq!(piece.to_rgba8(), page.crop_imm(xy[0],xy[1],xy[2]-xy[0],xy[3]-xy[1]).to_rgba8());
        let tip = fixture["arrow_tip"].as_array().unwrap();
        assert!(piece.to_luma8().get_pixel(tip[0].as_u64().unwrap() as u32-xy[0],tip[1].as_u64().unwrap() as u32-xy[1]).0[0] < 200);
        piece.save(fixture["out"].as_str().unwrap()).unwrap();
    }

    fn scope_page() -> Vec<u8> {
        let mut page = image::RgbImage::from_pixel(1000, 1000, image::Rgb([255, 255, 255]));
        for x in 0..1000 {
            for y in 0..1000 {
                if x % 25 == 0 || y % 25 == 0 { page.put_pixel(x, y, image::Rgb([0, 0, 0])); }
            }
        }
        let mut png = Vec::new();
        image::DynamicImage::ImageRgb8(page).write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).unwrap();
        png
    }

    fn scope_reply(boxes: &[[f64; 4]]) -> Value {
        let questions: Vec<_> = boxes.iter().enumerate().map(|(i, bounds)| serde_json::json!({
            "id":format!("q{}", i + 1), "label":format!("{}", i + 1), "type":"short-answer",
            "stem":format!("Original given {}", i + 1), "suggestedAnswer":"", "figure":{"box":bounds,"alt":"Original diagram description"}
        })).collect();
        serde_json::json!({"format":FORMAT,"bank":{"name":"Scope","questions":questions},"exams":[{"name":"Scope","sections":[{"instructions":"Original task."}],"positions":[]}]})
    }

    #[test]
    fn nonblank_neighboring_problem_boxes_require_scope_review() {
        // Saved raw boxes from the parametric worksheet: q1/q2 accidentally extend into the next
        // row, overlapping 19% of q3/q4 and including their text and partial grids. Every crop has ink.
        let raw = scope_reply(&[[262.,50.,600.,330.],[262.,495.,600.,770.],[560.,50.,770.,330.],[560.,495.,770.,770.]]);
        let issues = figure_box_issues(&raw.to_string(), Some(&scope_page()), 1);
        assert_eq!(issues.len(), 2);
        assert!(issues.iter().all(|i| i.kind == "overlap" && i.message.contains("19.0%")));
        assert_eq!(issues[0].indices, [0, 2]);
        assert_eq!(issues[1].indices, [1, 3]);
    }

    #[test]
    fn shared_figures_and_multi_panel_figures_are_not_split_or_rejected() {
        let mut raw = scope_reply(&[[100.,100.,400.,400.],[100.,100.,400.,400.]]);
        // The same figure in the legacy fraction convention is still the identical source box.
        raw["bank"]["questions"][1]["figure"]["box"] = serde_json::json!([0.1,0.1,0.4,0.4]);
        assert!(figure_box_issues(&raw.to_string(), Some(&scope_page()), 1).is_empty());
        let multi = scope_reply(&[[100.,50.,600.,950.]]);
        assert!(figure_box_issues(&multi.to_string(), Some(&scope_page()), 1).is_empty(), "one wide supplied multi-panel diagram is not split by ink heuristics");
    }

    #[test]
    fn box_scope_validation_ignores_touching_edges_and_rounding_slivers() {
        let raw = scope_reply(&[[100.,100.,400.,400.],[398.,100.,698.,400.],[700.,100.,950.,400.]]);
        assert!(figure_box_issues(&raw.to_string(), Some(&scope_page()), 1).is_empty());
    }

    #[test]
    fn a_box_retry_only_changes_the_targeted_bounds_and_page() {
        let before = scope_reply(&[[262.,50.,600.,330.],[262.,495.,600.,770.],[560.,50.,770.,330.],[560.,495.,770.,770.]]);
        let mut after = before.clone();
        after["bank"]["questions"][0]["figure"]["box"] = serde_json::json!([262.,50.,480.,330.]);
        after["bank"]["questions"][1]["figure"]["box"] = serde_json::json!([262.,495.,480.,770.]);
        after["bank"]["questions"][0]["stem"] = Value::String("Changed given and invented solution".into());
        after["bank"]["questions"][0]["suggestedAnswer"] = Value::String("Invented answer".into());
        after["bank"]["questions"][0]["figure"]["alt"] = Value::String("Changed figure labels".into());
        after["bank"]["questions"][3]["figure"]["box"] = serde_json::json!([0.,0.,1.,1.]);
        after["exams"][0]["sections"][0]["instructions"] = Value::String("Different task".into());
        let corrected = corrected_figure_boxes(&before.to_string(), &after.to_string(), &[0,1]).unwrap();
        let corrected = json_object(&corrected).unwrap();
        let mut expected = before.clone();
        for i in [0,1] { expected["bank"]["questions"][i]["figure"]["box"] = after["bank"]["questions"][i]["figure"]["box"].clone(); }
        assert_eq!(corrected, expected, "all original question content and unaffected boxes survive the correction");
        assert!(figure_box_issues(&corrected.to_string(), Some(&scope_page()), 1).is_empty());
        after["bank"]["questions"].as_array_mut().unwrap().swap(0,1);
        assert!(corrected_figure_boxes(&before.to_string(), &after.to_string(), &[0,1]).is_none(), "a reordered retry cannot shift box provenance");
        let mut duplicated = before.clone();
        duplicated["bank"]["questions"][1]["id"] = serde_json::json!("q1");
        duplicated["bank"]["questions"][1]["label"] = serde_json::json!("1");
        assert!(corrected_figure_boxes(&duplicated.to_string(), &duplicated.to_string(), &[0]).is_none(), "duplicate source identities cannot safely anchor a box repair");
    }

    #[test]
    fn unresolved_overlapping_crops_keep_questions_in_explicit_review_state() {
        store();
        let raw = scope_reply(&[[262.,50.,600.,330.],[262.,495.,600.,770.],[560.,50.,770.,330.],[560.,495.,770.,770.]]);
        let got = import_text(&raw.to_string(), Some(&scope_page()), 1, "scope").unwrap();
        assert_eq!((got.doc.questions.len(), got.figures), (4,4));
        for (i, item) in got.items.iter().enumerate() {
            assert_eq!(item.status, "saved_figure_box_unresolved");
            assert_eq!(item.figure, "attached_needs_review");
            assert_eq!(item.question_index, Some(i));
            assert!(item.reason.as_deref().is_some_and(|r| r.contains("overlaps question")));
            assert_eq!(got.doc.questions[i].stem, format!("Original given {}", i + 1));
            assert!(matches!(&got.doc.questions[i].kind, Kind::Short { sample } if sample.is_empty()));
        }
    }

    /// Local, read-only-source replay hook: the manifest supplies saved raw replies and original
    /// question-page paths; results go only to its explicit output path. Never calls an AI service.
    #[test]
    #[ignore = "requires VERSHION_CROP_REPLAY_MANIFEST with saved source fixtures"]
    fn replay_source_box_validation_from_manifest() {
        use sha2::{Digest, Sha256};
        let manifest_path = crate::env_var("VERSHION_CROP_REPLAY_MANIFEST").expect("fixture manifest");
        let manifest: Value = serde_json::from_slice(&std::fs::read(manifest_path).unwrap()).unwrap();
        let mut results = Vec::new();
        for case in manifest["cases"].as_array().unwrap() {
            let raw = case["raw_reply"].as_str().unwrap();
            let page = std::fs::read(case["source_png"].as_str().unwrap()).unwrap();
            let hash = format!("{:x}", Sha256::digest(&page));
            assert_eq!(hash, case["source_sha256"].as_str().unwrap(), "source changed for {}", case["case_id"]);
            let issues = figure_box_issues(raw, Some(&page), case["page_no"].as_u64().unwrap() as usize);
            let pairs: Vec<_> = issues.iter().filter(|i| i.kind == "overlap").map(|i| i.indices.clone()).collect();
            if let Some(expected) = case.get("expected_overlap_pairs") { assert_eq!(serde_json::to_value(&pairs).unwrap(), *expected, "{}", case["case_id"]); }
            results.push(serde_json::json!({"case_id":case["case_id"],"source_sha256":hash,"raw_reply_sha256":format!("{:x}",Sha256::digest(raw.as_bytes())),"issues":issues,"overlap_pairs":pairs}));
        }
        std::fs::write(manifest["output"].as_str().unwrap(), serde_json::to_vec_pretty(&results).unwrap()).unwrap();
    }

    fn figure_only_reply(instructions: &str, extra_figure: bool) -> String {
        let mut questions = vec![
            serde_json::json!({"id":"q1","label":"13","type":"short-answer","stem":"","suggestedAnswer":"","figure":{"box":[130, 100, 400, 400],"alt":"square"}}),
            serde_json::json!({"id":"q2","label":14,"type":"short-answer","stem":"","suggestedAnswer":""}),
        ];
        if extra_figure {
            questions.push(serde_json::json!({"id":"q3","label":"15","type":"short-answer","stem":"","suggestedAnswer":"","figure":{"box":[800, 800, 900, 900],"alt":"bare paper"}}));
        }
        let positions: Vec<Value> = (1..=questions.len()).map(|n| serde_json::json!({"question":format!("q{n}"),"section":0})).collect();
        serde_json::json!({"format":FORMAT,"bank":{"name":"Angles","questions":questions},
            "exams":[{"name":"Angles","sections":[{"title":"","instructions":instructions}],"positions":positions}]}).to_string()
    }

    #[test]
    fn an_empty_multipart_wrapper_does_not_drop_a_valid_figure_only_problem() {
        store();
        let mut reply = json_object(&figure_only_reply("Find the measure of b.", false)).unwrap();
        reply["bank"]["questions"][0]["type"] = serde_json::json!("multipart");
        reply["bank"]["questions"][0]["parts"] = serde_json::json!([]);
        let got = import_text(&reply.to_string(), Some(&worksheet_page(false)), 2, "w").unwrap();
        assert_eq!(got.doc.questions.len(), 1);
        assert_eq!(got.doc.questions[0].stem, "Find the measure of b.");
        assert!(matches!(&got.doc.questions[0].kind, Kind::Short { sample } if sample.is_empty()));
        assert!(got.doc.questions[0].image.is_some());
        assert_eq!(got.items[0].status, "saved_figure_only");
        assert!(got.warnings.iter().any(|w| w.contains("empty multipart wrapper repaired")));
        reply["bank"]["questions"][0]["figure"] = Value::Null;
        assert!(import_text(&reply.to_string(), Some(&worksheet_page(false)), 2, "w").unwrap().doc.questions.is_empty());
    }

    #[test]
    fn populated_parts_and_other_given_fields_are_never_discarded_to_repair_a_type() {
        store();
        let mut reply = json_object(&figure_only_reply("Use the provided diagram.", false)).unwrap();
        reply["bank"]["questions"][0]["type"] = serde_json::json!("multipart");
        reply["bank"]["questions"][0]["parts"] = serde_json::json!([{"stem":"Explain your reasoning.","type":"short-answer"}]);
        let got = import_text(&reply.to_string(), Some(&worksheet_page(false)), 2, "w").unwrap();
        assert!(matches!(&got.doc.questions[0].kind, Kind::Multipart { parts } if parts.len()==1 && parts[0].stem=="Explain your reasoning."));
        reply["bank"]["questions"][0]["parts"] = serde_json::json!([]);
        reply["bank"]["questions"][0]["choices"] = serde_json::json!([{"text":"A provided choice"}]);
        assert!(import_text(&reply.to_string(), Some(&worksheet_page(false)), 2, "w").unwrap().doc.questions.is_empty());
    }

    #[test]
    fn literal_mapped_headings_survive_without_becoming_guessed_task_directions() {
        let mut reply = json_object(&continuation_reply("", None)).unwrap();
        reply["exams"][0]["sections"][0]["title"] = serde_json::json!("Extension questions:");
        let got = import_text(&reply.to_string(), None, 1, "w").unwrap();
        assert!(got.doc.questions[0].stem.starts_with("Extension questions:\n\n"));
        assert_eq!(got.items[0].headings, ["Extension questions:"]);
        assert!(got.items[0].directions.is_none());
        // A title cannot resolve a figure whose actual task is absent.
        store();
        let mut figure = json_object(&figure_only_reply("", false)).unwrap();
        figure["exams"][0]["sections"][0]["title"] = serde_json::json!("Extension questions:");
        let got = import_text(&figure.to_string(), Some(&worksheet_page(false)), 2, "w").unwrap();
        assert_eq!(got.items[0].status, "saved_figure_only_unresolved");
        assert!(got.doc.questions[0].stem.contains(UNRESOLVED_TASK));
    }

    #[test]
    fn a_mapped_heading_is_not_duplicated_when_already_in_the_item() {
        let mut reply = json_object(&continuation_reply("Evaluate each expression.", None)).unwrap();
        reply["exams"][0]["sections"][0]["title"] = serde_json::json!("Practice");
        reply["bank"]["questions"][0]["stem"] = serde_json::json!("**Practice**\n$(9)/(5) - (5)/(8)$");
        let got = import_text(&reply.to_string(), None, 1, "w").unwrap();
        assert_eq!(got.doc.questions[0].stem.matches("Practice").count(), 1);
        assert!(got.doc.questions[0].stem.contains("Evaluate each expression."));
    }

    #[test]
    fn a_figure_only_question_keeps_its_figure_and_takes_the_printed_task() {
        store();
        let got = import_text(&figure_only_reply("Find the measure of b.", false), Some(&worksheet_page(false)), 2, "w").unwrap();
        assert_eq!(got.doc.questions.len(), 1, "the figure item is kept, the empty item with no figure is not");
        let q = &got.doc.questions[0];
        assert_eq!(q.stem, "Find the measure of b.");
        assert!(q.image.is_some() && q.topics.is_empty());
        assert!(matches!(&q.kind, Kind::Short { sample } if sample.is_empty()), "no invented answer");
        let first = &got.items[0];
        assert_eq!((first.status.as_str(), first.number, first.question_index, first.figure.as_str()), ("saved_figure_only", Some(13), Some(0), "attached"));
        assert_eq!(first.directions.as_ref().map(|d| (d.origin.as_str(), d.from_page)), Some(("printed_on_page", Some(2))));
    }

    #[test]
    fn an_empty_item_without_a_valid_figure_is_never_made_into_a_question() {
        store();
        let got = import_text(&figure_only_reply("Find the measure of b.", true), Some(&worksheet_page(false)), 2, "w").unwrap();
        assert_eq!(got.doc.questions.len(), 1);
        // The numeric label 14 and the bare-paper box 15 are both traced as dropped, in source order.
        let seen: Vec<_> = got.items.iter().map(|i| (i.number, i.status.as_str(), i.question_index)).collect();
        assert_eq!(seen, [(Some(13), "saved_figure_only", Some(0)), (Some(14), "dropped", None), (Some(15), "dropped", None)]);
        assert!(got.items[2].reason.as_deref().is_some_and(|r| r.contains("no stem")));
        assert_eq!(got.items[2].figure, "dropped");
        assert!(got.warnings.iter().any(|w| w.contains("Question 15") && w.contains("blank part of the page")));
    }

    #[test]
    fn a_figure_with_no_visible_task_is_kept_in_an_explicit_review_state() {
        store();
        let mut reply: Value = serde_json::from_str(&figure_only_reply("", false)).unwrap();
        reply["exams"] = serde_json::json!([]);
        let got = import_text(&reply.to_string(), Some(&worksheet_page(false)), 2, "w").unwrap();
        let q = &got.doc.questions[0];
        assert_eq!(q.stem, UNRESOLVED_TASK);
        assert!(q.topics.iter().any(|t| t == UNRESOLVED_TOPIC) && q.image.is_some());
        assert!(!q.stem.to_lowercase().contains("find") && !q.stem.to_lowercase().contains("solve"), "the placeholder names no task");
        assert_eq!(got.items[0].status, "saved_figure_only_unresolved");
        assert!(got.items[0].directions.is_none());
        assert!(got.warnings.iter().any(|w| w.contains("no task directions are visible")));
    }

    fn continuation_reply(instruction: &str, inherited: Option<u64>) -> String {
        let mut section = serde_json::json!({"title":"","instructions":instruction});
        if let Some(p) = inherited {
            section["inheritedFromPage"] = serde_json::json!(p);
        }
        serde_json::json!({"format":FORMAT,"bank":{"name":"Fractions","questions":[
            {"id":"q1","label":"15","type":"short-answer","stem":"$(9)/(5) - (5)/(8)$"},
            {"id":"q2","label":"16","type":"short-answer","stem":"$(7)/(3) + (1)/(2)$"}]},
            "exams":[{"name":"Fractions","sections":[section],"positions":[{"question":"q1","section":0},{"question":"q2","section":0}]}]}).to_string()
    }

    fn page_one_context() -> DocContext {
        let first = import_text(&continuation_reply("Evaluate each expression.", None), None, 1, "w").unwrap();
        DocContext { document: "arithmetic-fractions".into(), directions: Vec::new() }.after_page(1, &first.items)
    }

    #[test]
    fn a_continuation_page_inherits_the_directions_of_its_own_document_and_records_where_from() {
        let context = page_one_context();
        assert_eq!(context.directions.len(), 1);
        assert_eq!(context.directions[0].page, 1);
        assert_eq!(context.directions[0].labels, ["15", "16"]);
        let block = context.prompt_block(2);
        assert!(block.contains("page 2") && block.contains("printed on page 1, questions 15-16") && block.contains("Evaluate each expression."));
        let got = import_text_in(&continuation_reply("Evaluate each expression.", Some(1)), None, 2, "w", Some(&context)).unwrap();
        assert_eq!(got.doc.questions[0].stem, "Evaluate each expression.\n\n$(9)/(5) - (5)/(8)$");
        let d = got.items[0].directions.as_ref().unwrap();
        assert_eq!((d.origin.as_str(), d.from_page, d.document.as_deref()), ("inherited_from_document_context", Some(1), Some("arithmetic-fractions")));
        // Page-only reading of the same reply has no context, so a claimed inheritance is refused, not guessed.
        let alone = import_text(&continuation_reply("Evaluate each expression.", Some(1)), None, 2, "w").unwrap();
        assert_eq!(alone.doc.questions[0].stem, "$(9)/(5) - (5)/(8)$");
        assert!(alone.warnings.iter().any(|w| w.contains("not in the document context")));
    }

    #[test]
    fn a_newly_printed_task_wins_and_directions_the_context_never_listed_are_not_accepted() {
        let context = page_one_context();
        let fresh = import_text_in(&continuation_reply("Simplify each fraction.", None), None, 2, "w", Some(&context)).unwrap();
        assert!(fresh.doc.questions[0].stem.starts_with("Simplify each fraction."));
        assert_eq!(fresh.items[0].directions.as_ref().unwrap().origin, "printed_on_page");
        let made_up = import_text_in(&continuation_reply("Round to two decimals.", Some(1)), None, 2, "w", Some(&context)).unwrap();
        assert_eq!(made_up.doc.questions[0].stem, "$(9)/(5) - (5)/(8)$");
        // The next page carries the newest task last and keeps the older one only as history.
        let next = context.after_page(2, &fresh.items);
        assert_eq!(next.directions.iter().map(|d| d.text.as_str()).collect::<Vec<_>>(), ["Evaluate each expression.", "Simplify each fraction."]);
    }

    #[test]
    fn a_new_document_starts_with_no_context() {
        let fresh = DocContext { document: "geometry-volume-solids".into(), directions: Vec::new() };
        assert!(fresh.is_empty() && fresh.prompt_block(1).is_empty());
        assert_eq!(label_number("Question 13."), Some(13));
        assert_eq!(label_number(" 7) "), Some(7));
        assert_eq!(label_number("5B-2"), None);
    }

    #[test]
    fn inherited_task_requires_its_actual_earlier_page_and_a_well_typed_claim() {
        let context = page_one_context();
        for claim in [serde_json::json!(2), serde_json::json!(0), serde_json::json!("1"), serde_json::json!(-1), serde_json::json!(1.5)] {
            let mut reply = json_object(&continuation_reply("Evaluate each expression.", Some(1))).unwrap();
            reply["exams"][0]["sections"][0]["inheritedFromPage"] = claim.clone();
            let got = import_text_in(&reply.to_string(), None, 3, "w", Some(&context)).unwrap();
            assert!(got.items.iter().all(|i| i.directions.is_none()), "accepted invalid origin {claim}");
            assert!(got.warnings.iter().any(|w| w.contains("not in the document context")));
        }
        let mut future = context.clone();
        future.directions[0].page = 3;
        let got = import_text_in(&continuation_reply("Evaluate each expression.", Some(3)), None, 3, "w", Some(&future)).unwrap();
        assert!(got.items.iter().all(|i| i.directions.is_none()));
    }

    #[test]
    fn task_inheritance_preserves_case_sensitive_math_symbols() {
        let context = DocContext { document: "symbols".into(), directions: vec![ContextDirection {
            text: "Find A.".into(), page: 1, labels: vec!["1".into()], headings:Vec::new(), heading_page:None,
        }] };
        let got = import_text_in(&continuation_reply("Find a.", Some(1)), None, 2, "w", Some(&context)).unwrap();
        assert!(got.items.iter().all(|i| i.directions.is_none()), "A and a are different givens");
    }

    #[test]
    fn carrying_a_task_forward_does_not_move_new_question_labels_to_its_source_page() {
        let context = page_one_context();
        let mut reply = json_object(&continuation_reply("Evaluate each expression.", Some(1))).unwrap();
        reply["bank"]["questions"][0]["label"] = serde_json::json!(17);
        reply["bank"]["questions"][1]["label"] = serde_json::json!(18);
        let imported = import_text_in(&reply.to_string(), None, 2, "w", Some(&context)).unwrap();
        let next = context.after_page(2, &imported.items);
        assert_eq!(next.directions[0], context.directions[0]);
        assert!(next.prompt_block(3).contains("printed on page 1, questions 15-16"));
        assert!(!next.prompt_block(3).contains("questions 17-18"));
    }

    fn page() -> Vec<u8> {
        // White paper with a dark block in the middle: a crop of the block has ink, a crop of the margin does not.
        let mut img = image::RgbaImage::from_pixel(200, 300, image::Rgba([255, 255, 255, 255]));
        for y in 30..270 {
            for x in 30..170 {
                img.put_pixel(x, y, image::Rgba([0, 0, 0, 255]));
            }
        }
        let mut out = Vec::new();
        image::DynamicImage::ImageRgba8(img).write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png).unwrap();
        out
    }

    fn store() {
        let dir = std::env::temp_dir().join(format!("ditto-test-{}", std::process::id()));
        unsafe { std::env::set_var("XDG_DATA_HOME", dir) };
    }

    #[test]
    fn bad_question_never_costs_the_page() {
        store();
        let t = r#"{"format":"queshion/ditto","formatVersion":"0.1.0","bank":{"name":"B","questions":[
          {"id":"q1","type":"multiple-choice","stem":"Pick","choices":[{"text":"a","correct":true},{"text":"b"}]},
          {"id":"q2","type":"mystery","stem":"?"},
          {"id":"q3","type":"fill-in-the-blank","stem":"A ___ and ___","blanks":[["x"],["y","z"]]}]}}"#;
        let r = import_text(t, None, 1, "n").unwrap();
        assert_eq!(r.doc.questions.len(), 2);
        assert_eq!(r.warnings.len(), 1);
    }

    #[test]
    fn grouped_row_answers_are_flattened_one_per_blank() {
        store();
        let t = r#"{"format":"queshion/ditto","bank":{"questions":[{"type":"fill-in-the-blank","stem":"KI ___ ___\nSO3 ___ ___","blanks":[["Ionic","potassium iodide"],["Covalent","sulfur trioxide"]]}]}}"#;
        let r = import_text(t, None, 1, "n").unwrap();
        match &r.doc.questions[0].kind {
            Kind::Blank { accept } => assert_eq!(accept, &vec![vec!["Ionic".to_string()], vec!["potassium iodide".into()], vec!["Covalent".into()], vec!["sulfur trioxide".into()]]),
            k => panic!("{k:?}"),
        }
    }

    const LEAD: &str = "Name the following compounds. Be sure to determine whether each is ionic or covalent so that you use the correct rules.";
    const CLEAN: &str = "Formula | Ionic or Covalent? | Name\nMg(NO₂)₂ | ___ | ___\nSO₃ | ___ | ___\nNO | ___ | ___\nKI | ___ | ___\nP₂S₅ | ___ | ___";
    const KEY: [&str; 10] = ["Ionic", "magnesium nitrite", "Covalent", "sulfur trioxide", "Covalent", "nitrogen monoxide", "Ionic", "potassium iodide", "Covalent", "diphosphorus pentasulfide"];

    fn import_one(stem: &str, blanks: Value) -> Question {
        store();
        let v = serde_json::json!({"format": FORMAT, "bank": {"questions": [{"type": "fill-in-the-blank", "stem": stem, "blanks": blanks}]}});
        import_text(&v.to_string(), None, 1, "n").unwrap().doc.questions.remove(0)
    }

    fn key_per_blank() -> Vec<Vec<String>> {
        KEY.iter().map(|k| vec![k.to_string()]).collect()
    }

    #[test]
    fn ch4_every_row_repeats_the_header_label_real_model_output() {
        // Real gemma4 output (trial run, page 1): header row AND every row carry "label ___",
        // answers grouped by row and padded with empties to the 12 markers it wrote.
        let stem = format!(
            "{LEAD}\nFormula | Ionic or Covalent? ___ | Name ___\nMg(NO₂)₂ | Ionic or Covalent? ___ | Name ___\nSO₃ | Ionic or Covalent? ___ | Name ___\nNO | Ionic or Covalent? ___ | Name ___\nKI | Ionic or Covalent? ___ | Name ___\nP₂S₅ | Ionic or Covalent? ___ | Name ___"
        );
        let grouped = serde_json::json!([["Ionic", "magnesium nitrite"], ["Covalent", "sulfur trioxide"], ["Covalent", "nitrogen monoxide"], ["Ionic", "potassium iodide"], ["Covalent", "diphosphorus pentasulfide"], [], [], [], [], [], [], []]);
        let q = import_one(&stem, grouped);
        assert_eq!(q.stem, format!("{LEAD}\n{CLEAN}"));
        assert!(matches!(&q.kind, Kind::Blank { accept } if *accept == key_per_blank()), "{:?}", q.kind);
        // The same shape with one entry per ORIGINAL marker (header ones empty) lines up too.
        let mut per_marker = vec![serde_json::json!([]), serde_json::json!([])];
        per_marker.extend(KEY.iter().map(|k| serde_json::json!([k])));
        let q = import_one(&stem, Value::Array(per_marker));
        assert!(matches!(&q.kind, Kind::Blank { accept } if *accept == key_per_blank()), "{:?}", q.kind);
        // No key at all: the title-like first cell still marks the header.
        let q = import_one(&stem, serde_json::json!([]));
        assert_eq!(q.stem, format!("{LEAD}\n{CLEAN}"));
        assert!(matches!(&q.kind, Kind::Blank { accept } if accept.len() == 10));
    }

    #[test]
    fn ch4_header_row_with_blanks_over_clean_rows() {
        // Real battery output: only the header row carries the stray blanks.
        let stem = format!("{LEAD}\nFormula | Ionic or Covalent? ___ | Name ___\nMg(NO₂)₂ | ___ | ___\nSO₃ | ___ | ___\nNO | ___ | ___\nKI | ___ | ___\nP₂S₅ | ___ | ___");
        let q = import_one(&stem, Value::Array(KEY.iter().map(|k| serde_json::json!([k])).collect()));
        assert_eq!(q.stem, format!("{LEAD}\n{CLEAN}"));
        assert!(matches!(&q.kind, Kind::Blank { accept } if *accept == key_per_blank()), "{:?}", q.kind);
        // Real battery output (rotated page): each alternative written twice.
        let q = import_one(&stem, Value::Array(KEY.iter().map(|k| serde_json::json!([k, k])).collect()));
        assert!(matches!(&q.kind, Kind::Blank { accept } if *accept == key_per_blank()), "{:?}", q.kind);
    }

    #[test]
    fn ch4_rows_repeat_labels_without_a_header_row() {
        // Real battery output (5B): no header row at all; one header is synthesised.
        let stem = format!(
            "{LEAD}\nN₂O | Ionic or Covalent? ___ | Name ___\nKI | Ionic or Covalent? ___ | Name ___\nCa(NO₂)₂ | Ionic or Covalent? ___ | Name ___\nP₂S₅ | Ionic or Covalent? ___ | Name ___\nSO₂ | Ionic or Covalent? ___ | Name ___"
        );
        let key = ["Covalent", "dinitrogen monoxide", "Ionic", "potassium iodide", "Ionic", "calcium nitrite", "Covalent", "diphosphorus pentasulfide", "Covalent", "sulfur dioxide"];
        let q = import_one(&stem, Value::Array(key.iter().map(|k| serde_json::json!([k])).collect()));
        assert_eq!(q.stem, format!("{LEAD}\n|  | Ionic or Covalent? | Name |\nN₂O | ___ | ___\nKI | ___ | ___\nCa(NO₂)₂ | ___ | ___\nP₂S₅ | ___ | ___\nSO₂ | ___ | ___"));
        assert!(matches!(&q.kind, Kind::Blank { accept } if accept.len() == 10 && accept[1] == vec!["dinitrogen monoxide".to_string()]));
        match &crate::richtext::parse(&q.stem)[1] {
            crate::richtext::Block::Table { header, rows } => {
                assert!(*header);
                assert_eq!(rows.len(), 6);
                assert!(rows.iter().all(|r| r.len() == 3));
            }
            b => panic!("{b:?}"),
        }
    }

    #[test]
    fn clean_tables_and_plain_stems_are_left_alone() {
        let stem = format!("{LEAD}\n{CLEAN}");
        assert_eq!(normalize_tables(&stem, &[10]), TableFix { stem: stem.clone(), dropped: vec![] });
        let q = import_one(&stem, Value::Array(KEY.iter().map(|k| serde_json::json!([k])).collect()));
        assert_eq!(q.stem, stem);
        for s in ["A ___ and ___", "x | y", "Bonds | Lone pairs\n___ | ___", "a | b\n1 | 2", "• Draw it.\n• Name it."] {
            assert_eq!(normalize_tables(s, &[]).stem, s, "{s}");
        }
        // Other types get the same cleaning, without answers to align.
        store();
        let v = serde_json::json!({"format": FORMAT, "bank": {"questions": [{"type": "short-answer", "stem": "Fill in:\nKI | Name ___\nNO | Name ___"}]}});
        let q = import_text(&v.to_string(), None, 1, "n").unwrap().doc.questions.remove(0);
        assert_eq!(q.stem, "Fill in:\n|  | Name |\nKI | ___\nNO | ___");
    }

    #[test]
    fn blank_without_markers_becomes_short() {
        store();
        let t = r#"{"format":"queshion/ditto","bank":{"questions":[{"type":"fill-in-the-blank","stem":"Bonding: 14","blanks":[["14"],["10"]]}]}}"#;
        let r = import_text(t, None, 1, "n").unwrap();
        assert!(matches!(&r.doc.questions[0].kind, Kind::Short { sample } if sample == "14; 10"));
    }

    #[test]
    fn figure_box_is_cropped_and_tolerates_fences() {
        store();
        let t = "```json\n{\"format\":\"queshion/ditto\",\"bank\":{\"questions\":[{\"type\":\"short-answer\",\"stem\":\"See\",\"figure\":{\"box\":[0.2,0.2,0.6,0.5]}},{\"type\":\"short-answer\",\"stem\":\"Bad\",\"figure\":{\"box\":[0,0,1,1]}}]}}\n```";
        let r = import_text(t, Some(&page()), 1, "n").unwrap();
        assert_eq!(r.figures, 1);
        assert!(r.doc.questions[0].image.is_some() && r.doc.questions[1].image.is_none());
        assert_eq!(r.warnings.len(), 1);
    }

    #[test]
    fn a_box_is_read_in_the_models_native_grid_or_in_legacy_fractions() {
        // Native: [ymin, xmin, ymax, xmax] on a 0..1000 grid (a value above 1 says so).
        assert_eq!(box_fractions(&[100.0, 200.0, 400.0, 600.0]), [0.2, 0.1, 0.6, 0.4]);
        // Legacy: [x0, y0, x1, y1] fractions, unchanged, including the whole-page box.
        assert_eq!(box_fractions(&[0.2, 0.1, 0.6, 0.4]), [0.2, 0.1, 0.6, 0.4]);
        assert_eq!(box_fractions(&[0.0, 0.0, 1.0, 1.0]), [0.0, 0.0, 1.0, 1.0]);
        // The whole page on the grid is still the whole page.
        assert_eq!(box_fractions(&[0.0, 0.0, 1000.0, 1000.0]), [0.0, 0.0, 1.0, 1.0]);
    }

    /// The size of what `crop` cuts from the 200x300 test page for a figure with this box.
    fn cropped_size(box_json: &str) -> Result<(u32, u32), String> {
        let page = image::load_from_memory(&page()).unwrap();
        let fig: Value = serde_json::from_str(&format!(r#"{{"box":{box_json}}}"#)).unwrap();
        let png = crop(&fig, Some(&page), 1)?;
        let img = image::load_from_memory(&png).unwrap();
        Ok((img.width(), img.height()))
    }

    #[test]
    fn a_grid_box_crops_the_same_region_as_its_fraction_twin() {
        // The same rectangle, once on the grid and once as fractions, on a 200x300 page. (Calls the
        // pure `crop`, not the whole import: the import saves into the process-wide store dir, which
        // other tests change concurrently.)
        let grid = cropped_size("[100,200,400,600]").expect("grid box cropped");
        let frac = cropped_size("[0.2,0.1,0.6,0.4]").expect("fraction box cropped");
        assert_eq!(grid, frac);
        // x 0.2..0.6 of 200 px (40..120) and y 0.1..0.4 of 300 px (30..120) on a dark block spanning 30..170 by 30..270:
        // the crop grows only through ink that continues from the box (here the block, up to 7 px at the bottom) plus
        // a 4 px margin of clean paper where the block starts: width 80 + 5 + 5, height 90 + 4 + 7.
        assert_eq!(grid, (90, 101), "{grid:?}");
    }

    #[test]
    fn a_model_that_puts_the_alt_text_inside_the_box_array_still_crops() {
        // Seen from the live model: the description rode along as a fifth element of `box`. A string
        // is not a number, so only the four numbers remain and the crop goes ahead.
        assert_eq!(cropped_size(r#"[100,200,400,600,"A right triangle"]"#), cropped_size("[100,200,400,600]"));
        assert!(cropped_size("[100,200,400,600]").is_ok());
    }

    #[test]
    fn a_box_over_bare_paper_is_dropped_as_blank() {
        // The margin of the test page (x 0..30) is white: a box there is the wrong box.
        let err = cropped_size("[100,0,400,100]").unwrap_err();
        assert!(err.contains("blank"), "{err}");
        // The same size over the dark block crops fine.
        assert!(cropped_size("[300,300,600,600]").is_ok());
        // A thin line drawing still counts as a picture.
        let mut img = image::RgbaImage::from_pixel(200, 300, image::Rgba([255, 255, 255, 255]));
        for x in 20..180 {
            img.put_pixel(x, 150, image::Rgba([0, 0, 0, 255]));
        }
        assert!(!is_blank(&image::DynamicImage::ImageRgba8(img)));
    }
}

#[cfg(test)]
mod child_figure_tests {
    use super::*;
    fn isolated_store() {
        crate::store::init(Box::new(crate::store::FsBackend::new(std::env::temp_dir().join(format!("vershion-child-figure-{}", std::process::id())))));
    }
    fn page() -> Vec<u8> {
        let mut im = image::RgbImage::from_pixel(400,400,image::Rgb([255,255,255]));
        for k in 100..300 { for t in 0..3 {
            im.put_pixel(k,100+t,image::Rgb([0,0,0])); im.put_pixel(k,297+t,image::Rgb([0,0,0]));
            im.put_pixel(100+t,k,image::Rgb([0,0,0])); im.put_pixel(297+t,k,image::Rgb([0,0,0]));
        }}
        let mut png=Vec::new(); image::DynamicImage::ImageRgb8(im).write_to(&mut std::io::Cursor::new(&mut png),image::ImageFormat::Png).unwrap(); png
    }
    fn package() -> Value {
        serde_json::json!({"format":FORMAT,"bank":{"questions":[{"id":"q1","label":"7","type":"multipart","stem":"Shared source givens.","parts":[
            {"type":"multiple-choice","stem":"a) Choose one.","choices":[{"text":"Choice A","correct":false},{"text":"Choice B","correct":true}]},
            {"type":"short-answer","stem":"b) Use this supplied diagram.","suggestedAnswer":"Existing source key","figure":{"page":1,"box":[0.22,0.22,0.78,0.78],"alt":"Supplied outline"}},
            {"type":"short-answer","stem":"c) Explain without another figure.","suggestedAnswer":""}
        ]}]},"exams":[{"name":"Source","sections":[{"title":"Literal source section","instructions":"Use the printed givens."}],"positions":[{"question":"q1","section":0}]}]})
    }
    #[test]
    fn unique_child_figure_survives_persistence_with_its_owner_and_all_other_parts() {
        isolated_store(); let raw=package(); let before=raw.clone();
        assert!(figure_box_issues(&raw.to_string(),Some(&page()),1).is_empty());
        let got=import_text(&raw.to_string(),Some(&page()),1,"source").unwrap();
        assert_eq!(raw,before,"selection/promotion cannot mutate the source JSON");
        assert_eq!(got.figures,1); assert_eq!(got.doc.questions.len(),1);
        let q=&got.doc.questions[0]; assert!(q.image.is_some() && q.figure.is_none(),"a faithful crop is ready for the ordinary fitter, not falsely certified editable");
        assert!(q.stem.contains("Shared source givens.") && q.stem.contains("Literal source section") && q.stem.contains("Use the printed givens."));
        assert_eq!(got.items[0].headings,["Literal source section"]);
        assert_eq!(got.items[0].figure_source,Some(FigureSource { origin:"multipart_part".into(), path:"/parts/1/figure".into(), part_index:Some(1) }));
        assert_eq!(got.items[0].status,"saved"); assert!(got.items[0].figure_review.is_empty());
        let serialized=serde_json::to_string(q).unwrap(); let restored:Question=serde_json::from_str(&serialized).unwrap();
        let Kind::Multipart { parts }=&restored.kind else { panic!("multipart retained") };
        assert_eq!(parts.len(),3); assert_eq!(parts[0].stem,"a) Choose one.");
        assert_eq!(parts[1].stem,"b) Use this supplied diagram.\n[picture]"); assert_eq!(parts[2].stem,"c) Explain without another figure.");
        assert!(matches!(&parts[0].kind,crate::model::PartKind::Mc {answers} if answers.len()==2 && !answers[0].correct && answers[1].correct && answers[1].text=="Choice B"));
        assert!(matches!(&parts[1].kind,crate::model::PartKind::Short {sample} if sample=="Existing source key"));
        assert!(matches!(&parts[2].kind,crate::model::PartKind::Short {sample} if sample.is_empty()));
        let preview=source_figure_previews(&raw.to_string(),&page(),1);
        assert_eq!((preview[0].0,preview[0].1.as_str()),(0,"7"));
        let crop=image::load_from_memory(&preview[0].2).unwrap().to_rgba8();
        let saved=image::load_from_memory(&crate::store::read_media(&q.image.as_ref().unwrap().file).unwrap()).unwrap().to_rgba8();
        assert_eq!(crop,saved,"preflight and import use exactly the same source crop");
    }
    #[test]
    fn child_box_preflight_and_bounds_only_correction_preserve_its_original_owner() {
        let original=package(); let mut correction=original.clone();
        correction["bank"]["questions"][0]["parts"][1]["figure"]["box"]=serde_json::json!([0.,0.,1.,1.]);
        let issues=figure_box_issues(&correction.to_string(),Some(&page()),1);
        assert!(issues.iter().any(|i|i.kind=="invalid_crop" && i.indices==[0]));
        assert!(source_figure_previews(&correction.to_string(),&page(),1).is_empty());
        let mut fix=original.clone(); fix["bank"]["questions"][0]["stem"]=Value::String("Invented replacement rejected by bounds merge".into());
        let fixed=corrected_figure_boxes(&correction.to_string(),&fix.to_string(),&[0]).unwrap();
        assert_eq!(json_object(&fixed).unwrap(),original,"only original child bounds/page can change");
        // A correction may return the normalized single parent slot; the original child remains owner.
        let mut promoted=fix; let fig=promoted["bank"]["questions"][0]["parts"][1].as_object_mut().unwrap().remove("figure").unwrap();
        promoted["bank"]["questions"][0]["figure"]=fig;
        let fixed=corrected_figure_boxes(&correction.to_string(),&promoted.to_string(),&[0]).unwrap();
        assert_eq!(json_object(&fixed).unwrap(),original);
        assert!(figure_box_issues(&fixed,Some(&page()),1).is_empty());
    }
    #[test]
    fn multiple_children_parent_child_and_orphan_markers_require_explicit_review() {
        isolated_store();
        for mode in ["multiple","parent","marker"] {
            let mut raw=package(); let q=&mut raw["bank"]["questions"][0];
            match mode { "multiple"=>q["parts"][2]["figure"]=q["parts"][1]["figure"].clone(),
                "parent"=>q["figure"]=q["parts"][1]["figure"].clone(), _=>q["stem"]=Value::String("Shared source givens.\n[picture]".into()) }
            assert!(figure_box_issues(&raw.to_string(),Some(&page()),1).iter().any(|i|i.kind=="figure_ownership"));
            assert!(source_figure_previews(&raw.to_string(),&page(),1).is_empty());
            let got=import_text(&raw.to_string(),Some(&page()),1,"source").unwrap();
            assert_eq!(got.doc.questions.len(),1); assert!(got.doc.questions[0].image.is_none()); assert_eq!(got.figures,0);
            assert_eq!(got.items[0].status,"saved_figure_ownership_unresolved"); assert!(!got.items[0].figure_review.is_empty());
            assert!(got.warnings.iter().any(|s|s.contains("ownership requires review")));
            let Kind::Multipart {parts}=&got.doc.questions[0].kind else { panic!("parts retained") };
            assert_eq!(parts.len(),3); assert_eq!(parts[1].stem,"b) Use this supplied diagram.");
            assert!(matches!(&parts[1].kind,crate::model::PartKind::Short {sample} if sample=="Existing source key"));
        }
    }
    #[test]
    fn child_promotion_never_saves_a_truncated_multipart_when_another_part_is_invalid() {
        isolated_store();let mut raw=package();
        raw["bank"]["questions"][0]["parts"][0]["choices"]=serde_json::json!([{"text":"Only one source choice"}]);
        let got=import_text(&raw.to_string(),Some(&page()),1,"source").unwrap();
        assert!(got.doc.questions.is_empty(),"do not silently hide the invalid earlier part or shift the figure owner");
        assert_eq!(got.items[0].status,"dropped");
        assert!(got.items[0].reason.as_deref().unwrap().contains("cannot preserve all multipart parts"));
        assert_eq!(raw["bank"]["questions"][0]["parts"][0]["choices"][0]["text"],"Only one source choice");
    }

}
