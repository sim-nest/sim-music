use sim_kernel::{Error, Expr, NumberLiteral, Result, Symbol};

pub(crate) const NS: &str = "daw-session";

pub(crate) fn field(name: &'static str) -> Expr {
    sim_value::build::qsym(NS, name)
}

pub(crate) fn tag(name: &'static str) -> Expr {
    Expr::Symbol(Symbol::qualified(NS, name))
}

pub(crate) fn number_u16(value: u16) -> Expr {
    number("i64", value.to_string())
}

pub(crate) fn number_u32(value: u32) -> Expr {
    number("i64", value.to_string())
}

pub(crate) fn number_u64(value: u64) -> Expr {
    number("i64", value.to_string())
}

pub(crate) fn number_f32(value: f32) -> Expr {
    number("f64", value.to_string())
}

pub(crate) fn number_f64(value: f64) -> Expr {
    number("f64", value.to_string())
}

pub(crate) fn number(domain: &str, canonical: String) -> Expr {
    Expr::Number(NumberLiteral {
        domain: Symbol::qualified("numbers", domain),
        canonical,
    })
}

pub(crate) fn expr_map<'a>(expr: &'a Expr, context: &str) -> Result<&'a [(Expr, Expr)]> {
    match expr {
        Expr::Map(entries) => Ok(entries),
        other => Err(Error::Eval(format!(
            "expected {context} map, found {}",
            expr_kind(other)
        ))),
    }
}

pub(crate) fn expr_vector<'a>(expr: &'a Expr, context: &str) -> Result<&'a [Expr]> {
    match expr {
        Expr::Vector(items) => Ok(items),
        other => Err(Error::Eval(format!(
            "expected {context} vector, found {}",
            expr_kind(other)
        ))),
    }
}

pub(crate) fn expr_string<'a>(expr: &'a Expr, context: &str) -> Result<&'a str> {
    match expr {
        Expr::String(text) => Ok(text),
        other => Err(Error::Eval(format!(
            "expected {context} string, found {}",
            expr_kind(other)
        ))),
    }
}

pub(crate) fn expr_bool(expr: &Expr, context: &str) -> Result<bool> {
    match expr {
        Expr::Bool(value) => Ok(*value),
        other => Err(Error::Eval(format!(
            "expected {context} bool, found {}",
            expr_kind(other)
        ))),
    }
}

pub(crate) fn expr_symbol(expr: &Expr, context: &str) -> Result<Symbol> {
    match expr {
        Expr::Symbol(symbol) => Ok(symbol.clone()),
        other => Err(Error::Eval(format!(
            "expected {context} symbol, found {}",
            expr_kind(other)
        ))),
    }
}

pub(crate) fn expr_u16(expr: &Expr, context: &str) -> Result<u16> {
    parse_number(expr, context)
}

pub(crate) fn expr_u32(expr: &Expr, context: &str) -> Result<u32> {
    parse_number(expr, context)
}

pub(crate) fn expr_u64(expr: &Expr, context: &str) -> Result<u64> {
    parse_number(expr, context)
}

pub(crate) fn expr_f32(expr: &Expr, context: &str) -> Result<f32> {
    let value = number_text(expr, context)?.parse::<f32>().map_err(|_| {
        Error::Eval(format!(
            "expected {context} f32 number, found {}",
            number_text(expr, context).unwrap_or("?")
        ))
    })?;
    if value.is_finite() {
        Ok(value)
    } else {
        Err(Error::Eval(format!("expected {context} finite f32 number")))
    }
}

pub(crate) fn expr_f64(expr: &Expr, context: &str) -> Result<f64> {
    let value = number_text(expr, context)?.parse::<f64>().map_err(|_| {
        Error::Eval(format!(
            "expected {context} f64 number, found {}",
            number_text(expr, context).unwrap_or("?")
        ))
    })?;
    if value.is_finite() {
        Ok(value)
    } else {
        Err(Error::Eval(format!("expected {context} finite f64 number")))
    }
}

pub(crate) fn lookup_required<'a>(map: &'a [(Expr, Expr)], name: &str) -> Result<&'a Expr> {
    lookup(map, name).ok_or_else(|| Error::Eval(format!("DAW field is missing: {name}")))
}

pub(crate) fn lookup<'a>(map: &'a [(Expr, Expr)], name: &str) -> Option<&'a Expr> {
    map.iter().find_map(|(key, value)| match key {
        Expr::Symbol(symbol) if is_symbol(symbol, NS, name) => Some(value),
        _ => None,
    })
}

pub(crate) fn expect_tag(map: &[(Expr, Expr)], name: &'static str, context: &str) -> Result<()> {
    match lookup(map, "tag") {
        Some(Expr::Symbol(symbol)) if is_symbol(symbol, NS, name) => Ok(()),
        Some(_) => Err(Error::Eval(format!("{context} tag is invalid"))),
        None => Err(Error::Eval(format!("{context} tag is missing"))),
    }
}

pub(crate) fn is_symbol(symbol: &Symbol, namespace: &str, name: &str) -> bool {
    symbol.namespace.as_deref() == Some(namespace) && symbol.name.as_ref() == name
}

fn parse_number<T>(expr: &Expr, context: &str) -> Result<T>
where
    T: std::str::FromStr,
{
    let text = number_text(expr, context)?;
    text.parse::<T>()
        .map_err(|_| Error::Eval(format!("expected {context} number, found {text}")))
}

fn number_text<'a>(expr: &'a Expr, context: &str) -> Result<&'a str> {
    match expr {
        Expr::Number(number) => Ok(number.canonical.as_str()),
        Expr::String(text) => Ok(text),
        other => Err(Error::Eval(format!(
            "expected {context} number, found {}",
            expr_kind(other)
        ))),
    }
}

use sim_value::kind::expr_kind;
