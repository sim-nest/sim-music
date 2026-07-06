use std::str::FromStr;

use super::MusicShapeError;
use sim_codec::{DomainForm, DomainFormError, DomainValue, parse_domain_form};
use sim_kernel::Symbol;
use sim_lib_midi_shapes::{decode_midi_event, decode_smf_file};
use sim_lib_music_core::{
    Arranger, ArrangerPlacement, Articulation, Channel, Chord, Counterpoint, FilterRef, LaneId,
    LaneTarget, Melody, MelodyItem, MidiFileObj, MidiTrackObj, Music, MusicObject, Note, Par,
    PianoRoll, PitchClass, PitchRemap, PlacementTransform, PlayableRef, Progression, Rest, Score,
    Seq, StretchPolicy, Time, TimedNote, TracePolicy, parse_pitch,
};

impl From<DomainFormError> for MusicShapeError {
    fn from(error: DomainFormError) -> Self {
        match error {
            DomainFormError::UnexpectedEof => MusicShapeError::UnexpectedEof,
            DomainFormError::ExpectedForm
            | DomainFormError::InvalidToken
            | DomainFormError::TrailingInput
            | DomainFormError::DuplicateField(_) => MusicShapeError::InvalidToken,
            DomainFormError::MissingField(_)
            | DomainFormError::WrongFieldKind(_)
            | DomainFormError::WrongValueKind => MusicShapeError::InvalidMusic,
        }
    }
}

/// Decodes a `numer/denom` text rational into a `Time`.
pub fn decode_time(value: &str) -> Result<Time, MusicShapeError> {
    let (numerator, denominator) = value.split_once('/').ok_or(MusicShapeError::InvalidTime)?;
    let numerator = numerator
        .parse::<i64>()
        .map_err(|_| MusicShapeError::InvalidTime)?;
    let denominator = denominator
        .parse::<i64>()
        .map_err(|_| MusicShapeError::InvalidTime)?;
    if denominator == 0 {
        return Err(MusicShapeError::InvalidTime);
    }
    Ok(Time::new(numerator, denominator))
}

/// Decodes a `#(Note ...)` form into a `Note`.
pub fn decode_note(value: &str) -> Result<Note, MusicShapeError> {
    decode_note_node(&parse_domain_form(value)?)
}

/// Decodes a `#(Rest ...)` form into a `Rest`.
pub fn decode_rest(value: &str) -> Result<Rest, MusicShapeError> {
    decode_rest_node(&parse_domain_form(value)?)
}

/// Decodes a `#(Chord ...)` form into a `Chord`.
pub fn decode_chord(value: &str) -> Result<Chord, MusicShapeError> {
    decode_chord_node(&parse_domain_form(value)?)
}

/// Decodes a `#(Melody ...)` form into a `Melody`.
pub fn decode_melody(value: &str) -> Result<Melody, MusicShapeError> {
    decode_melody_node(&parse_domain_form(value)?)
}

/// Decodes a `#(Progression ...)` form into a `Progression`.
pub fn decode_progression(value: &str) -> Result<Progression, MusicShapeError> {
    decode_progression_node(&parse_domain_form(value)?)
}

/// Decodes a `#(Counterpoint ...)` form into a `Counterpoint`.
pub fn decode_counterpoint(value: &str) -> Result<Counterpoint, MusicShapeError> {
    decode_counterpoint_node(&parse_domain_form(value)?)
}

/// Decodes a `#(PianoRoll ...)` form into a `PianoRoll`.
pub fn decode_piano_roll(value: &str) -> Result<PianoRoll, MusicShapeError> {
    decode_piano_roll_node(&parse_domain_form(value)?)
}

/// Decodes a `#(Arranger ...)` form into an `Arranger`.
pub fn decode_arranger(value: &str) -> Result<Arranger, MusicShapeError> {
    decode_arranger_node(&parse_domain_form(value)?)
}

/// Decodes a `#(MidiTrackObj ...)` form into a `MidiTrackObj`.
pub fn decode_midi_track(value: &str) -> Result<MidiTrackObj, MusicShapeError> {
    decode_midi_track_node(&parse_domain_form(value)?)
}

/// Decodes a `#(MidiFileObj ...)` form into a `MidiFileObj`.
pub fn decode_midi_file(value: &str) -> Result<MidiFileObj, MusicShapeError> {
    decode_midi_file_node(&parse_domain_form(value)?)
}

/// Decodes a `#(Score ...)` root form into a `Score`.
pub fn decode_score(value: &str) -> Result<Score, MusicShapeError> {
    decode_score_node(&parse_domain_form(value)?)
}

/// Decodes any music `#(...)` form, dispatching on the form name to a `Music`.
pub fn decode_music(value: &str) -> Result<Music, MusicShapeError> {
    decode_music_node(&parse_domain_form(value)?)
}

/// Decodes a top-level music file form, which is a `Score`.
pub fn decode_music_file(value: &str) -> Result<Score, MusicShapeError> {
    decode_score(value)
}

fn decode_music_node(form: &DomainForm) -> Result<Music, MusicShapeError> {
    match form.name.as_str() {
        "Note" => decode_note_node(form).map(Music::Note),
        "Rest" => decode_rest_node(form).map(Music::Rest),
        "Par" => decode_children(form).map(|children| Music::Par(Par { children })),
        "Seq" => decode_children(form).map(|children| Music::Seq(Seq { children })),
        "Chord" => decode_chord_node(form).map(Music::Chord),
        "Melody" => decode_melody_node(form).map(Music::Melody),
        "Progression" => decode_progression_node(form).map(Music::Progression),
        "Counterpoint" => decode_counterpoint_node(form).map(Music::Counterpoint),
        "PianoRoll" => decode_piano_roll_node(form).map(Music::PianoRoll),
        "Arranger" => decode_arranger_node(form).map(Music::Arranger),
        "MidiTrackObj" => decode_midi_track_node(form).map(Music::MidiTrack),
        "MidiFileObj" => decode_midi_file_node(form).map(Music::MidiFile),
        _ => Err(MusicShapeError::InvalidMusic),
    }
}

fn decode_note_node(form: &DomainForm) -> Result<Note, MusicShapeError> {
    ensure_form(form, "Note")?;
    Note::new(
        decode_time(form.atom("dur")?)?,
        parse_pitch(form.atom("pitch")?).map_err(|_| MusicShapeError::InvalidMusic)?,
        parse_u8(form.atom("vel")?)?,
        Channel::new(parse_u8(form.atom("channel")?)?)
            .map_err(|_| MusicShapeError::InvalidMusic)?,
        decode_articulation(form.atom("articulation")?)?,
    )
    .map_err(Into::into)
}

fn decode_rest_node(form: &DomainForm) -> Result<Rest, MusicShapeError> {
    ensure_form(form, "Rest")?;
    Rest::new(decode_time(form.atom("dur")?)?).map_err(Into::into)
}

fn decode_chord_node(form: &DomainForm) -> Result<Chord, MusicShapeError> {
    ensure_form(form, "Chord")?;
    let pitches = form
        .list("pitches")?
        .iter()
        .map(|value| match value {
            DomainValue::Atom(atom) => parse_pitch(atom).map_err(|_| MusicShapeError::InvalidMusic),
            _ => Err(MusicShapeError::InvalidMusic),
        })
        .collect::<Result<Vec<_>, _>>()?;
    Chord::new(
        decode_time(form.atom("dur")?)?,
        form.field_atom_or_string("symbol")?,
        pitches,
        parse_u8(form.atom("vel")?)?,
        Channel::new(parse_u8(form.atom("channel")?)?)
            .map_err(|_| MusicShapeError::InvalidMusic)?,
    )
    .map_err(Into::into)
}

fn decode_melody_node(form: &DomainForm) -> Result<Melody, MusicShapeError> {
    ensure_form(form, "Melody")?;
    let items = form
        .list("items")?
        .iter()
        .map(|value| {
            let item = value.as_form()?;
            match item.name.as_str() {
                "Note" => decode_note_node(item).map(MelodyItem::Note),
                "Rest" => decode_rest_node(item).map(MelodyItem::Rest),
                _ => Err(MusicShapeError::InvalidMusic),
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
    Melody::new(items).map_err(Into::into)
}

fn decode_progression_node(form: &DomainForm) -> Result<Progression, MusicShapeError> {
    ensure_form(form, "Progression")?;
    let key = decode_optional_string(field(form, "key")?)?;
    let chords = form
        .list("chords")?
        .iter()
        .map(|value| decode_chord_node(value.as_form()?))
        .collect::<Result<Vec<_>, _>>()?;
    Progression::new(key, chords).map_err(Into::into)
}

fn decode_counterpoint_node(form: &DomainForm) -> Result<Counterpoint, MusicShapeError> {
    ensure_form(form, "Counterpoint")?;
    let voice_names = form
        .list("voice_names")?
        .iter()
        .map(|value| match value {
            DomainValue::String(name) => Ok(name.clone()),
            _ => Err(MusicShapeError::InvalidMusic),
        })
        .collect::<Result<Vec<_>, _>>()?;
    let voices = form
        .list("voices")?
        .iter()
        .map(|value| decode_melody_node(value.as_form()?))
        .collect::<Result<Vec<_>, _>>()?;
    Counterpoint::new(voices, voice_names).map_err(Into::into)
}

fn decode_piano_roll_node(form: &DomainForm) -> Result<PianoRoll, MusicShapeError> {
    ensure_form(form, "PianoRoll")?;
    let items = form
        .list("items")?
        .iter()
        .map(|value| decode_timed_note_node(value.as_form()?))
        .collect::<Result<Vec<_>, _>>()?;
    PianoRoll::new(items).map_err(Into::into)
}

fn decode_arranger_node(form: &DomainForm) -> Result<Arranger, MusicShapeError> {
    ensure_form(form, "Arranger")?;
    let lanes = form
        .list("lanes")?
        .iter()
        .map(|value| Ok(LaneId::new(value.atom_or_string()?)))
        .collect::<Result<Vec<_>, MusicShapeError>>()?;
    let placements = form
        .list("placements")?
        .iter()
        .map(|value| decode_arranger_placement_node(value.as_form()?))
        .collect::<Result<Vec<_>, _>>()?;
    Arranger::new(placements, lanes).map_err(|_| MusicShapeError::InvalidMusic)
}

fn decode_arranger_placement_node(form: &DomainForm) -> Result<ArrangerPlacement, MusicShapeError> {
    ensure_form(form, "ArrangerPlacement")?;
    let duration = match field(form, "duration")? {
        DomainValue::Atom(value) if value == "none" => None,
        DomainValue::Atom(value) => Some(decode_time(value)?),
        _ => return Err(MusicShapeError::InvalidMusic),
    };
    let targets = form
        .list("targets")?
        .iter()
        .map(|value| decode_lane_target_node(value.as_form()?))
        .collect::<Result<Vec<_>, _>>()?;
    let filter = match field(form, "filter")? {
        DomainValue::Atom(value) if value == "none" => None,
        DomainValue::Form(form) => Some(decode_filter_ref_node(form)?),
        _ => return Err(MusicShapeError::InvalidMusic),
    };
    let seed = match field(form, "seed")? {
        DomainValue::Atom(value) if value == "none" => None,
        DomainValue::Atom(value) => Some(parse_u64(value)?),
        _ => return Err(MusicShapeError::InvalidMusic),
    };
    let placement = ArrangerPlacement {
        id: symbol_from_value(field(form, "id")?)?,
        playable: decode_playable_ref(field(form, "playable")?)?,
        at: decode_time(form.atom("at")?)?,
        duration,
        lane: LaneId::new(form.field_atom_or_string("lane")?),
        targets,
        stretch: decode_stretch_policy_node(form.form("stretch")?)?,
        transform: form
            .list("transforms")?
            .iter()
            .map(|value| decode_placement_transform_node(value.as_form()?))
            .collect::<Result<Vec<_>, _>>()?,
        remap_pitch: decode_pitch_remap_node(form.form("remap")?)?,
        filter,
        seed,
        trace: decode_trace_policy(form.atom("trace")?)?,
    };
    Arranger::new(vec![placement.clone()], Vec::new())
        .map_err(|_| MusicShapeError::InvalidMusic)?;
    Ok(placement)
}

fn decode_playable_ref(value: &DomainValue) -> Result<PlayableRef, MusicShapeError> {
    let form = value.as_form()?;
    if form.name == "PlayableRef" {
        match form.atom("kind")? {
            "symbol" => Ok(PlayableRef::symbol(symbol_from_value(field(
                form, "symbol",
            )?)?)),
            _ => Err(MusicShapeError::InvalidMusic),
        }
    } else {
        Ok(PlayableRef::inline(decode_music_node(form)?))
    }
}

fn decode_lane_target_node(form: &DomainForm) -> Result<LaneTarget, MusicShapeError> {
    ensure_form(form, "LaneTarget")?;
    match form.atom("kind")? {
        "instrument" => Ok(LaneTarget::Instrument(symbol_from_value(field(
            form, "symbol",
        )?)?)),
        "stream" => Ok(LaneTarget::Stream(symbol_from_value(field(
            form, "symbol",
        )?)?)),
        "control" => Ok(LaneTarget::Control(symbol_from_value(field(
            form, "symbol",
        )?)?)),
        "none" => Ok(LaneTarget::None),
        _ => Err(MusicShapeError::InvalidMusic),
    }
}

fn decode_stretch_policy_node(form: &DomainForm) -> Result<StretchPolicy, MusicShapeError> {
    ensure_form(form, "StretchPolicy")?;
    match form.atom("kind")? {
        "none" => Ok(StretchPolicy::None),
        "tempo-ratio" => Ok(StretchPolicy::TempoRatio(decode_time(form.atom("value")?)?)),
        "time-ratio" => Ok(StretchPolicy::TimeRatio(decode_time(form.atom("value")?)?)),
        "fit" => Ok(StretchPolicy::FitToDuration),
        _ => Err(MusicShapeError::InvalidMusic),
    }
}

fn decode_placement_transform_node(
    form: &DomainForm,
) -> Result<PlacementTransform, MusicShapeError> {
    ensure_form(form, "PlacementTransform")?;
    match form.atom("kind")? {
        "transpose-semitones" => Ok(PlacementTransform::TransposeSemitones(parse_i32(
            form.atom("value")?,
        )?)),
        "transpose-octaves" => Ok(PlacementTransform::TransposeOctaves(parse_i16(
            form.atom("value")?,
        )?)),
        "invert-pitch" => Ok(PlacementTransform::InvertAroundPitch(
            parse_pitch(form.atom("value")?).map_err(|_| MusicShapeError::InvalidMusic)?,
        )),
        "invert-pitch-class" => Ok(PlacementTransform::InvertAroundPitchClass(
            PitchClass::new(parse_u8(form.atom("value")?)?)
                .map_err(|_| MusicShapeError::InvalidMusic)?,
        )),
        "retrograde" => Ok(PlacementTransform::Retrograde),
        _ => Err(MusicShapeError::InvalidMusic),
    }
}

fn decode_pitch_remap_node(form: &DomainForm) -> Result<PitchRemap, MusicShapeError> {
    ensure_form(form, "PitchRemap")?;
    match form.atom("kind")? {
        "none" => Ok(PitchRemap::None),
        "chromatic" => Ok(PitchRemap::Chromatic(parse_i32(form.atom("value")?)?)),
        "pitch-class" => Ok(PitchRemap::PitchClass {
            from: PitchClass::new(parse_u8(form.atom("from")?)?)
                .map_err(|_| MusicShapeError::InvalidMusic)?,
            to: PitchClass::new(parse_u8(form.atom("to")?)?)
                .map_err(|_| MusicShapeError::InvalidMusic)?,
        }),
        "drum-key" => form
            .list("items")?
            .iter()
            .map(|value| {
                let item = value.as_form()?;
                ensure_form(item, "DrumKey")?;
                Ok((parse_u8(item.atom("from")?)?, parse_u8(item.atom("to")?)?))
            })
            .collect::<Result<Vec<_>, MusicShapeError>>()
            .map(PitchRemap::DrumKey),
        "scale-degree" => symbolic_remap(form, PitchRemap::ScaleDegree),
        "chord-tone" => symbolic_remap(form, PitchRemap::ChordTone),
        "tuning" => symbolic_remap(form, PitchRemap::Tuning),
        "vector" => symbolic_remap(form, PitchRemap::Vector),
        "matrix" => symbolic_remap(form, PitchRemap::Matrix),
        "callable" => symbolic_remap(form, PitchRemap::Callable),
        _ => Err(MusicShapeError::InvalidMusic),
    }
}

fn decode_filter_ref_node(form: &DomainForm) -> Result<FilterRef, MusicShapeError> {
    ensure_form(form, "FilterRef")?;
    Ok(FilterRef::new(
        symbol_from_value(field(form, "id")?)?,
        form.list("keep_lanes")?
            .iter()
            .map(|value| Ok(LaneId::new(value.atom_or_string()?)))
            .collect::<Result<Vec<_>, MusicShapeError>>()?,
    ))
}

fn decode_timed_note_node(form: &DomainForm) -> Result<TimedNote, MusicShapeError> {
    ensure_form(form, "TimedNote")?;
    Ok(TimedNote {
        onset: decode_time(form.atom("onset")?)?,
        note: decode_note_node(form.form("note")?)?,
    })
}

fn decode_midi_track_node(form: &DomainForm) -> Result<MidiTrackObj, MusicShapeError> {
    ensure_form(form, "MidiTrackObj")?;
    let channel_hint = match field(form, "channel_hint")? {
        DomainValue::Atom(atom) if atom == "none" => None,
        DomainValue::Atom(atom) => {
            Some(Channel::new(parse_u8(atom)?).map_err(|_| MusicShapeError::InvalidMusic)?)
        }
        _ => return Err(MusicShapeError::InvalidMusic),
    };
    let events = form
        .list("events")?
        .iter()
        .map(|value| match value {
            DomainValue::String(encoded) => {
                decode_midi_event(encoded).map_err(|_| MusicShapeError::InvalidMusic)
            }
            _ => Err(MusicShapeError::InvalidMusic),
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(MidiTrackObj::new(events, channel_hint))
}

fn decode_midi_file_node(form: &DomainForm) -> Result<MidiFileObj, MusicShapeError> {
    ensure_form(form, "MidiFileObj")?;
    let file = decode_smf_file(form.string("smf")?).map_err(|_| MusicShapeError::InvalidMusic)?;
    Ok(MidiFileObj::new(file))
}

fn decode_score_node(form: &DomainForm) -> Result<Score, MusicShapeError> {
    ensure_form(form, "Score")?;
    let time_sig = form.atom("time_sig")?;
    let (numerator, denominator) = time_sig
        .split_once('/')
        .ok_or(MusicShapeError::InvalidMusic)?;
    Score::new(
        parse_u32(form.atom("tempo")?)?,
        (parse_u8(numerator)?, parse_u8(denominator)?),
        decode_optional_string(field(form, "key")?)?,
        decode_music_node(form.form("body")?)?,
    )
    .map_err(Into::into)
}

fn decode_children(form: &DomainForm) -> Result<Vec<Box<dyn MusicObject>>, MusicShapeError> {
    form.list("children")?
        .iter()
        .map(|value| {
            let child = value.as_form()?;
            Ok(Box::new(decode_music_node(child)?) as Box<dyn MusicObject>)
        })
        .collect()
}

fn decode_optional_string(value: &DomainValue) -> Result<Option<String>, MusicShapeError> {
    match value {
        DomainValue::Atom(atom) if atom == "none" => Ok(None),
        DomainValue::String(value) => Ok(Some(value.clone())),
        DomainValue::Atom(value) => Ok(Some(value.clone())),
        _ => Err(MusicShapeError::InvalidMusic),
    }
}

fn decode_articulation(value: &str) -> Result<Articulation, MusicShapeError> {
    match value {
        "Normal" => Ok(Articulation::Normal),
        "Staccato" => Ok(Articulation::Staccato),
        "Legato" => Ok(Articulation::Legato),
        "Tenuto" => Ok(Articulation::Tenuto),
        "Accent" => Ok(Articulation::Accent),
        "Marcato" => Ok(Articulation::Marcato),
        _ => Err(MusicShapeError::InvalidMusic),
    }
}

fn ensure_form(form: &DomainForm, expected: &str) -> Result<(), MusicShapeError> {
    if form.name == expected {
        Ok(())
    } else {
        Err(MusicShapeError::InvalidMusic)
    }
}

fn field<'a>(form: &'a DomainForm, name: &str) -> Result<&'a DomainValue, MusicShapeError> {
    form.field(name).ok_or(MusicShapeError::InvalidMusic)
}

fn symbol_from_value(value: &DomainValue) -> Result<Symbol, MusicShapeError> {
    Ok(symbol_from_text(value.atom_or_string()?))
}

fn symbol_from_text(value: &str) -> Symbol {
    match value.rsplit_once('/') {
        Some((namespace, name)) => Symbol::qualified(namespace.to_owned(), name.to_owned()),
        None => Symbol::new(value.to_owned()),
    }
}

fn symbolic_remap(
    form: &DomainForm,
    build: impl FnOnce(Symbol) -> PitchRemap,
) -> Result<PitchRemap, MusicShapeError> {
    Ok(build(symbol_from_value(field(form, "symbol")?)?))
}

fn decode_trace_policy(value: &str) -> Result<TracePolicy, MusicShapeError> {
    match value {
        "off" => Ok(TracePolicy::Off),
        "diagnostics" => Ok(TracePolicy::Diagnostics),
        "full" => Ok(TracePolicy::Full),
        _ => Err(MusicShapeError::InvalidMusic),
    }
}

fn parse_i16(value: &str) -> Result<i16, MusicShapeError> {
    i16::from_str(value).map_err(|_| MusicShapeError::InvalidMusic)
}

fn parse_i32(value: &str) -> Result<i32, MusicShapeError> {
    i32::from_str(value).map_err(|_| MusicShapeError::InvalidMusic)
}

fn parse_u8(value: &str) -> Result<u8, MusicShapeError> {
    u8::from_str(value).map_err(|_| MusicShapeError::InvalidMusic)
}

fn parse_u32(value: &str) -> Result<u32, MusicShapeError> {
    u32::from_str(value).map_err(|_| MusicShapeError::InvalidMusic)
}

fn parse_u64(value: &str) -> Result<u64, MusicShapeError> {
    u64::from_str(value).map_err(|_| MusicShapeError::InvalidMusic)
}
