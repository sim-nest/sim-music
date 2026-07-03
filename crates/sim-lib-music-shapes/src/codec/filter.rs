use sim_codec::DomainValue;
use sim_kernel::{Error, Expr, Result as KernelResult, Symbol};
use sim_lib_music_core::{LaneId, LaneKind, TracePolicy};
use sim_lib_music_transform::{
    CallableFilterRef, CustomFilter, DeterminismPolicy, FilterBody, FilterCapability,
    FilterCapabilitySet, FilterOp, FilterPredicate, FilterRule, FilterShape, lane_kind_from_wire,
};

use super::analysis::{field, field_atom, field_list, parse_node};
use super::{MusicShapeError, encode_string};

const NS: &str = "music/custom-filter";

/// Encodes a `CustomFilter` into a tagged `Expr` map for kernel interchange.
pub fn custom_filter_to_expr(filter: &CustomFilter) -> Expr {
    map(vec![
        ("tag", tag_expr("custom-filter")),
        ("id", Expr::String(filter.id.clone())),
        ("input", shape_to_expr(&filter.input)),
        ("output", shape_to_expr(&filter.output)),
        (
            "capabilities",
            Expr::Vector(
                filter
                    .capabilities
                    .iter()
                    .map(|capability| {
                        Expr::Symbol(Symbol::qualified(
                            "music/filter-capability",
                            capability.wire_label(),
                        ))
                    })
                    .collect(),
            ),
        ),
        (
            "determinism",
            Expr::Symbol(Symbol::qualified(
                "music/filter-determinism",
                filter.determinism.wire_label(),
            )),
        ),
        ("trace", trace_to_expr(filter.trace)),
        ("body", body_to_expr(&filter.body)),
    ])
}

/// Reconstructs a `CustomFilter` from the tagged `Expr` map form.
pub fn custom_filter_from_expr(expr: &Expr) -> KernelResult<CustomFilter> {
    let entries = expr_map(expr, "custom filter")?;
    expect_tag(entries, "custom-filter", "custom filter")?;
    let id = expr_string(lookup_required(entries, "id")?, "filter id")?.to_owned();
    let input = shape_from_expr(lookup_required(entries, "input")?)?;
    let output = shape_from_expr(lookup_required(entries, "output")?)?;
    let capabilities = capability_set_from_expr(lookup_required(entries, "capabilities")?)?;
    let determinism = determinism_from_expr(lookup_required(entries, "determinism")?)?;
    let trace = trace_from_expr(lookup_required(entries, "trace")?)?;
    let body = body_from_expr(lookup_required(entries, "body")?)?;
    CustomFilter::new(id, input, output, capabilities, determinism, trace, body)
        .map_err(|err| Error::Eval(err.to_string()))
}

/// Encodes a `CustomFilter` as its `#(CustomFilter ...)` text form.
pub fn encode_custom_filter(filter: &CustomFilter) -> String {
    format!(
        "#(CustomFilter id={} input={} output={} caps=[{}] determinism={} trace={} body={})",
        encode_string(&filter.id),
        encode_shape(&filter.input),
        encode_shape(&filter.output),
        filter
            .capabilities
            .iter()
            .map(|capability| capability.wire_label())
            .collect::<Vec<_>>()
            .join(","),
        filter.determinism.wire_label(),
        encode_trace(filter.trace),
        encode_body(&filter.body),
    )
}

/// Decodes a `#(CustomFilter ...)` text form into a `CustomFilter`.
pub fn decode_custom_filter(value: &str) -> Result<CustomFilter, MusicShapeError> {
    let node = parse_node(value)?;
    if node.name != "CustomFilter" {
        return Err(MusicShapeError::InvalidMusic);
    }
    let id = string_or_atom(field(&node, "id")?)?;
    let input = decode_shape(value_form(field(&node, "input")?)?)?;
    let output = decode_shape(value_form(field(&node, "output")?)?)?;
    let capabilities = decode_capabilities(field_list(&node, "caps")?)?;
    let determinism = DeterminismPolicy::from_wire_label(&field_atom(&node, "determinism")?)
        .ok_or(MusicShapeError::InvalidMusic)?;
    let trace = decode_trace(&field_atom(&node, "trace")?)?;
    let body = decode_body(value_form(field(&node, "body")?)?)?;
    CustomFilter::new(id, input, output, capabilities, determinism, trace, body)
        .map_err(|_| MusicShapeError::InvalidMusic)
}

fn shape_to_expr(shape: &FilterShape) -> Expr {
    map(vec![
        ("tag", tag_expr("filter-shape")),
        (
            "kinds",
            Expr::Vector(
                shape
                    .kinds()
                    .map(|kind| Expr::Symbol(LaneKind::symbol(kind)))
                    .collect(),
            ),
        ),
    ])
}

fn shape_from_expr(expr: &Expr) -> KernelResult<FilterShape> {
    let entries = expr_map(expr, "filter shape")?;
    expect_tag(entries, "filter-shape", "filter shape")?;
    FilterShape::new(
        expr_vector(lookup_required(entries, "kinds")?, "filter shape kinds")?
            .iter()
            .map(|expr| {
                lane_kind_from_wire(symbol_name(expr, "lane kind")?)
                    .ok_or_else(|| Error::Eval("lane kind is invalid".to_owned()))
            })
            .collect::<KernelResult<Vec<_>>>()?,
    )
    .map_err(|err| Error::Eval(err.to_string()))
}

fn capability_set_from_expr(expr: &Expr) -> KernelResult<FilterCapabilitySet> {
    Ok(FilterCapabilitySet::new(
        expr_vector(expr, "filter capabilities")?
            .iter()
            .map(|expr| {
                FilterCapability::from_wire_label(symbol_name(expr, "filter capability")?)
                    .ok_or_else(|| Error::Eval("filter capability is invalid".to_owned()))
            })
            .collect::<KernelResult<Vec<_>>>()?,
    ))
}

fn determinism_from_expr(expr: &Expr) -> KernelResult<DeterminismPolicy> {
    DeterminismPolicy::from_wire_label(symbol_name(expr, "filter determinism")?)
        .ok_or_else(|| Error::Eval("filter determinism is invalid".to_owned()))
}

fn trace_to_expr(trace: TracePolicy) -> Expr {
    Expr::Symbol(Symbol::qualified("music/filter-trace", encode_trace(trace)))
}

fn trace_from_expr(expr: &Expr) -> KernelResult<TracePolicy> {
    match symbol_name(expr, "filter trace")? {
        "off" => Ok(TracePolicy::Off),
        "diagnostics" => Ok(TracePolicy::Diagnostics),
        "full" => Ok(TracePolicy::Full),
        _ => Err(Error::Eval("filter trace is invalid".to_owned())),
    }
}

fn body_to_expr(body: &FilterBody) -> Expr {
    match body {
        FilterBody::Rule(rules) => map(vec![
            ("tag", tag_expr("filter-body")),
            ("kind", tag_expr("rule")),
            (
                "rules",
                Expr::Vector(rules.iter().map(rule_to_expr).collect()),
            ),
        ]),
        FilterBody::Callable(callable) => map(vec![
            ("tag", tag_expr("filter-body")),
            ("kind", tag_expr("callable")),
            ("name", Expr::String(callable.name.clone())),
        ]),
    }
}

fn body_from_expr(expr: &Expr) -> KernelResult<FilterBody> {
    let entries = expr_map(expr, "filter body")?;
    expect_tag(entries, "filter-body", "filter body")?;
    match symbol_name(lookup_required(entries, "kind")?, "filter body kind")? {
        "rule" => Ok(FilterBody::Rule(
            expr_vector(lookup_required(entries, "rules")?, "filter rules")?
                .iter()
                .map(rule_from_expr)
                .collect::<KernelResult<Vec<_>>>()?,
        )),
        "callable" => Ok(FilterBody::Callable(CallableFilterRef::new(expr_string(
            lookup_required(entries, "name")?,
            "callable filter name",
        )?))),
        _ => Err(Error::Eval("filter body kind is invalid".to_owned())),
    }
}

fn rule_to_expr(rule: &FilterRule) -> Expr {
    map(vec![
        ("tag", tag_expr("filter-rule")),
        ("when", predicate_to_expr(&rule.when)),
        ("op", op_to_expr(&rule.op)),
    ])
}

fn rule_from_expr(expr: &Expr) -> KernelResult<FilterRule> {
    let entries = expr_map(expr, "filter rule")?;
    expect_tag(entries, "filter-rule", "filter rule")?;
    Ok(FilterRule::new(
        predicate_from_expr(lookup_required(entries, "when")?)?,
        op_from_expr(lookup_required(entries, "op")?)?,
    ))
}

fn predicate_to_expr(predicate: &FilterPredicate) -> Expr {
    match predicate {
        FilterPredicate::Any => map(vec![
            ("tag", tag_expr("filter-predicate")),
            ("kind", tag_expr("any")),
        ]),
        FilterPredicate::Kind(kind) => map(vec![
            ("tag", tag_expr("filter-predicate")),
            ("kind", tag_expr("kind")),
            ("lane-kind", Expr::Symbol(LaneKind::symbol(*kind))),
        ]),
        FilterPredicate::Lane(lane) => map(vec![
            ("tag", tag_expr("filter-predicate")),
            ("kind", tag_expr("lane")),
            ("lane", Expr::String(lane.0.clone())),
        ]),
    }
}

fn predicate_from_expr(expr: &Expr) -> KernelResult<FilterPredicate> {
    let entries = expr_map(expr, "filter predicate")?;
    expect_tag(entries, "filter-predicate", "filter predicate")?;
    match symbol_name(lookup_required(entries, "kind")?, "filter predicate kind")? {
        "any" => Ok(FilterPredicate::Any),
        "kind" => Ok(FilterPredicate::Kind(
            lane_kind_from_wire(symbol_name(
                lookup_required(entries, "lane-kind")?,
                "lane kind",
            )?)
            .ok_or_else(|| Error::Eval("lane kind is invalid".to_owned()))?,
        )),
        "lane" => Ok(FilterPredicate::Lane(LaneId::new(expr_string(
            lookup_required(entries, "lane")?,
            "filter predicate lane",
        )?))),
        _ => Err(Error::Eval("filter predicate kind is invalid".to_owned())),
    }
}

fn op_to_expr(op: &FilterOp) -> Expr {
    let mut entries = vec![
        ("tag", tag_expr("filter-op")),
        ("kind", tag_expr(op.wire_label())),
    ];
    match op {
        FilterOp::Accept | FilterOp::Reject => {}
        FilterOp::Clone { copies } => entries.push(("copies", number_text(*copies))),
        FilterOp::Rewrite {
            lane,
            pitch_delta,
            velocity_delta,
        } => {
            entries.push((
                "lane",
                lane.as_ref()
                    .map(|lane| Expr::String(lane.0.clone()))
                    .unwrap_or(Expr::Nil),
            ));
            entries.push(("pitch-delta", number_text(*pitch_delta)));
            entries.push(("velocity-delta", number_text(*velocity_delta)));
        }
        FilterOp::Route { lane } => entries.push(("lane", Expr::String(lane.0.clone()))),
        FilterOp::Annotate { message } => entries.push(("message", Expr::String(message.clone()))),
        FilterOp::Quantize { grid_ticks } => entries.push(("grid-ticks", number_text(*grid_ticks))),
        FilterOp::Thin { keep_every } => entries.push(("keep-every", number_text(*keep_every))),
        FilterOp::Expand { copies, step_ticks } => {
            entries.push(("copies", number_text(*copies)));
            entries.push(("step-ticks", number_text(*step_ticks)));
        }
        FilterOp::Sidechain { lane, control } => {
            entries.push(("lane", Expr::String(lane.0.clone())));
            entries.push(("control", Expr::String(control.clone())));
        }
    }
    map(entries)
}

fn op_from_expr(expr: &Expr) -> KernelResult<FilterOp> {
    let entries = expr_map(expr, "filter op")?;
    expect_tag(entries, "filter-op", "filter op")?;
    match symbol_name(lookup_required(entries, "kind")?, "filter op kind")? {
        "accept" => Ok(FilterOp::Accept),
        "reject" => Ok(FilterOp::Reject),
        "clone" => Ok(FilterOp::Clone {
            copies: expr_u8(lookup_required(entries, "copies")?, "clone copies")?,
        }),
        "rewrite" => Ok(FilterOp::Rewrite {
            lane: match lookup_required(entries, "lane")? {
                Expr::Nil => None,
                expr => Some(LaneId::new(expr_string(expr, "rewrite lane")?)),
            },
            pitch_delta: expr_i16(lookup_required(entries, "pitch-delta")?, "pitch delta")?,
            velocity_delta: expr_i16(
                lookup_required(entries, "velocity-delta")?,
                "velocity delta",
            )?,
        }),
        "route" => Ok(FilterOp::Route {
            lane: LaneId::new(expr_string(
                lookup_required(entries, "lane")?,
                "route lane",
            )?),
        }),
        "annotate" => Ok(FilterOp::Annotate {
            message: expr_string(lookup_required(entries, "message")?, "annotation")?.to_owned(),
        }),
        "quantize" => Ok(FilterOp::Quantize {
            grid_ticks: expr_i64(lookup_required(entries, "grid-ticks")?, "grid ticks")?,
        }),
        "thin" => Ok(FilterOp::Thin {
            keep_every: expr_u32(lookup_required(entries, "keep-every")?, "keep every")?,
        }),
        "expand" => Ok(FilterOp::Expand {
            copies: expr_u8(lookup_required(entries, "copies")?, "expand copies")?,
            step_ticks: expr_i64(lookup_required(entries, "step-ticks")?, "step ticks")?,
        }),
        "sidechain" => Ok(FilterOp::Sidechain {
            lane: LaneId::new(expr_string(
                lookup_required(entries, "lane")?,
                "sidechain lane",
            )?),
            control: expr_string(lookup_required(entries, "control")?, "sidechain control")?
                .to_owned(),
        }),
        _ => Err(Error::Eval("filter op kind is invalid".to_owned())),
    }
}

fn encode_shape(shape: &FilterShape) -> String {
    format!(
        "#(FilterShape kinds=[{}])",
        shape
            .kinds()
            .map(|kind| kind.wire_label())
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn decode_shape(node: &sim_codec::DomainForm) -> Result<FilterShape, MusicShapeError> {
    if node.name != "FilterShape" {
        return Err(MusicShapeError::InvalidMusic);
    }
    FilterShape::new(
        field_list(node, "kinds")?
            .iter()
            .map(value_lane_kind)
            .collect::<Result<Vec<_>, _>>()?,
    )
    .map_err(|_| MusicShapeError::InvalidMusic)
}

fn encode_body(body: &FilterBody) -> String {
    match body {
        FilterBody::Rule(rules) => format!(
            "#(FilterBody kind=rule rules=[{}])",
            rules.iter().map(encode_rule).collect::<Vec<_>>().join(",")
        ),
        FilterBody::Callable(callable) => {
            format!(
                "#(FilterBody kind=callable name={})",
                encode_string(&callable.name)
            )
        }
    }
}

fn decode_body(node: &sim_codec::DomainForm) -> Result<FilterBody, MusicShapeError> {
    if node.name != "FilterBody" {
        return Err(MusicShapeError::InvalidMusic);
    }
    match field_atom(node, "kind")?.as_str() {
        "rule" => Ok(FilterBody::Rule(
            field_list(node, "rules")?
                .iter()
                .map(|value| decode_rule(value_form(value)?))
                .collect::<Result<Vec<_>, _>>()?,
        )),
        "callable" => Ok(FilterBody::Callable(CallableFilterRef::new(
            string_or_atom(field(node, "name")?)?,
        ))),
        _ => Err(MusicShapeError::InvalidMusic),
    }
}

fn encode_rule(rule: &FilterRule) -> String {
    format!(
        "#(FilterRule when={} op={})",
        encode_predicate(&rule.when),
        encode_op(&rule.op)
    )
}

fn decode_rule(node: &sim_codec::DomainForm) -> Result<FilterRule, MusicShapeError> {
    if node.name != "FilterRule" {
        return Err(MusicShapeError::InvalidMusic);
    }
    Ok(FilterRule::new(
        decode_predicate(value_form(field(node, "when")?)?)?,
        decode_op(value_form(field(node, "op")?)?)?,
    ))
}

fn encode_predicate(predicate: &FilterPredicate) -> String {
    match predicate {
        FilterPredicate::Any => "#(FilterPredicate kind=any)".to_owned(),
        FilterPredicate::Kind(kind) => {
            format!(
                "#(FilterPredicate kind=kind lane_kind={})",
                kind.wire_label()
            )
        }
        FilterPredicate::Lane(lane) => {
            format!(
                "#(FilterPredicate kind=lane lane={})",
                encode_string(&lane.0)
            )
        }
    }
}

fn decode_predicate(node: &sim_codec::DomainForm) -> Result<FilterPredicate, MusicShapeError> {
    if node.name != "FilterPredicate" {
        return Err(MusicShapeError::InvalidMusic);
    }
    match field_atom(node, "kind")?.as_str() {
        "any" => Ok(FilterPredicate::Any),
        "kind" => Ok(FilterPredicate::Kind(
            lane_kind_from_wire(&field_atom(node, "lane_kind")?)
                .ok_or(MusicShapeError::InvalidMusic)?,
        )),
        "lane" => Ok(FilterPredicate::Lane(LaneId::new(string_or_atom(field(
            node, "lane",
        )?)?))),
        _ => Err(MusicShapeError::InvalidMusic),
    }
}

fn encode_op(op: &FilterOp) -> String {
    match op {
        FilterOp::Accept => "#(FilterOp kind=accept)".to_owned(),
        FilterOp::Reject => "#(FilterOp kind=reject)".to_owned(),
        FilterOp::Clone { copies } => format!("#(FilterOp kind=clone copies={copies})"),
        FilterOp::Rewrite {
            lane,
            pitch_delta,
            velocity_delta,
        } => format!(
            "#(FilterOp kind=rewrite lane={} pitch_delta={} velocity_delta={})",
            lane.as_ref()
                .map(|lane| encode_string(&lane.0))
                .unwrap_or_else(|| "none".to_owned()),
            pitch_delta,
            velocity_delta
        ),
        FilterOp::Route { lane } => {
            format!("#(FilterOp kind=route lane={})", encode_string(&lane.0))
        }
        FilterOp::Annotate { message } => {
            format!(
                "#(FilterOp kind=annotate message={})",
                encode_string(message)
            )
        }
        FilterOp::Quantize { grid_ticks } => {
            format!("#(FilterOp kind=quantize grid_ticks={grid_ticks})")
        }
        FilterOp::Thin { keep_every } => {
            format!("#(FilterOp kind=thin keep_every={keep_every})")
        }
        FilterOp::Expand { copies, step_ticks } => {
            format!("#(FilterOp kind=expand copies={copies} step_ticks={step_ticks})")
        }
        FilterOp::Sidechain { lane, control } => format!(
            "#(FilterOp kind=sidechain lane={} control={})",
            encode_string(&lane.0),
            encode_string(control)
        ),
    }
}

fn decode_op(node: &sim_codec::DomainForm) -> Result<FilterOp, MusicShapeError> {
    if node.name != "FilterOp" {
        return Err(MusicShapeError::InvalidMusic);
    }
    match field_atom(node, "kind")?.as_str() {
        "accept" => Ok(FilterOp::Accept),
        "reject" => Ok(FilterOp::Reject),
        "clone" => Ok(FilterOp::Clone {
            copies: parse_u8(&field_atom(node, "copies")?)?,
        }),
        "rewrite" => Ok(FilterOp::Rewrite {
            lane: match string_or_atom(field(node, "lane")?)?.as_str() {
                "none" => None,
                value => Some(LaneId::new(value.to_owned())),
            },
            pitch_delta: parse_i16(&field_atom(node, "pitch_delta")?)?,
            velocity_delta: parse_i16(&field_atom(node, "velocity_delta")?)?,
        }),
        "route" => Ok(FilterOp::Route {
            lane: LaneId::new(string_or_atom(field(node, "lane")?)?),
        }),
        "annotate" => Ok(FilterOp::Annotate {
            message: string_or_atom(field(node, "message")?)?,
        }),
        "quantize" => Ok(FilterOp::Quantize {
            grid_ticks: parse_i64(&field_atom(node, "grid_ticks")?)?,
        }),
        "thin" => Ok(FilterOp::Thin {
            keep_every: parse_u32(&field_atom(node, "keep_every")?)?,
        }),
        "expand" => Ok(FilterOp::Expand {
            copies: parse_u8(&field_atom(node, "copies")?)?,
            step_ticks: parse_i64(&field_atom(node, "step_ticks")?)?,
        }),
        "sidechain" => Ok(FilterOp::Sidechain {
            lane: LaneId::new(string_or_atom(field(node, "lane")?)?),
            control: string_or_atom(field(node, "control")?)?,
        }),
        _ => Err(MusicShapeError::InvalidMusic),
    }
}

fn decode_capabilities(values: &[DomainValue]) -> Result<FilterCapabilitySet, MusicShapeError> {
    Ok(FilterCapabilitySet::new(
        values
            .iter()
            .map(|value| {
                FilterCapability::from_wire_label(&value_string_or_atom(value)?)
                    .ok_or(MusicShapeError::InvalidMusic)
            })
            .collect::<Result<Vec<_>, _>>()?,
    ))
}

fn encode_trace(trace: TracePolicy) -> &'static str {
    match trace {
        TracePolicy::Off => "off",
        TracePolicy::Diagnostics => "diagnostics",
        TracePolicy::Full => "full",
    }
}

fn decode_trace(value: &str) -> Result<TracePolicy, MusicShapeError> {
    match value {
        "off" => Ok(TracePolicy::Off),
        "diagnostics" => Ok(TracePolicy::Diagnostics),
        "full" => Ok(TracePolicy::Full),
        _ => Err(MusicShapeError::InvalidMusic),
    }
}

fn value_form(value: &DomainValue) -> Result<&sim_codec::DomainForm, MusicShapeError> {
    match value {
        DomainValue::Form(form) => Ok(form),
        _ => Err(MusicShapeError::InvalidMusic),
    }
}

fn value_lane_kind(value: &DomainValue) -> Result<LaneKind, MusicShapeError> {
    lane_kind_from_wire(&value_string_or_atom(value)?).ok_or(MusicShapeError::InvalidMusic)
}

fn string_or_atom(value: &DomainValue) -> Result<String, MusicShapeError> {
    match value {
        DomainValue::String(value) | DomainValue::Atom(value) => Ok(value.clone()),
        _ => Err(MusicShapeError::InvalidMusic),
    }
}

fn value_string_or_atom(value: &DomainValue) -> Result<String, MusicShapeError> {
    string_or_atom(value)
}

fn parse_u8(value: &str) -> Result<u8, MusicShapeError> {
    value.parse().map_err(|_| MusicShapeError::InvalidMusic)
}

fn parse_i16(value: &str) -> Result<i16, MusicShapeError> {
    value.parse().map_err(|_| MusicShapeError::InvalidMusic)
}

fn parse_u32(value: &str) -> Result<u32, MusicShapeError> {
    value.parse().map_err(|_| MusicShapeError::InvalidMusic)
}

fn parse_i64(value: &str) -> Result<i64, MusicShapeError> {
    value.parse().map_err(|_| MusicShapeError::InvalidMusic)
}

fn map(entries: Vec<(&'static str, Expr)>) -> Expr {
    Expr::Map(
        entries
            .into_iter()
            .map(|(key, value)| (Expr::Symbol(Symbol::new(key)), value))
            .collect(),
    )
}

fn tag_expr(name: &'static str) -> Expr {
    Expr::Symbol(Symbol::qualified(NS, name))
}

fn expr_map<'a>(expr: &'a Expr, context: &str) -> KernelResult<&'a [(Expr, Expr)]> {
    match expr {
        Expr::Map(entries) => Ok(entries),
        _ => Err(Error::Eval(format!("{context} must be a map"))),
    }
}

fn lookup_required<'a>(entries: &'a [(Expr, Expr)], name: &str) -> KernelResult<&'a Expr> {
    entries
        .iter()
        .find_map(|(key, value)| match key {
            Expr::Symbol(symbol) if symbol.namespace.is_none() && symbol.name.as_ref() == name => {
                Some(value)
            }
            _ => None,
        })
        .ok_or_else(|| Error::Eval(format!("missing {name} field")))
}

fn expect_tag(entries: &[(Expr, Expr)], expected: &str, context: &str) -> KernelResult<()> {
    if symbol_name(lookup_required(entries, "tag")?, context)? == expected {
        Ok(())
    } else {
        Err(Error::Eval(format!("{context} tag is invalid")))
    }
}

fn expr_vector<'a>(expr: &'a Expr, context: &str) -> KernelResult<&'a [Expr]> {
    match expr {
        Expr::Vector(items) => Ok(items),
        _ => Err(Error::Eval(format!("{context} must be a vector"))),
    }
}

fn expr_string<'a>(expr: &'a Expr, context: &str) -> KernelResult<&'a str> {
    match expr {
        Expr::String(value) => Ok(value),
        _ => Err(Error::Eval(format!("{context} must be text"))),
    }
}

fn symbol_name<'a>(expr: &'a Expr, context: &str) -> KernelResult<&'a str> {
    match expr {
        Expr::Symbol(symbol) => Ok(symbol.name.as_ref()),
        _ => Err(Error::Eval(format!("{context} must be a symbol"))),
    }
}

fn number_text(value: impl ToString) -> Expr {
    Expr::String(value.to_string())
}

fn expr_u8(expr: &Expr, context: &str) -> KernelResult<u8> {
    expr_string(expr, context)?
        .parse()
        .map_err(|err| Error::Eval(format!("invalid {context}: {err}")))
}

fn expr_i16(expr: &Expr, context: &str) -> KernelResult<i16> {
    expr_string(expr, context)?
        .parse()
        .map_err(|err| Error::Eval(format!("invalid {context}: {err}")))
}

fn expr_u32(expr: &Expr, context: &str) -> KernelResult<u32> {
    expr_string(expr, context)?
        .parse()
        .map_err(|err| Error::Eval(format!("invalid {context}: {err}")))
}

fn expr_i64(expr: &Expr, context: &str) -> KernelResult<i64> {
    expr_string(expr, context)?
        .parse()
        .map_err(|err| Error::Eval(format!("invalid {context}: {err}")))
}
