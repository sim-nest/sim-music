use sim_kernel::{Error, Expr, Result, Symbol};

use crate::{Channel, Pitch, Tick};

/// A raw performance gesture submitted to a source at a given input time.
///
/// Pairs a [`PerformanceIntent`] with the [`Tick`] at which it was played, before
/// any source-side transformation (transpose, scale lock) is applied.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PerformanceInput {
    /// Tick at which the gesture was played.
    pub input_time: Tick,
    /// The gesture to perform.
    pub intent: PerformanceIntent,
}

impl PerformanceInput {
    /// Creates an input pairing `input_time` with `intent`.
    pub fn new(input_time: Tick, intent: PerformanceIntent) -> Self {
        Self { input_time, intent }
    }

    /// Builds a note-on input from a raw MIDI note number and velocity.
    pub fn note_on(input_time: Tick, channel: Channel, midi: u8, velocity: u8) -> Self {
        Self::new(
            input_time,
            PerformanceIntent::NoteOn {
                pitch: Pitch::from_midi(midi),
                velocity,
                channel,
            },
        )
    }

    /// Builds a note-off input from a raw MIDI note number and velocity.
    pub fn note_off(input_time: Tick, channel: Channel, midi: u8, velocity: u8) -> Self {
        Self::new(
            input_time,
            PerformanceIntent::NoteOff {
                pitch: Pitch::from_midi(midi),
                velocity,
                channel,
            },
        )
    }

    /// Builds a sustain-pedal input with the pedal `down` state.
    pub fn sustain(input_time: Tick, channel: Channel, down: bool) -> Self {
        Self::new(input_time, PerformanceIntent::Sustain { down, channel })
    }
}

/// A single live-performance gesture.
///
/// Enumerates the MIDI-style intents a [`PerformanceSource`](crate::PerformanceSource)
/// can emit: note triggers, expression controls, the sustain pedal, parameter changes,
/// and an all-notes-off panic.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PerformanceIntent {
    /// Starts a note at the given pitch and velocity on a channel.
    NoteOn {
        /// Pitch of the note.
        pitch: Pitch,
        /// Attack velocity (0..=127).
        velocity: u8,
        /// Channel the note plays on.
        channel: Channel,
    },
    /// Releases a previously started note.
    NoteOff {
        /// Pitch of the note being released.
        pitch: Pitch,
        /// Release velocity (0..=127).
        velocity: u8,
        /// Channel the note plays on.
        channel: Channel,
    },
    /// Applies polyphonic key pressure to a sounding note.
    Aftertouch {
        /// Pitch the pressure applies to.
        pitch: Pitch,
        /// Pressure amount (0..=127).
        pressure: u8,
        /// Channel the note plays on.
        channel: Channel,
    },
    /// Bends pitch on a channel by a 14-bit amount.
    PitchBend {
        /// Raw 14-bit bend value (0..=16383).
        value: u16,
        /// Channel the bend applies to.
        channel: Channel,
    },
    /// Sets the sustain pedal state on a channel.
    Sustain {
        /// Whether the pedal is depressed.
        down: bool,
        /// Channel the pedal applies to.
        channel: Channel,
    },
    /// Sets a named parameter to an integer value.
    Parameter {
        /// Symbol naming the parameter target.
        target: Symbol,
        /// New parameter value.
        value: i64,
    },
    /// Requests an all-notes-off panic and pedal reset.
    Panic,
}

impl PerformanceIntent {
    /// Returns the qualified symbol identifying this intent's kind.
    pub fn kind_symbol(&self) -> Symbol {
        Symbol::qualified("music/performance-intent", self.kind_label())
    }

    /// Returns the short kebab-case label for this intent's kind.
    pub fn kind_label(&self) -> &'static str {
        match self {
            Self::NoteOn { .. } => "note-on",
            Self::NoteOff { .. } => "note-off",
            Self::Aftertouch { .. } => "aftertouch",
            Self::PitchBend { .. } => "pitch-bend",
            Self::Sustain { .. } => "sustain",
            Self::Parameter { .. } => "parameter",
            Self::Panic => "panic",
        }
    }

    /// Encodes this intent as an [`Expr`] map keyed by `kind` and its fields.
    pub fn to_expr(&self) -> Expr {
        let mut entries = vec![(
            Expr::Symbol(Symbol::new("kind")),
            Expr::Symbol(self.kind_symbol()),
        )];
        match self {
            Self::NoteOn {
                pitch,
                velocity,
                channel,
            }
            | Self::NoteOff {
                pitch,
                velocity,
                channel,
            } => {
                entries.push((Expr::Symbol(Symbol::new("pitch")), pitch_expr(*pitch)));
                entries.push((
                    Expr::Symbol(Symbol::new("velocity")),
                    Expr::String(velocity.to_string()),
                ));
                entries.push((Expr::Symbol(Symbol::new("channel")), channel_expr(*channel)));
            }
            Self::Aftertouch {
                pitch,
                pressure,
                channel,
            } => {
                entries.push((Expr::Symbol(Symbol::new("pitch")), pitch_expr(*pitch)));
                entries.push((
                    Expr::Symbol(Symbol::new("pressure")),
                    Expr::String(pressure.to_string()),
                ));
                entries.push((Expr::Symbol(Symbol::new("channel")), channel_expr(*channel)));
            }
            Self::PitchBend { value, channel } => {
                entries.push((
                    Expr::Symbol(Symbol::new("value")),
                    Expr::String(value.to_string()),
                ));
                entries.push((Expr::Symbol(Symbol::new("channel")), channel_expr(*channel)));
            }
            Self::Sustain { down, channel } => {
                entries.push((Expr::Symbol(Symbol::new("down")), Expr::Bool(*down)));
                entries.push((Expr::Symbol(Symbol::new("channel")), channel_expr(*channel)));
            }
            Self::Parameter { target, value } => {
                entries.push((
                    Expr::Symbol(Symbol::new("target")),
                    Expr::Symbol(target.clone()),
                ));
                entries.push((
                    Expr::Symbol(Symbol::new("value")),
                    Expr::String(value.to_string()),
                ));
            }
            Self::Panic => {}
        }
        Expr::Map(entries)
    }

    /// Decodes an intent from an [`Expr`] map produced by [`to_expr`](Self::to_expr).
    ///
    /// Accepts both bare and `music/performance-intent`-qualified kind labels, and
    /// returns an error for non-map exprs or unknown kinds.
    pub fn from_expr(expr: &Expr) -> Result<Self> {
        let Expr::Map(entries) = expr else {
            return Err(Error::Eval("performance intent must be a map".to_owned()));
        };
        match symbol_field(entries, "kind")?.as_qualified_str().as_str() {
            "note-on" | "music/performance-intent/note-on" => Ok(Self::NoteOn {
                pitch: pitch_field(entries, "pitch")?,
                velocity: u8_field(entries, "velocity")?,
                channel: channel_field(entries, "channel")?,
            }),
            "note-off" | "music/performance-intent/note-off" => Ok(Self::NoteOff {
                pitch: pitch_field(entries, "pitch")?,
                velocity: u8_field(entries, "velocity")?,
                channel: channel_field(entries, "channel")?,
            }),
            "aftertouch" | "music/performance-intent/aftertouch" => Ok(Self::Aftertouch {
                pitch: pitch_field(entries, "pitch")?,
                pressure: u8_field(entries, "pressure")?,
                channel: channel_field(entries, "channel")?,
            }),
            "pitch-bend" | "music/performance-intent/pitch-bend" => Ok(Self::PitchBend {
                value: u16_field(entries, "value")?,
                channel: channel_field(entries, "channel")?,
            }),
            "sustain" | "music/performance-intent/sustain" => Ok(Self::Sustain {
                down: bool_field(entries, "down")?,
                channel: channel_field(entries, "channel")?,
            }),
            "parameter" | "music/performance-intent/parameter" => Ok(Self::Parameter {
                target: symbol_field(entries, "target")?.clone(),
                value: i64_field(entries, "value")?,
            }),
            "panic" | "music/performance-intent/panic" => Ok(Self::Panic),
            other => Err(Error::Eval(format!(
                "unknown performance intent kind {other}"
            ))),
        }
    }
}

fn pitch_expr(pitch: Pitch) -> Expr {
    Expr::String(pitch.semitone().to_string())
}

fn channel_expr(channel: Channel) -> Expr {
    Expr::String(channel.0.to_string())
}

fn field<'a>(entries: &'a [(Expr, Expr)], name: &str) -> Result<&'a Expr> {
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

fn string_field<'a>(entries: &'a [(Expr, Expr)], name: &str) -> Result<&'a str> {
    match field(entries, name)? {
        Expr::String(value) => Ok(value),
        _ => Err(Error::Eval(format!("{name} field must be text"))),
    }
}

fn symbol_field<'a>(entries: &'a [(Expr, Expr)], name: &str) -> Result<&'a Symbol> {
    match field(entries, name)? {
        Expr::Symbol(value) => Ok(value),
        _ => Err(Error::Eval(format!("{name} field must be a symbol"))),
    }
}

fn bool_field(entries: &[(Expr, Expr)], name: &str) -> Result<bool> {
    match field(entries, name)? {
        Expr::Bool(value) => Ok(*value),
        _ => Err(Error::Eval(format!("{name} field must be a boolean"))),
    }
}

fn i64_field(entries: &[(Expr, Expr)], name: &str) -> Result<i64> {
    string_field(entries, name)?
        .parse::<i64>()
        .map_err(|err| Error::Eval(format!("invalid {name}: {err}")))
}

fn i32_field(entries: &[(Expr, Expr)], name: &str) -> Result<i32> {
    string_field(entries, name)?
        .parse::<i32>()
        .map_err(|err| Error::Eval(format!("invalid {name}: {err}")))
}

fn u8_field(entries: &[(Expr, Expr)], name: &str) -> Result<u8> {
    string_field(entries, name)?
        .parse::<u8>()
        .map_err(|err| Error::Eval(format!("invalid {name}: {err}")))
}

fn u16_field(entries: &[(Expr, Expr)], name: &str) -> Result<u16> {
    string_field(entries, name)?
        .parse::<u16>()
        .map_err(|err| Error::Eval(format!("invalid {name}: {err}")))
}

fn pitch_field(entries: &[(Expr, Expr)], name: &str) -> Result<Pitch> {
    Ok(Pitch::from_semitone(i32_field(entries, name)?))
}

fn channel_field(entries: &[(Expr, Expr)], name: &str) -> Result<Channel> {
    Channel::new(u8_field(entries, name)?).map_err(|err| Error::Eval(err.to_string()))
}
