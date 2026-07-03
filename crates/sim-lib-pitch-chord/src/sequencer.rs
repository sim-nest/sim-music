use sim_lib_pitch_core::{Pitch, PitchClass, parse_pitch};
use sim_lib_pitch_namer_roman::label_roman;
use sim_lib_pitch_scale::{Key, Mode};

use crate::{
    ChordSymbol, HarmonicSuggestion, HarmonicSuggestionContext, PitchChordError, VelocityPolicy,
    VoicingPolicy, suggest_harmony,
};

/// A single step in a chord progression: a chord symbol with voicing, duration,
/// optional trigger, and velocity policy.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ChordSequencerSlot {
    /// The chord sounded by this slot.
    pub chord: ChordSymbol,
    /// The voicing applied to the chord.
    pub voicing: VoicingPolicy,
    /// The slot's duration in ticks.
    pub duration_ticks: u32,
    /// An optional trigger pitch that selects this slot in live play.
    pub trigger: Option<Pitch>,
    /// The velocity policy applied to the slot's notes.
    pub velocity: VelocityPolicy,
}

impl ChordSequencerSlot {
    /// Constructs a slot, rejecting a zero `duration_ticks` with
    /// [`PitchChordError::InvalidSlotDuration`].
    pub fn new(
        chord: ChordSymbol,
        voicing: VoicingPolicy,
        duration_ticks: u32,
    ) -> Result<Self, PitchChordError> {
        if duration_ticks == 0 {
            return Err(PitchChordError::InvalidSlotDuration);
        }
        Ok(Self {
            chord,
            voicing,
            duration_ticks,
            trigger: None,
            velocity: VelocityPolicy::Preserve,
        })
    }

    /// Sets the trigger pitch used to select this slot in live play.
    pub fn with_trigger(mut self, trigger: Pitch) -> Self {
        self.trigger = Some(trigger);
        self
    }

    /// Sets the slot's velocity policy.
    pub fn with_velocity(mut self, velocity: VelocityPolicy) -> Self {
        self.velocity = velocity;
        self
    }
}

/// Configuration for a chord progression: a key, an ordered list of slots, and
/// render defaults.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ChordSequencerConfig {
    /// The key used for roman-numeral analysis and harmony suggestions.
    pub key: Key,
    /// The ordered progression slots.
    pub slots: Vec<ChordSequencerSlot>,
    /// The octave at which chord roots are voiced.
    pub root_octave: i16,
    /// The default velocity used when a slot does not override it.
    pub default_velocity: u8,
    /// The maximum number of harmony suggestions produced per slot.
    pub suggestion_limit: usize,
}

impl ChordSequencerConfig {
    /// Constructs a configuration from a `key` and `slots`, validating the slots.
    pub fn new(key: Key, slots: Vec<ChordSequencerSlot>) -> Result<Self, PitchChordError> {
        validate_slots(&slots)?;
        Ok(Self {
            key,
            slots,
            root_octave: 4,
            default_velocity: 96,
            suggestion_limit: 6,
        })
    }

    /// Sets the octave at which chord roots are voiced.
    pub fn with_root_octave(mut self, root_octave: i16) -> Self {
        self.root_octave = root_octave;
        self
    }

    /// Sets the default velocity, clamped to `1..=127`.
    pub fn with_default_velocity(mut self, default_velocity: u8) -> Self {
        self.default_velocity = default_velocity.clamp(1, 127);
        self
    }

    /// Sets the maximum number of harmony suggestions produced per slot.
    pub fn with_suggestion_limit(mut self, suggestion_limit: usize) -> Self {
        self.suggestion_limit = suggestion_limit;
        self
    }

    /// Serializes the configuration to its `chord-seq-v1` wire string.
    pub fn to_wire(&self) -> String {
        let slots = self
            .slots
            .iter()
            .map(slot_to_wire)
            .collect::<Vec<_>>()
            .join(";");
        format!(
            "chord-seq-v1|key={}:{}|octave={}|velocity={}|suggestions={}|slots={}",
            self.key.tonic.canonical_name(),
            self.key.mode.name(),
            self.root_octave,
            self.default_velocity,
            self.suggestion_limit,
            slots
        )
    }

    /// Parses a `chord-seq-v1` wire string back into a configuration.
    ///
    /// Returns [`PitchChordError::InvalidProgressionWire`] on malformed input.
    pub fn from_wire(value: &str) -> Result<Self, PitchChordError> {
        let mut key = None;
        let mut root_octave = 4;
        let mut default_velocity = 96;
        let mut suggestion_limit = 6;
        let mut slots = None;
        let mut fields = value.split('|');
        if fields.next() != Some("chord-seq-v1") {
            return Err(PitchChordError::InvalidProgressionWire);
        }
        for field in fields {
            let (name, data) = field
                .split_once('=')
                .ok_or(PitchChordError::InvalidProgressionWire)?;
            match name {
                "key" => key = Some(key_from_wire(data)?),
                "octave" => {
                    root_octave = data
                        .parse::<i16>()
                        .map_err(|_| PitchChordError::InvalidProgressionWire)?;
                }
                "velocity" => {
                    default_velocity = data
                        .parse::<u8>()
                        .map_err(|_| PitchChordError::InvalidProgressionWire)?
                        .clamp(1, 127);
                }
                "suggestions" => {
                    suggestion_limit = data
                        .parse::<usize>()
                        .map_err(|_| PitchChordError::InvalidProgressionWire)?;
                }
                "slots" => slots = Some(slots_from_wire(data)?),
                _ => return Err(PitchChordError::InvalidProgressionWire),
            }
        }
        let slots = slots.ok_or(PitchChordError::InvalidProgressionWire)?;
        validate_slots(&slots)?;
        Ok(Self {
            key: key.ok_or(PitchChordError::InvalidProgressionWire)?,
            slots,
            root_octave,
            default_velocity,
            suggestion_limit,
        })
    }

    /// Consumes the configuration and constructs a [`ChordSequencerPlayer`].
    pub fn player(self) -> Result<ChordSequencerPlayer, PitchChordError> {
        ChordSequencerPlayer::new(self)
    }
}

/// An input note that can trigger a sequencer slot in live play.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct ChordSequenceInput {
    /// The played pitch.
    pub pitch: Pitch,
    /// The played velocity, clamped to `1..=127`.
    pub velocity: u8,
}

impl ChordSequenceInput {
    /// Constructs an input note, clamping `velocity` to `1..=127`.
    pub fn new(pitch: Pitch, velocity: u8) -> Self {
        Self {
            pitch,
            velocity: velocity.clamp(1, 127),
        }
    }
}

/// A single rendered note within a chord-sequence event.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ChordSequenceNote {
    /// The note's pitch.
    pub pitch: Pitch,
    /// The note's velocity.
    pub velocity: u8,
    /// The tick at which the note starts.
    pub start_tick: u32,
    /// The note's duration in ticks.
    pub duration_ticks: u32,
}

/// A rendered progression event: one slot's chord realized as timed notes, with
/// its roman-numeral label and harmony suggestions.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ChordSequenceEvent {
    /// The index of the slot that produced this event.
    pub slot_index: usize,
    /// The tick at which the event starts.
    pub start_tick: u32,
    /// The event's duration in ticks.
    pub duration_ticks: u32,
    /// The trigger pitch, if the event was produced by a live trigger.
    pub trigger: Option<Pitch>,
    /// The chord symbol for the slot.
    pub chord: ChordSymbol,
    /// The roman-numeral label of the chord in the configured key.
    pub roman: String,
    /// The rendered notes.
    pub notes: Vec<ChordSequenceNote>,
    /// Suggested follow-on chords.
    pub suggestions: Vec<HarmonicSuggestion>,
}

/// The full rendered progression: a list of events and the total duration.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ChordSequenceRender {
    /// The rendered events in order.
    pub events: Vec<ChordSequenceEvent>,
    /// The total length of the progression in ticks.
    pub total_ticks: u32,
}

/// Renders a [`ChordSequencerConfig`] into timed chord events and answers live
/// triggers.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ChordSequencerPlayer {
    config: ChordSequencerConfig,
}

impl ChordSequencerPlayer {
    /// Builds a player from `config`, validating its slots.
    pub fn new(config: ChordSequencerConfig) -> Result<Self, PitchChordError> {
        validate_slots(&config.slots)?;
        Ok(Self { config })
    }

    /// Returns the player's configuration.
    pub fn config(&self) -> &ChordSequencerConfig {
        &self.config
    }

    /// Renders the whole progression into a sequence of timed events.
    pub fn render_progression(&self) -> ChordSequenceRender {
        let mut start_tick = 0u32;
        let events = self
            .config
            .slots
            .iter()
            .enumerate()
            .map(|(index, slot)| {
                let event = self.render_slot(index, start_tick, None);
                start_tick = start_tick.saturating_add(slot.duration_ticks);
                event
            })
            .collect();
        ChordSequenceRender {
            events,
            total_ticks: start_tick,
        }
    }

    /// Returns the slot index that `pitch` triggers, matching explicit triggers
    /// first and otherwise falling back to the pitch's scale degree.
    pub fn slot_index_for_trigger(&self, pitch: Pitch) -> Option<usize> {
        self.config
            .slots
            .iter()
            .position(|slot| {
                slot.trigger
                    .is_some_and(|trigger| trigger.class == pitch.class)
            })
            .or_else(|| {
                let scale =
                    sim_lib_pitch_scale::Scale::new(self.config.key.tonic, self.config.key.mode);
                scale
                    .degree_of(pitch.class)
                    .map(|degree| (degree - 1) % self.config.slots.len())
            })
    }

    /// Renders the event for the slot that `input` triggers, or `None` if no slot
    /// matches.
    pub fn trigger(&self, input: ChordSequenceInput) -> Option<ChordSequenceEvent> {
        let slot_index = self.slot_index_for_trigger(input.pitch)?;
        Some(self.render_slot(slot_index, 0, Some(input)))
    }

    /// Returns harmony suggestions for what could follow the chord at `slot_index`.
    pub fn suggest_next(&self, slot_index: usize) -> Vec<HarmonicSuggestion> {
        self.config
            .slots
            .get(slot_index)
            .map(|slot| {
                suggest_harmony(
                    HarmonicSuggestionContext::new(self.config.key, slot.chord.clone())
                        .with_max_candidates(self.config.suggestion_limit),
                )
            })
            .unwrap_or_default()
    }

    fn render_slot(
        &self,
        slot_index: usize,
        start_tick: u32,
        input: Option<ChordSequenceInput>,
    ) -> ChordSequenceEvent {
        let slot = &self.config.slots[slot_index];
        let chord = slot.chord.to_chord(self.config.root_octave);
        let velocity = slot.velocity.apply(
            input
                .map(|input| input.velocity)
                .unwrap_or(self.config.default_velocity),
        );
        let notes = slot
            .voicing
            .apply(chord.pitches())
            .into_iter()
            .map(|pitch| ChordSequenceNote {
                pitch,
                velocity,
                start_tick,
                duration_ticks: slot.duration_ticks,
            })
            .collect();
        ChordSequenceEvent {
            slot_index,
            start_tick,
            duration_ticks: slot.duration_ticks,
            trigger: input.map(|input| input.pitch),
            chord: slot.chord.clone(),
            roman: roman_label(self.config.key, &slot.chord),
            notes,
            suggestions: self.suggest_next(slot_index),
        }
    }
}

fn validate_slots(slots: &[ChordSequencerSlot]) -> Result<(), PitchChordError> {
    if slots.is_empty() {
        return Err(PitchChordError::EmptyProgression);
    }
    if slots.iter().any(|slot| slot.duration_ticks == 0) {
        return Err(PitchChordError::InvalidSlotDuration);
    }
    Ok(())
}

fn roman_label(key: Key, chord: &ChordSymbol) -> String {
    label_roman(
        chord.to_chord(4).pitch_classes(),
        Some(key),
        Some(chord.root),
    )
    .unwrap_or_else(|_| chord.wire_label())
}

fn slot_to_wire(slot: &ChordSequencerSlot) -> String {
    format!(
        "{},{},{},{},{}",
        slot.chord.wire_label(),
        voicing_to_wire(slot.voicing),
        slot.duration_ticks,
        trigger_to_wire(slot.trigger),
        velocity_to_wire(slot.velocity)
    )
}

fn slots_from_wire(value: &str) -> Result<Vec<ChordSequencerSlot>, PitchChordError> {
    if value.is_empty() {
        return Err(PitchChordError::EmptyProgression);
    }
    value.split(';').map(slot_from_wire).collect()
}

fn slot_from_wire(value: &str) -> Result<ChordSequencerSlot, PitchChordError> {
    let parts = value.split(',').collect::<Vec<_>>();
    if parts.len() != 5 {
        return Err(PitchChordError::InvalidProgressionWire);
    }
    let chord = ChordSymbol::parse(parts[0])?;
    let voicing = voicing_from_wire(parts[1])?;
    let duration_ticks = parts[2]
        .parse::<u32>()
        .map_err(|_| PitchChordError::InvalidProgressionWire)?;
    let mut slot = ChordSequencerSlot::new(chord, voicing, duration_ticks)?;
    slot.trigger = trigger_from_wire(parts[3])?;
    slot.velocity = velocity_from_wire(parts[4])?;
    Ok(slot)
}

fn key_from_wire(value: &str) -> Result<Key, PitchChordError> {
    let (tonic, mode) = value
        .split_once(':')
        .ok_or(PitchChordError::InvalidProgressionWire)?;
    Ok(Key {
        tonic: pitch_class_from_wire(tonic)?,
        mode: mode_from_wire(mode)?,
    })
}

fn pitch_class_from_wire(value: &str) -> Result<PitchClass, PitchChordError> {
    parse_pitch(&format!("{value}4"))
        .map(|pitch| pitch.class)
        .map_err(|_| PitchChordError::InvalidProgressionWire)
}

fn mode_from_wire(value: &str) -> Result<Mode, PitchChordError> {
    match value {
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
        _ => Err(PitchChordError::InvalidProgressionWire),
    }
}

fn voicing_to_wire(voicing: VoicingPolicy) -> String {
    match voicing {
        VoicingPolicy::Closed => "closed".to_owned(),
        VoicingPolicy::Open { spread } => format!("open:{spread}"),
        VoicingPolicy::Drop {
            voice_index_from_top,
            octaves,
        } => format!("drop:{voice_index_from_top}:{octaves}"),
    }
}

fn voicing_from_wire(value: &str) -> Result<VoicingPolicy, PitchChordError> {
    if value == "closed" {
        return Ok(VoicingPolicy::Closed);
    }
    if let Some(spread) = value.strip_prefix("open:") {
        return Ok(VoicingPolicy::Open {
            spread: spread
                .parse::<i32>()
                .map_err(|_| PitchChordError::InvalidProgressionWire)?,
        });
    }
    if let Some(rest) = value.strip_prefix("drop:") {
        let (voice, octaves) = rest
            .split_once(':')
            .ok_or(PitchChordError::InvalidProgressionWire)?;
        return Ok(VoicingPolicy::Drop {
            voice_index_from_top: voice
                .parse::<usize>()
                .map_err(|_| PitchChordError::InvalidProgressionWire)?,
            octaves: octaves
                .parse::<i16>()
                .map_err(|_| PitchChordError::InvalidProgressionWire)?,
        });
    }
    Err(PitchChordError::InvalidProgressionWire)
}

fn trigger_to_wire(trigger: Option<Pitch>) -> String {
    trigger
        .map(|pitch| pitch.semitone().to_string())
        .unwrap_or_else(|| "-".to_owned())
}

fn trigger_from_wire(value: &str) -> Result<Option<Pitch>, PitchChordError> {
    if value == "-" {
        return Ok(None);
    }
    value
        .parse::<i32>()
        .map(Pitch::from_semitone)
        .map(Some)
        .map_err(|_| PitchChordError::InvalidProgressionWire)
}

fn velocity_to_wire(velocity: VelocityPolicy) -> String {
    match velocity {
        VelocityPolicy::Preserve => "preserve".to_owned(),
        VelocityPolicy::Fixed(value) => format!("fixed:{value}"),
        VelocityPolicy::Offset(offset) => format!("offset:{offset}"),
    }
}

fn velocity_from_wire(value: &str) -> Result<VelocityPolicy, PitchChordError> {
    if value == "preserve" {
        return Ok(VelocityPolicy::Preserve);
    }
    if let Some(value) = value.strip_prefix("fixed:") {
        return Ok(VelocityPolicy::Fixed(
            value
                .parse::<u8>()
                .map_err(|_| PitchChordError::InvalidProgressionWire)?,
        ));
    }
    if let Some(value) = value.strip_prefix("offset:") {
        return Ok(VelocityPolicy::Offset(
            value
                .parse::<i16>()
                .map_err(|_| PitchChordError::InvalidProgressionWire)?,
        ));
    }
    Err(PitchChordError::InvalidProgressionWire)
}
