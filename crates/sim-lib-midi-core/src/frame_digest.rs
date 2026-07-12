//! A deterministic offline MIDI artifact for the cookbook (COOKBOOK_8 Category C).
//!
//! `midi/chord-digest` encodes a fixed C-major triad to its canonical MIDI wire
//! bytes and returns the [`sim_cookbook::frame_digest`] of those bytes. It is a
//! Category C recipe made deterministic by the digest convention: the render is
//! offline (no device, no clock, no entropy), the bytes are fixed by the MIDI
//! spec, and the digest is integer-only -- so two runs reproduce byte-for-byte,
//! which the cookbook twice-run guard asserts.

use std::sync::Arc;

use sim_cookbook::frame_digest;
use sim_kernel::{
    AbiVersion, Args, Callable, ClassRef, Cx, Error, Export, Expr, Lib, LibManifest, LibTarget,
    Linker, LoadCx, Object, ObjectCompat, Result, Symbol, Value, Version,
};

use crate::wire::encode_channel;
use crate::{Channel, ChannelMessage, U7};

/// The `midi/digest` lib id.
pub fn manifest_name() -> Symbol {
    Symbol::qualified("midi", "digest")
}

/// The `midi/chord-digest` op symbol.
fn chord_digest_symbol() -> Symbol {
    Symbol::qualified("midi", "chord-digest")
}

/// Encode a major triad rooted at `root` (note-ons on channel 0 at velocity 100)
/// to canonical MIDI wire bytes and return their frame digest. Pure and
/// deterministic. A root that would push a note past the 7-bit MIDI range wraps
/// via `U7`'s low 7 bits, keeping the encode total and thus the digest defined.
fn chord_digest(root: u8) -> String {
    let mut bytes = Vec::new();
    for key in [root, root.wrapping_add(4), root.wrapping_add(7)] {
        let (status, data) = encode_channel(&ChannelMessage::NoteOn {
            ch: Channel(0),
            key: U7(key & 0x7f),
            vel: U7(100),
        });
        bytes.push(status);
        bytes.extend(data);
    }
    frame_digest(&bytes)
}

/// Parse a `midi/chord-digest` root-note argument (a decimal string).
fn root_arg(cx: &mut Cx, value: &Value) -> Result<u8> {
    let text = match value.object().as_expr(cx)? {
        Expr::String(text) => text,
        _ => {
            return Err(Error::Eval(
                "midi/chord-digest expects a string root".to_owned(),
            ));
        }
    };
    text.trim().parse::<u8>().map_err(|_| {
        Error::Eval(format!(
            "midi/chord-digest root must be 0-255, got {text:?}"
        ))
    })
}

/// Callable runtime object exposing `midi/chord-digest`.
struct ChordDigestOp;

impl Object for ChordDigestOp {
    fn display(&self, _cx: &mut Cx) -> Result<String> {
        Ok("#<function midi/chord-digest>".to_owned())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl ObjectCompat for ChordDigestOp {
    fn class(&self, cx: &mut Cx) -> Result<ClassRef> {
        cx.resolve_class(&Symbol::qualified("core", "Function"))
    }

    fn as_callable(&self) -> Option<&dyn Callable> {
        Some(self)
    }
}

impl Callable for ChordDigestOp {
    fn call(&self, cx: &mut Cx, args: Args) -> Result<Value> {
        let args = args.into_vec();
        let [root] = args.as_slice() else {
            return Err(Error::Eval(format!(
                "midi/chord-digest expects one root-note argument, got {}",
                args.len()
            )));
        };
        let root = root_arg(cx, root)?;
        cx.factory().string(chord_digest(root))
    }
}

/// The MIDI digest lib: registers `midi/chord-digest` as a callable.
pub struct MidiDigestLib;

impl Lib for MidiDigestLib {
    fn manifest(&self) -> LibManifest {
        LibManifest {
            id: manifest_name(),
            version: Version(env!("CARGO_PKG_VERSION").to_owned()),
            abi: AbiVersion { major: 0, minor: 1 },
            target: LibTarget::HostRegistered,
            requires: Vec::new(),
            capabilities: Vec::new(),
            exports: vec![Export::Function {
                symbol: chord_digest_symbol(),
                function_id: None,
            }],
        }
    }

    fn load(&self, cx: &mut LoadCx, linker: &mut Linker<'_>) -> Result<()> {
        linker.function_value(
            chord_digest_symbol(),
            cx.factory().opaque(Arc::new(ChordDigestOp))?,
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chord_digest_is_a_deterministic_frame() {
        // Three note-ons -> 9 wire bytes (status + 2 data each).
        let digest = chord_digest(60);
        assert!(digest.starts_with("(frame (bytes 9) (hash "));
        assert_eq!(digest, chord_digest(60));
    }
}
