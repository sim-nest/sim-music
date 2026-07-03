use num_rational::Ratio;
use sim_kernel::{Expr, Result as KernelResult, Symbol};
use sim_lib_midi_core::{Channel, MidiEvent, U7, U14};

use crate::model::ensure_non_negative;
use crate::{
    Articulation, LaneId, LaneKind, MusicError, Note, NoteEvent, PerformanceTake, Pitch, Time,
};

/// Timing grid for a piano roll.
///
/// Couples a ticks-per-quarter resolution with a quantization step measured in
/// whole-note fractions.
///
/// # Examples
///
/// ```
/// use sim_lib_music_core::TimeGrid;
///
/// let grid = TimeGrid::default();
/// assert_eq!(grid.tpq, 480);
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TimeGrid {
    /// Ticks per quarter note.
    pub tpq: u32,
    /// Quantization step, as a fraction of a whole note.
    pub step: Time,
}

impl TimeGrid {
    /// Builds a grid, rejecting a zero `tpq` or a non-positive `step`.
    ///
    /// Returns `MusicError::InvalidPianoRollGrid` when the inputs are invalid.
    pub fn new(tpq: u32, step: Time) -> Result<Self, MusicError> {
        if tpq == 0 || step <= Time::from_integer(0) {
            return Err(MusicError::InvalidPianoRollGrid);
        }
        Ok(Self { tpq, step })
    }
}

impl Default for TimeGrid {
    fn default() -> Self {
        Self {
            tpq: 480,
            step: Ratio::new(1, 16),
        }
    }
}

/// A note placed at an absolute onset time.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TimedNote {
    /// Absolute onset time of the note.
    pub onset: Time,
    /// The note sounded at `onset`.
    pub note: Note,
}

/// A drum hit cell addressed by MIDI key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DrumCell {
    /// Onset time of the hit.
    pub onset: Time,
    /// Sounding duration of the hit.
    pub duration: Time,
    /// MIDI key (drum voice) struck.
    pub key: U7,
    /// Strike velocity.
    pub velocity: U7,
    /// MIDI channel of the hit.
    pub channel: Channel,
}

/// A note addressed by scale degree and octave rather than absolute pitch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScaleDegreeCell {
    /// Onset time of the cell.
    pub onset: Time,
    /// Sounding duration of the cell.
    pub duration: Time,
    /// Scale degree, relative to the prevailing scale.
    pub degree: i16,
    /// Octave offset applied to the degree.
    pub octave: i8,
    /// Strike velocity.
    pub velocity: U7,
    /// MIDI channel of the cell.
    pub channel: Channel,
}

/// A cell that places a named runtime object on a lane.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ObjectCell {
    /// Onset time of the object.
    pub onset: Time,
    /// Duration the object occupies.
    pub duration: Time,
    /// Symbol naming the placed object.
    pub object: Symbol,
}

/// An automation breakpoint targeting a named parameter.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AutomationCell {
    /// Time of the breakpoint.
    pub time: Time,
    /// Symbol naming the automation target.
    pub target: Symbol,
    /// Value applied at `time`.
    pub value: i64,
}

/// A MIDI control-change cell.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ControlChangeCell {
    /// Time of the control change.
    pub time: Time,
    /// MIDI channel affected.
    pub channel: Channel,
    /// Controller number.
    pub controller: U7,
    /// New controller value.
    pub value: U7,
}

/// A MIDI pitch-bend cell.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PitchBendCell {
    /// Time of the bend.
    pub time: Time,
    /// MIDI channel affected.
    pub channel: Channel,
    /// 14-bit bend value.
    pub value: U14,
}

/// A MIDI polyphonic key-pressure cell.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PolyPressureCell {
    /// Time of the pressure event.
    pub time: Time,
    /// MIDI channel affected.
    pub channel: Channel,
    /// Key the pressure applies to.
    pub key: U7,
    /// Pressure amount.
    pub pressure: U7,
}

/// A MIDI channel-pressure (aftertouch) cell.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChannelPressureCell {
    /// Time of the pressure event.
    pub time: Time,
    /// MIDI channel affected.
    pub channel: Channel,
    /// Pressure amount applied to the whole channel.
    pub pressure: U7,
}

/// A single placed event in a piano roll lane.
///
/// Each variant carries the cell payload appropriate to its lane kind.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PianoRollCell {
    /// A pitched note.
    Note(TimedNote),
    /// A drum hit.
    Drum(DrumCell),
    /// A scale-degree note.
    ScaleDegree(ScaleDegreeCell),
    /// A placed runtime object.
    Object(ObjectCell),
    /// An automation breakpoint.
    Automation(AutomationCell),
    /// A MIDI control change.
    ControlChange(ControlChangeCell),
    /// A MIDI pitch bend.
    PitchBend(PitchBendCell),
    /// A MIDI polyphonic key pressure.
    PolyPressure(PolyPressureCell),
    /// A MIDI channel pressure.
    ChannelPressure(ChannelPressureCell),
    /// A raw MIDI event.
    Midi(MidiEvent),
}

impl PianoRollCell {
    /// Returns the cell's time position on its lane.
    pub fn time(&self) -> Time {
        match self {
            Self::Note(cell) => cell.onset,
            Self::Drum(cell) => cell.onset,
            Self::ScaleDegree(cell) => cell.onset,
            Self::Object(cell) => cell.onset,
            Self::Automation(cell) => cell.time,
            Self::ControlChange(cell) => cell.time,
            Self::PitchBend(cell) => cell.time,
            Self::PolyPressure(cell) => cell.time,
            Self::ChannelPressure(cell) => cell.time,
            Self::Midi(event) => tick_time_to_time(event.time),
        }
    }

    /// Returns the lane kind this cell belongs on.
    pub fn lane_kind(&self) -> LaneKind {
        match self {
            Self::Note(_) => LaneKind::Note,
            Self::Drum(_) => LaneKind::Drum,
            Self::ScaleDegree(_) => LaneKind::ScaleDegree,
            Self::Object(_) => LaneKind::Object,
            Self::Automation(_) => LaneKind::Automation,
            Self::ControlChange(_)
            | Self::PitchBend(_)
            | Self::PolyPressure(_)
            | Self::ChannelPressure(_) => LaneKind::Control,
            Self::Midi(_) => LaneKind::Midi,
        }
    }

    /// Returns the wire label naming this cell's variant.
    pub fn kind_label(&self) -> &'static str {
        match self {
            Self::Note(_) => "note",
            Self::Drum(_) => "drum",
            Self::ScaleDegree(_) => "scale-degree",
            Self::Object(_) => "object",
            Self::Automation(_) => "automation",
            Self::ControlChange(_) => "control-change",
            Self::PitchBend(_) => "pitch-bend",
            Self::PolyPressure(_) => "poly-pressure",
            Self::ChannelPressure(_) => "channel-pressure",
            Self::Midi(_) => "midi",
        }
    }

    /// Renders the cell as its expression form for codecs and browse.
    pub fn to_expr(&self) -> Expr {
        match self {
            Self::Note(cell) => map(vec![
                ("kind", Expr::String("note".to_owned())),
                ("onset", time_expr(cell.onset)),
                ("duration", time_expr(cell.note.duration)),
                ("pitch", Expr::String(pitch_label(cell.note.pitch))),
                ("velocity", Expr::String(cell.note.velocity.to_string())),
                ("channel", Expr::String(cell.note.channel.0.to_string())),
            ]),
            Self::Drum(cell) => map(vec![
                ("kind", Expr::String("drum".to_owned())),
                ("onset", time_expr(cell.onset)),
                ("duration", time_expr(cell.duration)),
                ("key", Expr::String(cell.key.0.to_string())),
                ("velocity", Expr::String(cell.velocity.0.to_string())),
                ("channel", Expr::String(cell.channel.0.to_string())),
            ]),
            Self::ScaleDegree(cell) => map(vec![
                ("kind", Expr::String("scale-degree".to_owned())),
                ("onset", time_expr(cell.onset)),
                ("duration", time_expr(cell.duration)),
                ("degree", Expr::String(cell.degree.to_string())),
                ("octave", Expr::String(cell.octave.to_string())),
                ("velocity", Expr::String(cell.velocity.0.to_string())),
                ("channel", Expr::String(cell.channel.0.to_string())),
            ]),
            Self::Object(cell) => map(vec![
                ("kind", Expr::String("object".to_owned())),
                ("onset", time_expr(cell.onset)),
                ("duration", time_expr(cell.duration)),
                ("object", Expr::Symbol(cell.object.clone())),
            ]),
            Self::Automation(cell) => map(vec![
                ("kind", Expr::String("automation".to_owned())),
                ("time", time_expr(cell.time)),
                ("target", Expr::Symbol(cell.target.clone())),
                ("value", Expr::String(cell.value.to_string())),
            ]),
            Self::ControlChange(cell) => map(vec![
                ("kind", Expr::String("control-change".to_owned())),
                ("time", time_expr(cell.time)),
                ("channel", Expr::String(cell.channel.0.to_string())),
                ("controller", Expr::String(cell.controller.0.to_string())),
                ("value", Expr::String(cell.value.0.to_string())),
            ]),
            Self::PitchBend(cell) => map(vec![
                ("kind", Expr::String("pitch-bend".to_owned())),
                ("time", time_expr(cell.time)),
                ("channel", Expr::String(cell.channel.0.to_string())),
                ("value", Expr::String(cell.value.0.to_string())),
            ]),
            Self::PolyPressure(cell) => map(vec![
                ("kind", Expr::String("poly-pressure".to_owned())),
                ("time", time_expr(cell.time)),
                ("channel", Expr::String(cell.channel.0.to_string())),
                ("key", Expr::String(cell.key.0.to_string())),
                ("pressure", Expr::String(cell.pressure.0.to_string())),
            ]),
            Self::ChannelPressure(cell) => map(vec![
                ("kind", Expr::String("channel-pressure".to_owned())),
                ("time", time_expr(cell.time)),
                ("channel", Expr::String(cell.channel.0.to_string())),
                ("pressure", Expr::String(cell.pressure.0.to_string())),
            ]),
            Self::Midi(event) => map(vec![
                ("kind", Expr::String("midi".to_owned())),
                ("time", time_expr(tick_time_to_time(event.time))),
                ("payload", Expr::String(format!("{:?}", event.payload))),
            ]),
        }
    }
}

/// A single lane of a piano roll holding cells of one kind.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PianoRollLane {
    /// Identifier of the lane.
    pub id: LaneId,
    /// Kind of cell the lane carries.
    pub kind: LaneKind,
    /// Cells on the lane, kept in stable time order.
    pub cells: Vec<PianoRollCell>,
}

impl PianoRollLane {
    /// Builds a lane, validating cell kinds and times then ordering the cells.
    ///
    /// Every cell must match `kind` and carry non-negative timing; otherwise a
    /// `MusicError` is returned. The cells are sorted into stable order.
    pub fn new(
        id: LaneId,
        kind: LaneKind,
        mut cells: Vec<PianoRollCell>,
    ) -> Result<Self, MusicError> {
        for cell in &cells {
            if cell.lane_kind() != kind {
                return Err(MusicError::PianoRollLaneCellMismatch {
                    lane: id.0.clone(),
                    lane_kind: kind.wire_label().to_owned(),
                    cell_kind: cell.kind_label().to_owned(),
                });
            }
            validate_cell_time(cell)?;
        }
        stable_cell_order(&mut cells);
        Ok(Self { id, kind, cells })
    }

    /// Renders the lane and its cells as an expression.
    pub fn to_expr(&self) -> Expr {
        map(vec![
            ("id", Expr::String(self.id.0.clone())),
            ("kind", Expr::Symbol(self.kind.symbol())),
            (
                "cells",
                Expr::List(self.cells.iter().map(PianoRollCell::to_expr).collect()),
            ),
        ])
    }
}

/// A piano roll: a set of timed lanes over a shared timing grid.
///
/// Holds the note projection in `items` alongside the full `lanes`, all keyed
/// to a single [`TimeGrid`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PianoRoll {
    /// Note projection across all lanes, in stable order.
    pub items: Vec<TimedNote>,
    /// Lanes making up the roll, sorted by id then kind.
    pub lanes: Vec<PianoRollLane>,
    /// Timing grid shared by every lane.
    pub time: TimeGrid,
}

impl PianoRoll {
    /// Builds a roll from notes, placing them on a single note lane.
    ///
    /// Uses the default [`TimeGrid`]; an empty input yields a roll with no lanes.
    pub fn new(items: Vec<TimedNote>) -> Result<Self, MusicError> {
        let cells = items
            .into_iter()
            .map(PianoRollCell::Note)
            .collect::<Vec<_>>();
        let lanes = if cells.is_empty() {
            Vec::new()
        } else {
            vec![PianoRollLane::new(
                LaneId::new("notes"),
                LaneKind::Note,
                cells,
            )?]
        };
        Self::from_lanes_with_time(lanes, TimeGrid::default())
    }

    /// Builds a roll from prepared lanes using the default [`TimeGrid`].
    pub fn from_lanes(lanes: Vec<PianoRollLane>) -> Result<Self, MusicError> {
        Self::from_lanes_with_time(lanes, TimeGrid::default())
    }

    /// Builds a roll from prepared lanes over an explicit timing grid.
    ///
    /// Validates `time`, sorts the lanes, and derives the note projection in
    /// stable order.
    pub fn from_lanes_with_time(
        mut lanes: Vec<PianoRollLane>,
        time: TimeGrid,
    ) -> Result<Self, MusicError> {
        TimeGrid::new(time.tpq, time.step)?;
        lanes.sort_by(|left, right| {
            left.id
                .cmp(&right.id)
                .then_with(|| left.kind.cmp(&right.kind))
        });
        let mut items = lanes
            .iter()
            .flat_map(|lane| lane.cells.iter())
            .filter_map(cell_note)
            .collect::<Vec<_>>();
        stable_note_order(&mut items);
        Ok(Self { items, lanes, time })
    }

    /// Builds a roll from note events on a single performance note lane.
    ///
    /// Converts each event's tick timing to grid time with normal articulation.
    pub fn from_note_events(events: Vec<NoteEvent>) -> Result<Self, MusicError> {
        let cells = events
            .into_iter()
            .map(|event| {
                PianoRollCell::Note(TimedNote {
                    onset: tick_time_to_time(event.time),
                    note: Note {
                        duration: tick_time_to_time(event.duration),
                        pitch: event.pitch,
                        velocity: event.velocity,
                        channel: event.channel,
                        articulation: Articulation::Normal,
                    },
                })
            })
            .collect::<Vec<_>>();
        Self::from_lanes(vec![PianoRollLane::new(
            LaneId::new("performance-notes"),
            LaneKind::Note,
            cells,
        )?])
    }

    /// Builds a roll from a performance take's extracted note events.
    ///
    /// Surfaces extraction or validation failures as a kernel evaluation error.
    pub fn from_performance_take(take: &PerformanceTake) -> KernelResult<Self> {
        let note_events = take.note_events()?;
        Self::from_note_events(note_events)
            .map_err(|err| sim_kernel::Error::Eval(format!("invalid piano-roll take: {err}")))
    }

    /// Iterates over every cell across all lanes.
    pub fn cells(&self) -> impl Iterator<Item = &PianoRollCell> {
        self.lanes.iter().flat_map(|lane| lane.cells.iter())
    }

    /// Renders the roll, its grid, and its lanes as an expression.
    pub fn to_expr(&self) -> Expr {
        map(vec![
            (
                "object",
                Expr::Symbol(Symbol::qualified("music", "PianoRoll")),
            ),
            ("tpq", Expr::String(self.time.tpq.to_string())),
            ("step", time_expr(self.time.step)),
            (
                "lanes",
                Expr::List(self.lanes.iter().map(PianoRollLane::to_expr).collect()),
            ),
        ])
    }
}

fn stable_note_order(items: &mut [TimedNote]) {
    items.sort_by(|left, right| {
        left.onset
            .cmp(&right.onset)
            .then_with(|| left.note.pitch.semitone().cmp(&right.note.pitch.semitone()))
            .then_with(|| left.note.channel.0.cmp(&right.note.channel.0))
    });
}

fn stable_cell_order(cells: &mut [PianoRollCell]) {
    cells.sort_by(|left, right| {
        left.time()
            .cmp(&right.time())
            .then_with(|| left.kind_label().cmp(right.kind_label()))
    });
}

fn validate_cell_time(cell: &PianoRollCell) -> Result<(), MusicError> {
    ensure_non_negative(cell.time())?;
    match cell {
        PianoRollCell::Note(cell) => ensure_non_negative(cell.note.duration),
        PianoRollCell::Drum(cell) => ensure_non_negative(cell.duration),
        PianoRollCell::ScaleDegree(cell) => ensure_non_negative(cell.duration),
        PianoRollCell::Object(cell) => ensure_non_negative(cell.duration),
        PianoRollCell::Automation(_)
        | PianoRollCell::ControlChange(_)
        | PianoRollCell::PitchBend(_)
        | PianoRollCell::PolyPressure(_)
        | PianoRollCell::ChannelPressure(_)
        | PianoRollCell::Midi(_) => Ok(()),
    }
}

fn cell_note(cell: &PianoRollCell) -> Option<TimedNote> {
    match cell {
        PianoRollCell::Note(cell) => Some(cell.clone()),
        PianoRollCell::Drum(cell) => Some(TimedNote {
            onset: cell.onset,
            note: Note {
                duration: cell.duration,
                pitch: Pitch::from_midi(cell.key.0),
                velocity: cell.velocity.0.max(1),
                channel: cell.channel,
                articulation: Articulation::Normal,
            },
        }),
        _ => None,
    }
}

fn tick_time_to_time(time: sim_lib_midi_core::TickTime) -> Time {
    Ratio::new(time.ticks, i64::from(time.tpq) * 4)
}

fn time_expr(time: Time) -> Expr {
    map(vec![
        ("numer", Expr::String(time.numer().to_string())),
        ("denom", Expr::String(time.denom().to_string())),
    ])
}

fn pitch_label(pitch: Pitch) -> String {
    pitch
        .to_midi()
        .map(|midi| format!("midi:{midi}"))
        .unwrap_or_else(|| format!("semitone:{}", pitch.semitone()))
}

fn map(entries: Vec<(&'static str, Expr)>) -> Expr {
    Expr::Map(
        entries
            .into_iter()
            .map(|(key, value)| (Expr::Symbol(Symbol::new(key)), value))
            .collect(),
    )
}
