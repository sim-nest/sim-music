use sim_kernel::{Expr, Symbol};
use sim_lib_pitch_core::PitchClass;
use sim_lib_pitch_scale::{Mode, Scale};
use sim_lib_pitch_set::PitchClassMask;

use crate::HarmonyError;

const NS: &str = "harmony";

pub(crate) fn scale_to_expr(scale: Scale) -> Expr {
    tagged(
        "scale",
        vec![
            field("tonic", scalar(scale.tonic.value())),
            field("mode", variant(scale.mode.name())),
        ],
    )
}

pub(crate) fn scale_from_expr(expr: &Expr) -> Result<Scale, HarmonyError> {
    require_tag(expr, "scale")?;
    Ok(Scale::new(
        pitch_class(required(expr, "tonic")?, "scale.tonic")?,
        mode(symbol_name(required(expr, "mode")?, "scale.mode")?)?,
    ))
}

fn mode(name: &str) -> Result<Mode, HarmonyError> {
    match name {
        "major" => Ok(Mode::Major),
        "minor-natural" => Ok(Mode::MinorNatural),
        "minor-harmonic" => Ok(Mode::MinorHarmonic),
        "minor-melodic" => Ok(Mode::MinorMelodic),
        "dorian" => Ok(Mode::Dorian),
        "phrygian" => Ok(Mode::Phrygian),
        "lydian" => Ok(Mode::Lydian),
        "mixolydian" => Ok(Mode::Mixolydian),
        "aeolian" => Ok(Mode::Aeolian),
        "locrian" => Ok(Mode::Locrian),
        "whole-tone" => Ok(Mode::WholeTone),
        "diminished" => Ok(Mode::Diminished),
        "chromatic" => Ok(Mode::Chromatic),
        other => Err(invalid(format!("unknown scale mode {other}"))),
    }
}

pub(crate) fn tagged(kind: &str, fields: Vec<(Expr, Expr)>) -> Expr {
    let mut entries = vec![field("tag", variant(kind))];
    entries.extend(fields);
    Expr::Map(entries)
}

pub(crate) fn field(name: &str, value: Expr) -> (Expr, Expr) {
    (Expr::Symbol(Symbol::qualified(NS, name)), value)
}

pub(crate) fn string(value: &str) -> Expr {
    Expr::String(value.to_owned())
}

pub(crate) fn scalar(value: impl ToString) -> Expr {
    string(&value.to_string())
}

pub(crate) fn variant(value: &str) -> Expr {
    Expr::Symbol(Symbol::qualified(NS, value))
}

pub(crate) fn vector(values: Vec<Expr>) -> Expr {
    Expr::Vector(values)
}

pub(crate) fn require_tag<'a>(
    expr: &'a Expr,
    expected: &str,
) -> Result<&'a [(Expr, Expr)], HarmonyError> {
    let map = map(expr)?;
    let actual = symbol_name(
        lookup(map, "tag").ok_or_else(|| invalid("missing tag"))?,
        "tag",
    )?;
    if actual != expected {
        return Err(invalid(format!("expected tag {expected}, found {actual}")));
    }
    Ok(map)
}

pub(crate) fn tag(expr: &Expr) -> Result<&str, HarmonyError> {
    let map = map(expr)?;
    symbol_name(
        lookup(map, "tag").ok_or_else(|| invalid("missing tag"))?,
        "tag",
    )
}

pub(crate) fn required<'a>(expr: &'a Expr, name: &str) -> Result<&'a Expr, HarmonyError> {
    lookup(map(expr)?, name).ok_or_else(|| invalid(format!("missing field {name}")))
}

fn map(expr: &Expr) -> Result<&[(Expr, Expr)], HarmonyError> {
    match expr {
        Expr::Map(entries) => Ok(entries),
        _ => Err(invalid("expected map expression")),
    }
}

fn lookup<'a>(entries: &'a [(Expr, Expr)], name: &str) -> Option<&'a Expr> {
    entries.iter().find_map(|(key, value)| match key {
        Expr::Symbol(symbol) if symbol.name.as_ref() == name => Some(value),
        _ => None,
    })
}

pub(crate) fn text<'a>(expr: &'a Expr, context: &str) -> Result<&'a str, HarmonyError> {
    match expr {
        Expr::String(value) => Ok(value),
        _ => Err(invalid(format!("{context} must be a string"))),
    }
}

pub(crate) fn symbol_name<'a>(expr: &'a Expr, context: &str) -> Result<&'a str, HarmonyError> {
    match expr {
        Expr::Symbol(value) => Ok(value.name.as_ref()),
        _ => Err(invalid(format!("{context} must be a symbol"))),
    }
}

pub(crate) fn sequence<'a>(expr: &'a Expr, context: &str) -> Result<&'a [Expr], HarmonyError> {
    match expr {
        Expr::List(values) | Expr::Vector(values) => Ok(values),
        _ => Err(invalid(format!("{context} must be a list or vector"))),
    }
}

pub(crate) fn parse<T>(expr: &Expr, context: &str) -> Result<T, HarmonyError>
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    text(expr, context)?
        .parse()
        .map_err(|error| invalid(format!("{context}: {error}")))
}

pub(crate) fn scalars<T>(expr: &Expr, context: &str) -> Result<Vec<T>, HarmonyError>
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    sequence(expr, context)?
        .iter()
        .map(|value| parse(value, context))
        .collect()
}

pub(crate) fn strings(expr: &Expr, context: &str) -> Result<Vec<String>, HarmonyError> {
    sequence(expr, context)?
        .iter()
        .map(|value| Ok(text(value, context)?.to_owned()))
        .collect()
}

pub(crate) fn pitch_class(expr: &Expr, context: &str) -> Result<PitchClass, HarmonyError> {
    PitchClass::new(parse(expr, context)?).map_err(|error| invalid(error.to_string()))
}

pub(crate) fn pitch_classes(expr: &Expr) -> Result<Vec<PitchClass>, HarmonyError> {
    sequence(expr, "pitch classes")?
        .iter()
        .map(|expr| pitch_class(expr, "pitch class"))
        .collect()
}

pub(crate) fn mask(expr: &Expr, context: &str) -> Result<PitchClassMask, HarmonyError> {
    PitchClassMask::new(parse(expr, context)?).map_err(|error| invalid(error.to_string()))
}

pub(crate) fn invalid(reason: impl Into<String>) -> HarmonyError {
    HarmonyError::Expression(reason.into())
}
