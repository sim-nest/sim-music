//! Text adapter for pitch-independent serial series.

use sim_codec::{DomainForm, DomainValue, parse_domain_form};
use sim_lib_serial_core::{
    AggregateRule, AggregateRuleKind, AlphabetId, FiniteAlphabet, ProjectedClassSpec, ProjectionId,
    SerialAlphabet, Series, SeriesError,
};

use super::analysis::{ensure_form, field, field_atom, field_list};
use super::{MusicShapeError, encode_string};

/// Concrete string-symbol series used by the Lisp adapter.
pub type SymbolSeries = Series<FiniteAlphabet<String>>;

/// Decodes and semantically validates a `#(SerialSeries ...)` value.
pub fn decode_serial_series(value: &str) -> Result<SymbolSeries, MusicShapeError> {
    let node = parse_domain_form(value)?;
    ensure_form(&node, "SerialSeries")?;
    let id = AlphabetId::try_new(text_field(&node, "alphabet_id")?).map_err(SeriesError::from)?;
    let symbols = atom_list(&node, "symbols")?;
    let alphabet = FiniteAlphabet::try_new(id, symbols).map_err(SeriesError::from)?;
    let rule = decode_rule(field(&node, "rule")?.as_form()?, &alphabet)?;
    let order = atom_list(&node, "order")?;
    Ok(Series::try_new(alphabet, rule, order)?)
}

/// Encodes a string-symbol series into its canonical `#(SerialSeries ...)` value.
pub fn encode_serial_series(series: &SymbolSeries) -> Result<String, MusicShapeError> {
    let alphabet = series.alphabet();
    Ok(format!(
        "#(SerialSeries alphabet_id={} symbols=[{}] rule={} order=[{}])",
        encode_string(alphabet.id().as_str()),
        alphabet
            .symbols()
            .iter()
            .map(|symbol| encode_string(symbol))
            .collect::<Vec<_>>()
            .join(","),
        encode_rule(series)?,
        series
            .order()
            .iter()
            .map(|symbol| encode_string(symbol))
            .collect::<Vec<_>>()
            .join(","),
    ))
}

fn decode_rule(
    node: &DomainForm,
    alphabet: &FiniteAlphabet<String>,
) -> Result<AggregateRule, MusicShapeError> {
    ensure_form(node, "AggregateRule")?;
    let kind = field_atom(node, "kind")?;
    match kind.as_str() {
        "ExhaustiveExactlyOnce" => Ok(AggregateRule::exhaustive_exactly_once()),
        "NoRepeat" => Ok(AggregateRule::no_repeat()),
        "FreeOrder" => Ok(AggregateRule::free_order()),
        "DeclaredMultiplicity" => {
            let entries = count_entries(node, "counts")?;
            AggregateRule::declared_multiplicity(alphabet, entries)
                .map_err(SeriesError::from)
                .map_err(MusicShapeError::from)
        }
        "DeclaredOmissions" => {
            let omissions = atom_list(node, "omissions")?;
            AggregateRule::declared_omissions(alphabet, omissions)
                .map_err(SeriesError::from)
                .map_err(MusicShapeError::from)
        }
        "ProjectedAggregate" => {
            let classes = field_list(node, "classes")?
                .iter()
                .map(|value| decode_projected_class(value.as_form()?))
                .collect::<Result<Vec<_>, MusicShapeError>>()?;
            AggregateRule::projected_aggregate(alphabet, classes)
                .map_err(SeriesError::from)
                .map_err(MusicShapeError::from)
        }
        _ => Err(MusicShapeError::InvalidMusic),
    }
}

fn encode_rule(series: &SymbolSeries) -> Result<String, MusicShapeError> {
    let rule = series.rule();
    let alphabet = series.alphabet();
    let encoded = match rule.kind() {
        AggregateRuleKind::ExhaustiveExactlyOnce
        | AggregateRuleKind::NoRepeat
        | AggregateRuleKind::FreeOrder => {
            format!("#(AggregateRule kind={})", rule_kind_name(rule.kind()))
        }
        AggregateRuleKind::DeclaredMultiplicity => {
            let counts = rule
                .declared_counts(alphabet)
                .map_err(SeriesError::from)?
                .ok_or(MusicShapeError::InvalidMusic)?;
            format!(
                "#(AggregateRule kind=DeclaredMultiplicity counts=[{}])",
                counts
                    .iter()
                    .map(|(symbol, count)| format!(
                        "#(SerialCount symbol={} multiplicity={count})",
                        encode_string(symbol)
                    ))
                    .collect::<Vec<_>>()
                    .join(",")
            )
        }
        AggregateRuleKind::DeclaredOmissions => {
            let counts = rule
                .declared_counts(alphabet)
                .map_err(SeriesError::from)?
                .ok_or(MusicShapeError::InvalidMusic)?;
            format!(
                "#(AggregateRule kind=DeclaredOmissions omissions=[{}])",
                counts
                    .iter()
                    .filter(|(_, count)| *count == 0)
                    .map(|(symbol, _)| encode_string(symbol))
                    .collect::<Vec<_>>()
                    .join(",")
            )
        }
        AggregateRuleKind::ProjectedAggregate => {
            let classes = rule
                .projected_classes(alphabet)
                .map_err(SeriesError::from)?
                .ok_or(MusicShapeError::InvalidMusic)?;
            format!(
                "#(AggregateRule kind=ProjectedAggregate classes=[{}])",
                classes
                    .iter()
                    .map(|class| format!(
                        "#(ProjectedClass id={} symbols=[{}] multiplicity={})",
                        encode_string(class.id.as_str()),
                        class
                            .symbols
                            .iter()
                            .map(|symbol| encode_string(symbol))
                            .collect::<Vec<_>>()
                            .join(","),
                        class.multiplicity,
                    ))
                    .collect::<Vec<_>>()
                    .join(",")
            )
        }
    };
    Ok(encoded)
}

fn atom_list(node: &DomainForm, name: &str) -> Result<Vec<String>, MusicShapeError> {
    field_list(node, name)?
        .iter()
        .map(|value| match value {
            DomainValue::Atom(value) | DomainValue::String(value) => Ok(value.clone()),
            _ => Err(MusicShapeError::InvalidMusic),
        })
        .collect()
}

fn count_entries(node: &DomainForm, name: &str) -> Result<Vec<(String, usize)>, MusicShapeError> {
    field_list(node, name)?
        .iter()
        .map(|value| {
            let form = value.as_form()?;
            ensure_form(form, "SerialCount")?;
            Ok((
                text_field(form, "symbol")?,
                parse_usize(&field_atom(form, "multiplicity")?)?,
            ))
        })
        .collect()
}

fn decode_projected_class(
    node: &DomainForm,
) -> Result<ProjectedClassSpec<String>, MusicShapeError> {
    ensure_form(node, "ProjectedClass")?;
    let id = ProjectionId::try_new(text_field(node, "id")?).map_err(SeriesError::from)?;
    Ok(ProjectedClassSpec::new(
        id,
        atom_list(node, "symbols")?,
        parse_usize(&field_atom(node, "multiplicity")?)?,
    ))
}

fn parse_usize(value: &str) -> Result<usize, MusicShapeError> {
    value
        .parse::<usize>()
        .map_err(|_| MusicShapeError::InvalidMusic)
}

fn text_field(node: &DomainForm, name: &str) -> Result<String, MusicShapeError> {
    Ok(node.field_atom_or_string(name)?.to_owned())
}

fn rule_kind_name(kind: AggregateRuleKind) -> &'static str {
    match kind {
        AggregateRuleKind::ExhaustiveExactlyOnce => "ExhaustiveExactlyOnce",
        AggregateRuleKind::NoRepeat => "NoRepeat",
        AggregateRuleKind::DeclaredMultiplicity => "DeclaredMultiplicity",
        AggregateRuleKind::DeclaredOmissions => "DeclaredOmissions",
        AggregateRuleKind::ProjectedAggregate => "ProjectedAggregate",
        AggregateRuleKind::FreeOrder => "FreeOrder",
    }
}
