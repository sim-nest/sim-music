//! Runtime expression surface for bounded counterpoint generation.

use sim_kernel::{Cx, Error, Expr, Result, Value};
use sim_lib_discrete_search::{NeverInterrupt, SearchControl, SearchOrder, SearchStatus};
use sim_lib_music_shapes::{decode_melody, encode_counterpoint};

use crate::runtime::{
    expect_keyword, integer, keyword_name, map, named_rules, parse_usize, scalar_text, strings,
    symbol, symbolish, time_expr, unquote, unquote_ref, value_expr,
};
use crate::{
    CounterpointGeneration, CounterpointGenerationPolicy, CounterpointGenerationReceipt,
    CounterpointGenerationResult, generate_counterpoint,
};

pub(crate) fn generate_call(cx: &mut Cx, args: &[Expr], evaluate_values: bool) -> Result<Value> {
    let [
        cantus,
        rules_key,
        rules,
        voices_key,
        voices,
        control_key,
        control,
    ] = args
    else {
        return Err(Error::Eval(
            "music/counterpoint/generate expects CANTUS :rules SYMBOL :voices INTEGER :control MAP"
                .to_owned(),
        ));
    };
    expect_keyword(rules_key, "rules")?;
    expect_keyword(voices_key, "voices")?;
    expect_keyword(control_key, "control")?;
    let cantus = value_expr(cx, cantus, evaluate_values)?;
    let Expr::String(cantus) = unquote(cantus) else {
        return Err(Error::TypeMismatch {
            expected: "canonical #(Melody ...) string",
            found: "non-string",
        });
    };
    let cantus = decode_melody(&cantus)
        .map_err(|error| Error::Eval(format!("invalid counterpoint cantus: {error}")))?;
    let rules = value_expr(cx, rules, evaluate_values)?;
    let rules = named_rules(&symbolish(&rules)?)?;
    let voices = value_expr(cx, voices, evaluate_values)?;
    let mut policy = CounterpointGenerationPolicy {
        voices: parse_usize(&voices)?,
        ..CounterpointGenerationPolicy::default()
    };
    let control = value_expr(cx, control, evaluate_values)?;
    let control = parse_generation_control(&control, &mut policy)?;
    let generated = generate_counterpoint(&cantus, &rules, &policy, control, &NeverInterrupt)
        .map_err(|error| Error::Eval(error.to_string()))?;
    cx.factory().expr(counterpoint_generation_expr(&generated))
}

fn parse_generation_control(
    expr: &Expr,
    policy: &mut CounterpointGenerationPolicy,
) -> Result<SearchControl> {
    let Expr::Map(entries) = unquote_ref(expr) else {
        return Err(Error::TypeMismatch {
            expected: "counterpoint generation control map",
            found: "non-map",
        });
    };
    let mut control = SearchControl::default().with_max_frontier(4_096);
    for (key, value) in entries {
        match keyword_name(key)?.as_str() {
            "work" => control.max_work = Some(parse_u64(value)?),
            "frontier" => control.max_frontier = Some(parse_usize(value)?),
            "results" => control.max_results = Some(parse_usize(value)?),
            "memory" => control.max_memory_nodes = Some(parse_usize(value)?),
            "seed" => control.seed = parse_u64(value)?,
            "minimum-pitch-changes" => {
                policy.diversity.minimum_pitch_changes = parse_usize(value)?;
            }
            "order" => {
                control.order = match symbolish(value)?.as_str() {
                    "depth-first" => SearchOrder::DepthFirst,
                    "breadth-first" => SearchOrder::BreadthFirst,
                    "best-first" => SearchOrder::BestFirst,
                    "a-star" => SearchOrder::AStar,
                    other => {
                        return Err(Error::Eval(format!(
                            "unknown counterpoint search order {other}"
                        )));
                    }
                };
            }
            other => {
                return Err(Error::Eval(format!(
                    "unknown music/counterpoint generation control :{other}"
                )));
            }
        }
    }
    Ok(control)
}

fn counterpoint_generation_expr(generation: &CounterpointGeneration) -> Expr {
    map(vec![
        ("mode", Expr::String("generated-counterpoint".to_owned())),
        ("rule-set", Expr::String(generation.csp.rule_set.clone())),
        (
            "csp",
            map(vec![
                ("variables", integer(generation.csp.variables.len())),
                ("slots", integer(generation.csp.slots())),
                ("rhythm", time_expr(generation.csp.rhythm)),
                (
                    "domain-sizes",
                    Expr::Vector(
                        generation
                            .csp
                            .domains
                            .iter()
                            .map(|domain| integer(domain.pitches.len()))
                            .collect(),
                    ),
                ),
                ("facts", strings(&generation.csp.facts)),
            ]),
        ),
        (
            "results",
            Expr::Vector(
                generation
                    .results
                    .iter()
                    .map(counterpoint_generation_result_expr)
                    .collect(),
            ),
        ),
        ("receipt", generation_receipt_expr(&generation.receipt)),
    ])
}

fn counterpoint_generation_result_expr(result: &CounterpointGenerationResult) -> Expr {
    map(vec![
        (
            "counterpoint",
            Expr::String(encode_counterpoint(&result.counterpoint)),
        ),
        ("score", integer(result.score)),
        ("fingerprint", Expr::String(result.fingerprint.clone())),
        ("legal", Expr::Bool(result.analysis.is_legal())),
        (
            "patch",
            map(vec![
                ("base", Expr::String(content_id_text(&result.patch.base))),
                ("additions", integer(result.patch.additions.len())),
                ("operation", Expr::String("additions-only".to_owned())),
                (
                    "inverse",
                    Expr::String("remove(apply(source,patch),patch)==source".to_owned()),
                ),
            ]),
        ),
    ])
}

fn generation_receipt_expr(receipt: &CounterpointGenerationReceipt) -> Expr {
    map(vec![
        (
            "status",
            symbol(search_status_label(&receipt.search.status)),
        ),
        (
            "reason",
            receipt
                .search
                .reason
                .clone()
                .map_or(Expr::Nil, Expr::String),
        ),
        ("work-used", integer(receipt.search.work_used)),
        ("expanded", integer(receipt.search.expanded)),
        ("propagated", integer(receipt.search.propagated)),
        ("pruned", integer(receipt.search.pruned)),
        ("max-frontier", integer(receipt.search.max_frontier)),
        ("raw-results", integer(receipt.raw_result_count)),
        ("selected-results", integer(receipt.selected_result_count)),
        ("diversity-rejected", integer(receipt.diversity_rejected)),
        ("seed", integer(receipt.search.seed)),
        (
            "policy-digest",
            Expr::String(receipt.search.policy_digest.clone()),
        ),
        ("digest", Expr::String(receipt.search.digest.clone())),
        ("facts", strings(&receipt.facts)),
    ])
}

fn search_status_label(status: &SearchStatus) -> &'static str {
    match status {
        SearchStatus::Complete => "complete",
        SearchStatus::Partial => "partial",
        SearchStatus::Cancelled => "cancelled",
        SearchStatus::Infeasible => "infeasible",
    }
}

fn content_id_text(id: &sim_kernel::ContentId) -> String {
    let digest = id
        .bytes
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("{}:{digest}", id.algorithm)
}

fn parse_u64(expr: &Expr) -> Result<u64> {
    let value = scalar_text(expr)?;
    value
        .parse()
        .map_err(|_| Error::Eval(format!("invalid u64 {value}")))
}
