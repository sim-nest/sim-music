use std::sync::Arc;

use sim_kernel::{
    AbiVersion, Args, Callable, ClassRef, Cx, Error, Export, Lib, LibManifest, LibTarget, Linker,
    Object, ObjectCompat, RawArgs, Result, Symbol, Value, Version,
};
use sim_lib_stream_core::{StreamItem, StreamValue};
use sim_lib_stream_prelude::{StreamHandle, install_stream_prelude_lib};

use crate::{
    StreamBridgeLiftMidiOptions, StreamBridgeRenderOptions, lift_pcm_items_to_midi,
    render_midi_items_to_pcm, stream_bridge_symbol,
};

const STREAM_BRIDGE_LIB_ID: &str = "stream-bridge";

/// Loadable library exposing the `stream/bridge` function to a runtime.
pub struct StreamBridgeLib;

impl Lib for StreamBridgeLib {
    fn manifest(&self) -> LibManifest {
        LibManifest {
            id: manifest_name(),
            version: Version(env!("CARGO_PKG_VERSION").to_owned()),
            abi: AbiVersion { major: 0, minor: 1 },
            target: LibTarget::HostRegistered,
            requires: Vec::new(),
            capabilities: Vec::new(),
            exports: vec![Export::Function {
                symbol: stream_bridge_symbol(),
                function_id: None,
            }],
        }
    }

    fn load(&self, cx: &mut sim_kernel::LoadCx, linker: &mut Linker<'_>) -> Result<()> {
        linker.function_value(
            stream_bridge_symbol(),
            cx.factory().opaque(Arc::new(StreamBridgeFunction))?,
        )?;
        Ok(())
    }
}

/// Installs the stream prelude and registers [`StreamBridgeLib`] into `cx`.
pub fn install_stream_bridge_lib(cx: &mut Cx) -> Result<()> {
    install_stream_prelude_lib(cx)?;
    sim_lib_core::install_once(cx, &StreamBridgeLib).map(|_| ())
}

/// Returns the manifest id symbol for [`StreamBridgeLib`].
pub fn manifest_name() -> Symbol {
    Symbol::new(STREAM_BRIDGE_LIB_ID)
}

struct StreamBridgeFunction;

impl Object for StreamBridgeFunction {
    fn display(&self, _cx: &mut Cx) -> Result<String> {
        Ok("#<function stream/bridge>".to_owned())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl ObjectCompat for StreamBridgeFunction {
    fn class(&self, cx: &mut Cx) -> Result<ClassRef> {
        cx.factory().class_stub(
            sim_kernel::CORE_FUNCTION_CLASS_ID,
            Symbol::qualified("core", "Function"),
        )
    }

    fn as_callable(&self) -> Option<&dyn Callable> {
        Some(self)
    }
}

impl Callable for StreamBridgeFunction {
    fn call(&self, cx: &mut Cx, args: Args) -> Result<Value> {
        let exprs = args
            .into_vec()
            .into_iter()
            .map(|value| value.object().as_expr(cx))
            .collect::<Result<Vec<_>>>()?;
        bridge_call(cx, &exprs)
    }

    fn call_exprs(&self, cx: &mut Cx, args: RawArgs) -> Result<Value> {
        bridge_call(cx, args.exprs())
    }
}

fn bridge_call(cx: &mut Cx, args: &[sim_kernel::Expr]) -> Result<Value> {
    let [stream, options @ ..] = args else {
        return Err(Error::Eval(
            "stream/bridge expects a stream and option pairs".to_owned(),
        ));
    };
    if !options.len().is_multiple_of(2) {
        return Err(Error::Eval(
            "stream/bridge options must be key/value pairs".to_owned(),
        ));
    }
    let mut op = None;
    let mut render = StreamBridgeRenderOptions::default();
    let mut lift = StreamBridgeLiftMidiOptions::default();
    for pair in options.chunks(2) {
        let key = keyword(&pair[0])?;
        match key.as_str() {
            "op" => op = Some(symbolish(&pair[1])?),
            "rate" | "sample-rate" => {
                let value = u32_option(&pair[1], "sample-rate")?;
                render.sample_rate = value;
                lift.sample_rate = value;
            }
            "channels" => render.channels = u8_option(&pair[1], "channels")?,
            "chunk-frames" => render.chunk_frames = usize_value(&pair[1])?,
            "min-confidence" => lift.min_confidence = f64_value(&pair[1])?,
            "window" => lift.window_size = usize_value(&pair[1])?,
            "hop" => lift.hop_size = usize_value(&pair[1])?,
            "tpq" => lift.tpq = u16_option(&pair[1], "tpq")?,
            "max-events" => lift.max_events_per_packet = usize_value(&pair[1])?,
            other => {
                return Err(Error::Eval(format!(
                    "unknown stream/bridge option :{other}"
                )));
            }
        }
    }
    let value = cx.eval_expr(unquote(stream))?;
    let items = items_from_value(value)?;
    let output = match op.as_deref() {
        Some("render") => render_midi_items_to_pcm(items, render)?,
        Some("lift-midi") => lift_pcm_items_to_midi(items, lift)?,
        Some(other) => return Err(Error::Eval(format!("unknown stream/bridge op {other}"))),
        None => {
            return Err(Error::Eval(
                "stream/bridge requires :op render or :op lift-midi".to_owned(),
            ));
        }
    };
    let stream = Arc::new(output.stream);
    let handle = StreamHandle::source(stream.metadata().clone(), stream);
    cx.factory().opaque(Arc::new(handle))
}

fn items_from_value(value: Value) -> Result<Vec<StreamItem>> {
    if let Some(handle) = value.object().downcast_ref::<StreamHandle>() {
        return take_handle_items(handle);
    }
    if let Some(stream) = value.object().downcast_ref::<StreamValue>() {
        return take_stream_items(stream);
    }
    Err(Error::TypeMismatch {
        expected: "stream handle",
        found: "non-stream",
    })
}

fn take_handle_items(handle: &StreamHandle) -> Result<Vec<StreamItem>> {
    let mut out = Vec::new();
    while let Some(item) = handle.next_packet()? {
        out.push(item);
    }
    Ok(out)
}

fn take_stream_items(stream: &StreamValue) -> Result<Vec<StreamItem>> {
    let mut out = Vec::new();
    while let Some(item) = stream.next_packet()? {
        out.push(item);
    }
    Ok(out)
}

fn keyword(expr: &sim_kernel::Expr) -> Result<String> {
    let sim_kernel::Expr::Symbol(symbol) = expr else {
        return Err(Error::TypeMismatch {
            expected: "keyword symbol",
            found: "non-symbol",
        });
    };
    Ok(symbol
        .name
        .strip_prefix(':')
        .unwrap_or(symbol.name.as_ref())
        .to_owned())
}

fn symbolish(expr: &sim_kernel::Expr) -> Result<String> {
    match unquote_ref(expr) {
        sim_kernel::Expr::Symbol(symbol) => Ok(symbol.name.to_string()),
        sim_kernel::Expr::String(value) => Ok(value.clone()),
        _ => Err(Error::Eval(
            "stream/bridge :op expects a symbol or string".to_owned(),
        )),
    }
}

fn usize_value(expr: &sim_kernel::Expr) -> Result<usize> {
    match unquote_ref(expr) {
        sim_kernel::Expr::String(value) => value
            .parse()
            .map_err(|err| Error::Eval(format!("invalid stream/bridge integer option: {err}"))),
        sim_kernel::Expr::Number(number) => number.canonical.parse().map_err(|err| {
            Error::Eval(format!(
                "invalid stream/bridge integer number option: {err}"
            ))
        }),
        _ => Err(Error::Eval(
            "stream/bridge integer option expects string or number".to_owned(),
        )),
    }
}

fn u32_option(expr: &sim_kernel::Expr, label: &str) -> Result<u32> {
    u32::try_from(usize_value(expr)?)
        .map_err(|_| Error::Eval(format!("stream/bridge {label} is out of range")))
}

fn u16_option(expr: &sim_kernel::Expr, label: &str) -> Result<u16> {
    u16::try_from(usize_value(expr)?)
        .map_err(|_| Error::Eval(format!("stream/bridge {label} is out of range")))
}

fn u8_option(expr: &sim_kernel::Expr, label: &str) -> Result<u8> {
    u8::try_from(usize_value(expr)?)
        .map_err(|_| Error::Eval(format!("stream/bridge {label} is out of range")))
}

fn f64_value(expr: &sim_kernel::Expr) -> Result<f64> {
    match unquote_ref(expr) {
        sim_kernel::Expr::String(value) => value
            .parse()
            .map_err(|err| Error::Eval(format!("invalid stream/bridge float option: {err}"))),
        sim_kernel::Expr::Number(number) => number.canonical.parse().map_err(|err| {
            Error::Eval(format!("invalid stream/bridge float number option: {err}"))
        }),
        _ => Err(Error::Eval(
            "stream/bridge float option expects string or number".to_owned(),
        )),
    }
}

fn unquote(expr: &sim_kernel::Expr) -> sim_kernel::Expr {
    unquote_ref(expr).clone()
}

fn unquote_ref(expr: &sim_kernel::Expr) -> &sim_kernel::Expr {
    match expr {
        sim_kernel::Expr::Quote {
            mode: sim_kernel::QuoteMode::Quote,
            expr,
        } => expr,
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integer_option_helpers_reject_narrowing_overflow() {
        assert!(
            u32_option(
                &sim_kernel::Expr::String("4294967296".to_owned()),
                "sample-rate"
            )
            .unwrap_err()
            .to_string()
            .contains("out of range")
        );
        assert!(
            u16_option(&sim_kernel::Expr::String("65536".to_owned()), "tpq")
                .unwrap_err()
                .to_string()
                .contains("out of range")
        );
        assert!(
            u8_option(&sim_kernel::Expr::String("256".to_owned()), "channels")
                .unwrap_err()
                .to_string()
                .contains("out of range")
        );
    }
}
