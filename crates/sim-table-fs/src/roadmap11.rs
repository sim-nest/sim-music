use sim_kernel::Expr;
#[cfg(any(
    feature = "midi",
    feature = "music",
    feature = "sound",
    feature = "tuning",
    feature = "notation"
))]
use sim_kernel::{Error, Symbol};

#[cfg(feature = "midi")]
use sim_lib_midi_shapes::{decode_smf_file, encode_smf_file};
#[cfg(feature = "midi")]
use sim_lib_midi_smf::{read_smf, write_smf};
#[cfg(feature = "notation")]
use sim_lib_music_notation::{export_lilypond, import_lilypond};
#[cfg(feature = "music")]
use sim_lib_music_shapes::{decode_music_file, encode_music_file};
#[cfg(feature = "sound")]
use sim_lib_sound_shapes::{decode_tone, encode_tone};
#[cfg(feature = "tuning")]
use sim_lib_sound_shapes::{decode_tuning_descriptor, encode_tuning_descriptor};
#[cfg(feature = "tuning")]
use sim_lib_sound_tuning::TuningDescriptor;

pub(crate) fn known_exts() -> Vec<&'static str> {
    with_roadmap11_exts(vec!["siml", "simb", "simb64", "simj", "sima"])
}

#[cfg(any(
    feature = "midi",
    feature = "music",
    feature = "sound",
    feature = "tuning",
    feature = "notation"
))]
fn with_roadmap11_exts(mut exts: Vec<&'static str>) -> Vec<&'static str> {
    #[cfg(feature = "midi")]
    exts.push("mid");
    #[cfg(feature = "music")]
    exts.push("music");
    #[cfg(feature = "sound")]
    exts.push("tone");
    #[cfg(feature = "tuning")]
    exts.push("scl");
    #[cfg(feature = "notation")]
    exts.push("ly");
    exts
}

#[cfg(not(any(
    feature = "midi",
    feature = "music",
    feature = "sound",
    feature = "tuning",
    feature = "notation"
)))]
fn with_roadmap11_exts(exts: Vec<&'static str>) -> Vec<&'static str> {
    exts
}

pub(crate) fn decode_expr_for_ext(ext: &str, _bytes: &[u8]) -> Option<sim_kernel::Result<Expr>> {
    match ext {
        #[cfg(feature = "midi")]
        "mid" => Some(decode_midi(_bytes)),
        #[cfg(feature = "music")]
        "music" => Some(decode_music(_bytes)),
        #[cfg(feature = "sound")]
        "tone" => Some(decode_tone_expr(_bytes)),
        #[cfg(feature = "tuning")]
        "scl" => Some(decode_scala(_bytes)),
        #[cfg(feature = "notation")]
        "ly" => Some(decode_lilypond(_bytes)),
        _ => None,
    }
}

pub(crate) fn infer_ext_from_expr(expr: &Expr) -> Option<&'static str> {
    #[cfg(any(
        feature = "midi",
        feature = "music",
        feature = "sound",
        feature = "tuning"
    ))]
    {
        let (tag, payload) = tagged_payload(expr)?;
        #[cfg(feature = "midi")]
        if tag == &Symbol::qualified("midi", "SmfFile") {
            return decode_smf_payload(payload).ok().map(|_| "mid");
        }
        #[cfg(feature = "music")]
        if tag == &Symbol::qualified("music", "Score") {
            return decode_score_payload(payload).ok().map(|_| "music");
        }
        #[cfg(feature = "sound")]
        if tag == &Symbol::qualified("sound", "Tone") {
            return decode_tone_payload(payload).ok().map(|_| "tone");
        }
        #[cfg(feature = "tuning")]
        if tag == &Symbol::qualified("sound", "TuningDescriptor") {
            return decode_tuning_payload(payload).ok().and_then(|descriptor| {
                matches!(descriptor, TuningDescriptor::ScalaScl { .. }).then_some("scl")
            });
        }
    }
    #[cfg(not(any(
        feature = "midi",
        feature = "music",
        feature = "sound",
        feature = "tuning"
    )))]
    let _ = expr;
    None
}

pub(crate) fn encode_expr_for_ext(ext: &str, _expr: &Expr) -> Option<sim_kernel::Result<Vec<u8>>> {
    match ext {
        #[cfg(feature = "midi")]
        "mid" => Some(encode_midi(_expr)),
        #[cfg(feature = "music")]
        "music" => Some(encode_music(_expr)),
        #[cfg(feature = "sound")]
        "tone" => Some(encode_tone_expr(_expr)),
        #[cfg(feature = "tuning")]
        "scl" => Some(encode_scala(_expr)),
        #[cfg(feature = "notation")]
        "ly" => Some(encode_lilypond(_expr)),
        _ => None,
    }
}

#[cfg(any(
    feature = "midi",
    feature = "music",
    feature = "sound",
    feature = "tuning",
    feature = "notation"
))]
fn tagged_payload(expr: &Expr) -> Option<(&Symbol, &str)> {
    let Expr::Extension { tag, payload } = expr else {
        return None;
    };
    let Expr::String(text) = payload.as_ref() else {
        return None;
    };
    Some((tag, text.as_str()))
}

#[cfg(feature = "midi")]
fn decode_midi(bytes: &[u8]) -> sim_kernel::Result<Expr> {
    let file = read_smf(bytes).map_err(|err| Error::Eval(format!("table/fs: midi read {err}")))?;
    Ok(Expr::Extension {
        tag: Symbol::qualified("midi", "SmfFile"),
        payload: Box::new(Expr::String(encode_smf_file(&file))),
    })
}

#[cfg(feature = "midi")]
fn encode_midi(expr: &Expr) -> sim_kernel::Result<Vec<u8>> {
    let payload = expect_tagged_string(expr, &Symbol::qualified("midi", "SmfFile"))?;
    let file = decode_smf_payload(payload)?;
    write_smf(&file).map_err(|err| Error::Eval(format!("table/fs: midi write {err}")))
}

#[cfg(feature = "midi")]
fn decode_smf_payload(payload: &str) -> sim_kernel::Result<sim_lib_midi_smf::SmfFile> {
    decode_smf_file(payload).map_err(|err| Error::Eval(format!("table/fs: midi shape {err}")))
}

#[cfg(feature = "music")]
fn decode_music(bytes: &[u8]) -> sim_kernel::Result<Expr> {
    let text =
        std::str::from_utf8(bytes).map_err(|err| Error::Eval(format!("table/fs: utf8 {err}")))?;
    let score = decode_music_file(text)
        .map_err(|err| Error::Eval(format!("table/fs: music decode {err}")))?;
    Ok(Expr::Extension {
        tag: Symbol::qualified("music", "Score"),
        payload: Box::new(Expr::String(encode_music_file(&score))),
    })
}

#[cfg(feature = "music")]
fn encode_music(expr: &Expr) -> sim_kernel::Result<Vec<u8>> {
    let payload = expect_tagged_string(expr, &Symbol::qualified("music", "Score"))?;
    let score = decode_score_payload(payload)?;
    Ok(encode_music_file(&score).into_bytes())
}

#[cfg(feature = "music")]
fn decode_score_payload(payload: &str) -> sim_kernel::Result<sim_lib_music_core::Score> {
    decode_music_file(payload).map_err(|err| Error::Eval(format!("table/fs: music shape {err}")))
}

#[cfg(feature = "sound")]
fn decode_tone_expr(bytes: &[u8]) -> sim_kernel::Result<Expr> {
    let text =
        std::str::from_utf8(bytes).map_err(|err| Error::Eval(format!("table/fs: utf8 {err}")))?;
    let tone =
        decode_tone(text).map_err(|err| Error::Eval(format!("table/fs: tone decode {err}")))?;
    Ok(Expr::Extension {
        tag: Symbol::qualified("sound", "Tone"),
        payload: Box::new(Expr::String(encode_tone(&tone))),
    })
}

#[cfg(feature = "sound")]
fn encode_tone_expr(expr: &Expr) -> sim_kernel::Result<Vec<u8>> {
    let payload = expect_tagged_string(expr, &Symbol::qualified("sound", "Tone"))?;
    let tone = decode_tone_payload(payload)?;
    Ok(encode_tone(&tone).into_bytes())
}

#[cfg(feature = "sound")]
fn decode_tone_payload(payload: &str) -> sim_kernel::Result<sim_lib_sound_core::Tone> {
    decode_tone(payload).map_err(|err| Error::Eval(format!("table/fs: tone shape {err}")))
}

#[cfg(feature = "tuning")]
fn decode_scala(bytes: &[u8]) -> sim_kernel::Result<Expr> {
    let text =
        std::str::from_utf8(bytes).map_err(|err| Error::Eval(format!("table/fs: utf8 {err}")))?;
    let scala = sim_lib_sound_tuning::ScalaScl::parse(
        text,
        (
            sim_lib_pitch_core::Pitch::from_midi(69),
            sim_lib_sound_core::Frequency(440.0),
        ),
    )
    .map_err(|err| Error::Eval(format!("table/fs: scala decode {err}")))?;
    let descriptor = TuningDescriptor::ScalaScl {
        cents: scala.cents,
        reference_midi: 69,
        reference_hz: 440.0,
    };
    Ok(Expr::Extension {
        tag: Symbol::qualified("sound", "TuningDescriptor"),
        payload: Box::new(Expr::String(encode_tuning_descriptor(&descriptor))),
    })
}

#[cfg(feature = "tuning")]
fn encode_scala(expr: &Expr) -> sim_kernel::Result<Vec<u8>> {
    let payload = expect_tagged_string(expr, &Symbol::qualified("sound", "TuningDescriptor"))?;
    let descriptor = decode_tuning_payload(payload)?;
    let TuningDescriptor::ScalaScl { cents, .. } = descriptor else {
        return Err(Error::Eval(
            "table/fs: only ScalaScl descriptors can write .scl".to_owned(),
        ));
    };
    Ok(render_scala(&cents).into_bytes())
}

#[cfg(feature = "tuning")]
fn decode_tuning_payload(payload: &str) -> sim_kernel::Result<TuningDescriptor> {
    decode_tuning_descriptor(payload)
        .map_err(|err| Error::Eval(format!("table/fs: tuning shape {err}")))
}

#[cfg(feature = "tuning")]
fn render_scala(cents: &[f64]) -> String {
    let mut lines = Vec::with_capacity(cents.len() + 2);
    lines.push("SIM Scala export".to_owned());
    lines.push(cents.len().to_string());
    lines.extend(cents.iter().map(|value| value.to_string()));
    lines.join("\n")
}

#[cfg(feature = "notation")]
fn decode_lilypond(bytes: &[u8]) -> sim_kernel::Result<Expr> {
    let text =
        std::str::from_utf8(bytes).map_err(|err| Error::Eval(format!("table/fs: utf8 {err}")))?;
    let score =
        import_lilypond(text).map_err(|err| Error::Eval(format!("table/fs: lilypond {err}")))?;
    Ok(Expr::Extension {
        tag: Symbol::qualified("music", "Score"),
        payload: Box::new(Expr::String(encode_music_file(&score))),
    })
}

#[cfg(feature = "notation")]
fn encode_lilypond(expr: &Expr) -> sim_kernel::Result<Vec<u8>> {
    let payload = expect_tagged_string(expr, &Symbol::qualified("music", "Score"))?;
    let score = decode_score_payload(payload)?;
    export_lilypond(&score)
        .map(String::into_bytes)
        .map_err(|err| Error::Eval(format!("table/fs: lilypond write {err}")))
}

#[cfg(any(
    feature = "midi",
    feature = "music",
    feature = "sound",
    feature = "tuning",
    feature = "notation"
))]
fn expect_tagged_string<'a>(expr: &'a Expr, tag: &Symbol) -> sim_kernel::Result<&'a str> {
    match tagged_payload(expr) {
        Some((actual, payload)) if actual == tag => Ok(payload),
        _ => Err(Error::Eval(format!("table/fs: expected {} artifact", tag))),
    }
}
