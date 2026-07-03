use sim_kernel::{Error, Expr, Result, Symbol};

use crate::arranger::{Arranger, music_err};
use crate::{Articulation, Music, MusicObject, Note, Pitch, Time, TimedNote};

const NS: &str = "music/arranger";

pub(crate) fn music_to_expr(music: &Music) -> Expr {
    match music {
        Music::Note(note) => note_expr(note),
        Music::Rest(rest) => map(vec![
            ("tag", tag_expr("music-rest")),
            ("duration", time_expr(rest.duration)),
        ]),
        Music::PianoRoll(roll) => map(vec![
            ("tag", tag_expr("music-piano-roll")),
            (
                "items",
                Expr::Vector(
                    roll.items
                        .iter()
                        .map(|item| {
                            map(vec![
                                ("onset", time_expr(item.onset)),
                                ("note", note_expr(&item.note)),
                            ])
                        })
                        .collect(),
                ),
            ),
        ]),
        Music::Arranger(arranger) => arranger.to_expr(),
        _ => map(vec![
            ("tag", tag_expr("music-object")),
            ("kind", Expr::String(music.kind().to_owned())),
        ]),
    }
}

pub(crate) fn music_from_expr(expr: &Expr) -> Result<Music> {
    let entries = expr_map(expr, "music object")?;
    match symbol_name(lookup_required(entries, "tag")?, "music object tag")? {
        "music-note" => Ok(Music::Note(note_from_expr(expr)?)),
        "music-rest" => Ok(Music::Rest(
            crate::Rest::new(time_from_expr(lookup_required(entries, "duration")?)?)
                .map_err(music_err)?,
        )),
        "music-piano-roll" => Ok(Music::PianoRoll(
            crate::PianoRoll::new(
                expr_vector(lookup_required(entries, "items")?, "piano-roll items")?
                    .iter()
                    .map(timed_note_from_expr)
                    .collect::<Result<Vec<_>>>()?,
            )
            .map_err(music_err)?,
        )),
        "arranger" => Ok(Music::Arranger(Arranger::from_expr(expr)?)),
        _ => Err(Error::Eval(
            "arranger expression contains an unsupported music object".to_owned(),
        )),
    }
}

fn note_expr(note: &Note) -> Expr {
    map(vec![
        ("tag", tag_expr("music-note")),
        ("duration", time_expr(note.duration)),
        ("pitch", pitch_expr(note.pitch)),
        ("velocity", Expr::String(note.velocity.to_string())),
        ("channel", Expr::String(note.channel.0.to_string())),
        (
            "articulation",
            Expr::String(format!("{:?}", note.articulation)),
        ),
    ])
}

fn note_from_expr(expr: &Expr) -> Result<Note> {
    let entries = expr_map(expr, "note")?;
    expect_tag(entries, "music-note", "note")?;
    Note::new(
        time_from_expr(lookup_required(entries, "duration")?)?,
        pitch_from_expr(lookup_required(entries, "pitch")?)?,
        expr_u8(lookup_required(entries, "velocity")?, "velocity")?,
        crate::Channel::new(expr_u8(lookup_required(entries, "channel")?, "channel")?)
            .map_err(|_| Error::Eval("channel is invalid".to_owned()))?,
        articulation_from_expr(lookup_required(entries, "articulation")?)?,
    )
    .map_err(music_err)
}

fn timed_note_from_expr(expr: &Expr) -> Result<TimedNote> {
    let item = expr_map(expr, "piano-roll item")?;
    Ok(TimedNote {
        onset: time_from_expr(lookup_required(item, "onset")?)?,
        note: note_from_expr(lookup_required(item, "note")?)?,
    })
}

fn articulation_from_expr(expr: &Expr) -> Result<Articulation> {
    match expr_string(expr, "articulation")? {
        "Normal" => Ok(Articulation::Normal),
        "Staccato" => Ok(Articulation::Staccato),
        "Legato" => Ok(Articulation::Legato),
        "Tenuto" => Ok(Articulation::Tenuto),
        "Accent" => Ok(Articulation::Accent),
        "Marcato" => Ok(Articulation::Marcato),
        _ => Err(Error::Eval("articulation is invalid".to_owned())),
    }
}

fn time_expr(time: Time) -> Expr {
    map(vec![
        ("numer", Expr::String(time.numer().to_string())),
        ("denom", Expr::String(time.denom().to_string())),
    ])
}

fn time_from_expr(expr: &Expr) -> Result<Time> {
    let entries = expr_map(expr, "time")?;
    let denominator = expr_i64(lookup_required(entries, "denom")?, "time denominator")?;
    if denominator == 0 {
        return Err(Error::Eval("time denominator cannot be zero".to_owned()));
    }
    Ok(Time::new(
        expr_i64(lookup_required(entries, "numer")?, "time numerator")?,
        denominator,
    ))
}

fn pitch_expr(pitch: Pitch) -> Expr {
    Expr::String(
        pitch
            .to_midi()
            .map(|midi| format!("midi:{midi}"))
            .unwrap_or_else(|| format!("semitone:{}", pitch.semitone())),
    )
}

fn pitch_from_expr(expr: &Expr) -> Result<Pitch> {
    let value = expr_string(expr, "pitch")?;
    if let Some(midi) = value.strip_prefix("midi:") {
        return Ok(Pitch::from_midi(
            midi.parse::<u8>()
                .map_err(|_| Error::Eval("MIDI pitch is invalid".to_owned()))?,
        ));
    }
    if let Some(semitone) = value.strip_prefix("semitone:") {
        return Ok(Pitch::from_semitone(semitone.parse::<i32>().map_err(
            |_| Error::Eval("semitone pitch is invalid".to_owned()),
        )?));
    }
    crate::parse_pitch(value).map_err(|_| Error::Eval("pitch is invalid".to_owned()))
}

fn map(entries: Vec<(&'static str, Expr)>) -> Expr {
    Expr::Map(
        entries
            .into_iter()
            .map(|(key, value)| (field(key), value))
            .collect(),
    )
}

fn field(name: &'static str) -> Expr {
    sim_value::build::qsym(NS, name)
}

fn tag(name: &'static str) -> Symbol {
    Symbol::qualified(NS, name)
}

fn tag_expr(name: &'static str) -> Expr {
    Expr::Symbol(tag(name))
}

fn expr_map<'a>(expr: &'a Expr, context: &str) -> Result<&'a [(Expr, Expr)]> {
    match expr {
        Expr::Map(entries) => Ok(entries),
        _ => Err(Error::Eval(format!("{context} must be a map"))),
    }
}

fn expr_vector<'a>(expr: &'a Expr, context: &str) -> Result<&'a [Expr]> {
    match expr {
        Expr::Vector(items) => Ok(items),
        _ => Err(Error::Eval(format!("{context} must be a vector"))),
    }
}

fn lookup_required<'a>(entries: &'a [(Expr, Expr)], name: &str) -> Result<&'a Expr> {
    lookup(entries, name).ok_or_else(|| Error::Eval(format!("arranger field is missing: {name}")))
}

fn lookup<'a>(entries: &'a [(Expr, Expr)], name: &str) -> Option<&'a Expr> {
    entries.iter().find_map(|(key, value)| match key {
        Expr::Symbol(symbol)
            if symbol.namespace.as_deref() == Some(NS) && symbol.name.as_ref() == name =>
        {
            Some(value)
        }
        _ => None,
    })
}

fn expect_tag(entries: &[(Expr, Expr)], name: &'static str, context: &str) -> Result<()> {
    match lookup(entries, "tag") {
        Some(Expr::Symbol(symbol))
            if symbol.namespace.as_deref() == Some(NS) && symbol.name.as_ref() == name =>
        {
            Ok(())
        }
        Some(_) => Err(Error::Eval(format!("{context} tag is invalid"))),
        None => Err(Error::Eval(format!("{context} tag is missing"))),
    }
}

fn symbol_name<'a>(expr: &'a Expr, context: &str) -> Result<&'a str> {
    match expr {
        Expr::Symbol(symbol) if symbol.namespace.as_deref() == Some(NS) => Ok(symbol.name.as_ref()),
        _ => Err(Error::Eval(format!("{context} must be an arranger symbol"))),
    }
}

fn expr_string<'a>(expr: &'a Expr, context: &str) -> Result<&'a str> {
    match expr {
        Expr::String(value) => Ok(value),
        _ => Err(Error::Eval(format!("{context} must be a string"))),
    }
}

fn expr_i64(expr: &Expr, context: &str) -> Result<i64> {
    parse_number(expr, context)
}

fn expr_u8(expr: &Expr, context: &str) -> Result<u8> {
    parse_number(expr, context)
}

fn parse_number<T>(expr: &Expr, context: &str) -> Result<T>
where
    T: std::str::FromStr,
{
    let text = match expr {
        Expr::String(value) => value.as_str(),
        Expr::Number(value) => value.canonical.as_str(),
        _ => return Err(Error::Eval(format!("{context} must be a number"))),
    };
    text.parse()
        .map_err(|_| Error::Eval(format!("{context} is invalid")))
}
