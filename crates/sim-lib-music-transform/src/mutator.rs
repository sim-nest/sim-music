use std::collections::{BTreeMap, BTreeSet};
use std::num::ParseIntError;

use sim_lib_music_core::{Music, MusicObject, Time, TimedNote};
use sim_lib_pitch_core::{Pitch, PitchClass};
use sim_lib_pitch_scale::{Mode, Scale};
use thiserror::Error;

use crate::{
    RetrogradeMode, canonical_roll, chord_tones_in, pitch_invert, retrograde_with_mode,
    to_piano_roll, transpose,
};

/// Error raised while parsing or validating a pattern mutator.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum PatternMutatorError {
    /// The wire string was not a valid pattern mutator encoding.
    #[error("invalid pattern mutator wire format")]
    InvalidWire,
    /// A numeric field could not be parsed.
    #[error("invalid pattern mutator number")]
    InvalidNumber,
    /// A scale mode name was not recognized.
    #[error("invalid pattern mutator mode: {0}")]
    InvalidMode(String),
    /// A pitch class value was out of range.
    #[error("invalid pattern mutator pitch class: {0}")]
    InvalidPitchClass(u8),
}

/// Set of source note indices held fixed (locked) during mutation.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PatternLockSet {
    note_indices: BTreeSet<usize>,
}

impl PatternLockSet {
    /// Builds a lock set from a collection of source note indices.
    pub fn from_note_indices(indices: impl IntoIterator<Item = usize>) -> Self {
        Self {
            note_indices: indices.into_iter().collect(),
        }
    }

    /// Returns whether the given source index is locked.
    pub fn contains(&self, index: usize) -> bool {
        self.note_indices.contains(&index)
    }

    /// Returns the set of locked source note indices.
    pub fn note_indices(&self) -> &BTreeSet<usize> {
        &self.note_indices
    }
}

/// A single mutation operation applied to a pattern's notes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MutationOp {
    /// Reverse note onsets within the pattern span.
    Reverse,
    /// Rotate notes across their distinct onset slots by `steps`.
    Rotate {
        /// Number of slots to rotate (signed).
        steps: i32,
    },
    /// Transpose unlocked notes by `semitones`.
    Transpose {
        /// Semitone offset.
        semitones: i32,
    },
    /// Invert unlocked notes about `axis`.
    Invert {
        /// Inversion axis pitch.
        axis: Pitch,
    },
    /// Shuffle note onsets within each beat-sized bucket.
    ShuffleWithinBeat {
        /// Bucket width in beats.
        beat: Time,
    },
    /// Randomly drop notes, keeping roughly `keep_percent` of them.
    Thin {
        /// Target percentage of notes to keep.
        keep_percent: u8,
    },
    /// Duplicate notes transposed by `semitones` to thicken the texture.
    Thicken {
        /// Semitone offset of the added copies.
        semitones: i32,
    },
    /// Remap velocities into the `[low, high]` range.
    VelocityRemap {
        /// Lower velocity bound.
        low: u8,
        /// Upper velocity bound.
        high: u8,
    },
    /// Displace note onsets forward or backward by `offset`.
    RhythmDisplace {
        /// Displacement magnitude.
        offset: Time,
    },
    /// Conform note pitches to the nearest tone of `scale`.
    ScaleConform {
        /// Scale notes are conformed to.
        scale: Scale,
    },
}

/// Configuration describing a sequence of pattern mutations and their controls.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PatternMutatorConfig {
    /// Operations applied in order.
    pub operations: Vec<MutationOp>,
    /// Strength of each operation, from 0 to 100.
    pub amount: u8,
    /// Seed for the deterministic pseudo-random generator.
    pub seed: u64,
    /// Source notes held fixed across all operations.
    pub locks: PatternLockSet,
}

impl PatternMutatorConfig {
    /// Builds a config from operations with default amount, seed, and locks.
    pub fn new(operations: Vec<MutationOp>) -> Self {
        Self {
            operations,
            amount: 100,
            seed: 0,
            locks: PatternLockSet::default(),
        }
    }

    /// Sets the mutation strength, clamped to at most 100.
    pub fn with_amount(mut self, amount: u8) -> Self {
        self.amount = amount.min(100);
        self
    }

    /// Sets the random seed.
    pub fn with_seed(mut self, seed: u64) -> Self {
        self.seed = seed;
        self
    }

    /// Sets the locked note set.
    pub fn with_locks(mut self, locks: PatternLockSet) -> Self {
        self.locks = locks;
        self
    }

    /// Applies the configured mutations to the input and returns the result.
    pub fn apply(&self, object: &dyn MusicObject) -> Music {
        mutate_pattern(object, self)
    }

    /// Serializes this config to its `pattern-mutator|...` wire string.
    pub fn to_wire(&self) -> String {
        let locks = self
            .locks
            .note_indices()
            .iter()
            .map(usize::to_string)
            .collect::<Vec<_>>()
            .join(",");
        let ops = self
            .operations
            .iter()
            .map(op_wire)
            .collect::<Vec<_>>()
            .join(";");
        format!(
            "pattern-mutator|amount={}|seed={}|locks={}|ops={}",
            self.amount, self.seed, locks, ops
        )
    }

    /// Parses a config from its wire string, validating each field.
    ///
    /// # Examples
    ///
    /// ```
    /// use sim_lib_music_transform::{MutationOp, PatternMutatorConfig};
    ///
    /// let config = PatternMutatorConfig::new(vec![MutationOp::Reverse]).with_amount(80);
    /// let wire = config.to_wire();
    /// assert_eq!(PatternMutatorConfig::from_wire(&wire), Ok(config));
    /// ```
    pub fn from_wire(value: &str) -> Result<Self, PatternMutatorError> {
        let Some(rest) = value.strip_prefix("pattern-mutator|") else {
            return Err(PatternMutatorError::InvalidWire);
        };
        let mut amount = 100;
        let mut seed = 0;
        let mut locks = PatternLockSet::default();
        let mut operations = Vec::new();

        for part in rest.split('|') {
            let (key, value) = part
                .split_once('=')
                .ok_or(PatternMutatorError::InvalidWire)?;
            match key {
                "amount" => amount = parse_number::<u8>(value)?.min(100),
                "seed" => seed = parse_number(value)?,
                "locks" if value.is_empty() => locks = PatternLockSet::default(),
                "locks" => {
                    locks = PatternLockSet::from_note_indices(
                        value
                            .split(',')
                            .map(parse_number)
                            .collect::<Result<Vec<_>, _>>()?,
                    )
                }
                "ops" if value.is_empty() => operations = Vec::new(),
                "ops" => {
                    operations = value
                        .split(';')
                        .map(parse_op)
                        .collect::<Result<Vec<_>, _>>()?
                }
                _ => return Err(PatternMutatorError::InvalidWire),
            }
        }

        Ok(Self {
            operations,
            amount,
            seed,
            locks,
        })
    }
}

/// Applies a [`PatternMutatorConfig`] to material and returns the mutated music.
pub fn mutate_pattern(object: &dyn MusicObject, config: &PatternMutatorConfig) -> Music {
    let original = to_piano_roll(object)
        .items
        .into_iter()
        .enumerate()
        .map(|(source_index, item)| PatternNote { source_index, item })
        .collect::<Vec<_>>();
    let mut notes = original.clone();
    let mut rng = PatternRng::new(config.seed);
    let mut next_source_index = original.len();

    for op in &config.operations {
        apply_op(
            &mut notes,
            op,
            config.amount,
            &config.locks,
            &mut rng,
            &mut next_source_index,
        );
        restore_locks(&mut notes, &original, &config.locks);
    }

    Music::PianoRoll(canonical_roll(
        notes.into_iter().map(|note| note.item).collect(),
    ))
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct PatternNote {
    source_index: usize,
    item: TimedNote,
}

fn apply_op(
    notes: &mut Vec<PatternNote>,
    op: &MutationOp,
    amount: u8,
    locks: &PatternLockSet,
    rng: &mut PatternRng,
    next_source_index: &mut usize,
) {
    if amount == 0 && !matches!(op, MutationOp::Thin { .. }) {
        return;
    }
    match op {
        MutationOp::Reverse => apply_reverse(notes, locks),
        MutationOp::Rotate { steps } => apply_rotate(notes, scaled_i32(*steps, amount), locks),
        MutationOp::Transpose { semitones } => {
            let semitones = scaled_i32(*semitones, amount);
            if semitones != 0 {
                transform_unlocked(notes, locks, |item| {
                    transform_single(item, |object| transpose(object, semitones))
                });
            }
        }
        MutationOp::Invert { axis } => {
            transform_unlocked(notes, locks, |item| {
                transform_single(item, |object| pitch_invert(object, *axis))
            });
        }
        MutationOp::ShuffleWithinBeat { beat } => {
            if *beat > Time::from_integer(0) {
                apply_shuffle_within_beat(notes, *beat, amount, locks, rng);
            }
        }
        MutationOp::Thin { keep_percent } => {
            apply_thin(notes, effective_keep(*keep_percent, amount), locks, rng);
        }
        MutationOp::Thicken { semitones } => {
            let semitones = scaled_i32(*semitones, amount);
            if semitones != 0 {
                apply_thicken(notes, semitones, amount, locks, rng, next_source_index);
            }
        }
        MutationOp::VelocityRemap { low, high } => {
            apply_velocity_remap(notes, *low, *high, amount, locks, rng);
        }
        MutationOp::RhythmDisplace { offset } => {
            let offset = scaled_time(*offset, amount);
            if offset != Time::from_integer(0) {
                apply_rhythm_displace(notes, offset, locks, rng);
            }
        }
        MutationOp::ScaleConform { scale } => {
            transform_unlocked(notes, locks, |item| {
                transform_single(item, |object| chord_tones_in(object, scale))
            });
        }
    }
}

fn apply_reverse(notes: &mut [PatternNote], locks: &PatternLockSet) {
    let span = pattern_span(notes);
    let _algebra = retrograde_with_mode(&pattern_music(notes), RetrogradeMode::Cutout);
    for note in notes {
        if !locks.contains(note.source_index) {
            note.item.onset = span - note.item.onset - note.item.note.duration;
        }
    }
}

fn apply_rotate(notes: &mut [PatternNote], steps: i32, locks: &PatternLockSet) {
    if steps == 0 {
        return;
    }
    let mut slots = notes.iter().map(|note| note.item.onset).collect::<Vec<_>>();
    slots.sort();
    slots.dedup();
    if slots.len() < 2 {
        return;
    }
    for note in notes {
        if locks.contains(note.source_index) {
            continue;
        }
        let Ok(index) = slots.binary_search(&note.item.onset) else {
            continue;
        };
        let target = (index as i32 + steps).rem_euclid(slots.len() as i32) as usize;
        note.item.onset = slots[target];
    }
}

fn apply_shuffle_within_beat(
    notes: &mut [PatternNote],
    beat: Time,
    amount: u8,
    locks: &PatternLockSet,
    rng: &mut PatternRng,
) {
    let mut groups: BTreeMap<i64, Vec<usize>> = BTreeMap::new();
    for (index, note) in notes.iter().enumerate() {
        if !locks.contains(note.source_index) {
            groups
                .entry(time_bucket(note.item.onset, beat))
                .or_default()
                .push(index);
        }
    }
    for group in groups.values() {
        if group.len() < 2 || !rng.chance(amount) {
            continue;
        }
        let mut onsets = group
            .iter()
            .map(|index| notes[*index].item.onset)
            .collect::<Vec<_>>();
        rng.shuffle(&mut onsets);
        for (index, onset) in group.iter().zip(onsets) {
            notes[*index].item.onset = onset;
        }
    }
}

fn apply_thin(
    notes: &mut Vec<PatternNote>,
    keep_percent: u8,
    locks: &PatternLockSet,
    rng: &mut PatternRng,
) {
    notes.retain(|note| locks.contains(note.source_index) || rng.chance(keep_percent));
}

fn apply_thicken(
    notes: &mut Vec<PatternNote>,
    semitones: i32,
    amount: u8,
    locks: &PatternLockSet,
    rng: &mut PatternRng,
    next_source_index: &mut usize,
) {
    let mut extras = Vec::new();
    for note in notes.iter() {
        if locks.contains(note.source_index) || !rng.chance(amount) {
            continue;
        }
        let item = transform_single(note.item.clone(), |object| transpose(object, semitones));
        extras.push(PatternNote {
            source_index: *next_source_index,
            item,
        });
        *next_source_index += 1;
    }
    notes.extend(extras);
}

fn apply_velocity_remap(
    notes: &mut [PatternNote],
    low: u8,
    high: u8,
    amount: u8,
    locks: &PatternLockSet,
    rng: &mut PatternRng,
) {
    let (low, high) = if low <= high {
        (low, high)
    } else {
        (high, low)
    };
    let width = usize::from(high - low) + 1;
    for note in notes {
        if locks.contains(note.source_index) {
            continue;
        }
        let target = low + rng.range(width) as u8;
        note.item.note.velocity = blend_u8(note.item.note.velocity, target, amount);
    }
}

fn apply_rhythm_displace(
    notes: &mut [PatternNote],
    offset: Time,
    locks: &PatternLockSet,
    rng: &mut PatternRng,
) {
    for note in notes {
        if locks.contains(note.source_index) {
            continue;
        }
        let moved = if rng.next_bool() {
            note.item.onset + offset
        } else {
            note.item.onset - offset
        };
        note.item.onset = moved.max(Time::from_integer(0));
    }
}

fn transform_unlocked(
    notes: &mut [PatternNote],
    locks: &PatternLockSet,
    mut transform: impl FnMut(TimedNote) -> TimedNote,
) {
    for note in notes {
        if !locks.contains(note.source_index) {
            note.item = transform(note.item.clone());
        }
    }
}

fn restore_locks(notes: &mut Vec<PatternNote>, original: &[PatternNote], locks: &PatternLockSet) {
    for original_note in original {
        if !locks.contains(original_note.source_index) {
            continue;
        }
        match notes
            .iter_mut()
            .find(|note| note.source_index == original_note.source_index)
        {
            Some(note) => note.item = original_note.item.clone(),
            None => notes.push(original_note.clone()),
        }
    }
}

fn transform_single(
    item: TimedNote,
    transform: impl FnOnce(&dyn MusicObject) -> Music,
) -> TimedNote {
    let music = Music::PianoRoll(canonical_roll(vec![item]));
    let Music::PianoRoll(mut roll) = transform(&music) else {
        unreachable!("music transform returns a piano roll")
    };
    roll.items.remove(0)
}

fn pattern_music(notes: &[PatternNote]) -> Music {
    Music::PianoRoll(canonical_roll(
        notes.iter().map(|note| note.item.clone()).collect(),
    ))
}

fn pattern_span(notes: &[PatternNote]) -> Time {
    notes
        .iter()
        .map(|note| note.item.onset + note.item.note.duration)
        .max()
        .unwrap_or_else(|| Time::from_integer(0))
}

fn scaled_i32(value: i32, amount: u8) -> i32 {
    let scaled = value * i32::from(amount) / 100;
    if scaled == 0 && value != 0 && amount > 0 {
        value.signum()
    } else {
        scaled
    }
}

fn scaled_time(value: Time, amount: u8) -> Time {
    value * Time::new(i64::from(amount), 100)
}

fn effective_keep(keep_percent: u8, amount: u8) -> u8 {
    let keep_percent = keep_percent.min(100);
    let remove = 100 - keep_percent;
    100 - ((u16::from(remove) * u16::from(amount) / 100) as u8)
}

fn blend_u8(source: u8, target: u8, amount: u8) -> u8 {
    let source = u16::from(source);
    let target = u16::from(target);
    let amount = u16::from(amount);
    ((source * (100 - amount) + target * amount) / 100) as u8
}

fn time_bucket(onset: Time, beat: Time) -> i64 {
    let ratio = onset / beat;
    (*ratio.numer()).div_euclid(*ratio.denom())
}

fn op_wire(op: &MutationOp) -> String {
    match op {
        MutationOp::Reverse => "reverse".to_owned(),
        MutationOp::Rotate { steps } => format!("rotate:{steps}"),
        MutationOp::Transpose { semitones } => format!("transpose:{semitones}"),
        MutationOp::Invert { axis } => format!("invert:{}", axis.semitone()),
        MutationOp::ShuffleWithinBeat { beat } => format!("shuffle:{}", time_wire(*beat)),
        MutationOp::Thin { keep_percent } => format!("thin:{keep_percent}"),
        MutationOp::Thicken { semitones } => format!("thicken:{semitones}"),
        MutationOp::VelocityRemap { low, high } => format!("velocity:{low}:{high}"),
        MutationOp::RhythmDisplace { offset } => format!("rhythm:{}", time_wire(*offset)),
        MutationOp::ScaleConform { scale } => {
            format!("scale:{}:{}", scale.tonic.0, scale.mode.name())
        }
    }
}

fn parse_op(value: &str) -> Result<MutationOp, PatternMutatorError> {
    let mut parts = value.split(':');
    match parts.next().ok_or(PatternMutatorError::InvalidWire)? {
        "reverse" => Ok(MutationOp::Reverse),
        "rotate" => Ok(MutationOp::Rotate {
            steps: parse_required(parts.next())?,
        }),
        "transpose" => Ok(MutationOp::Transpose {
            semitones: parse_required(parts.next())?,
        }),
        "invert" => Ok(MutationOp::Invert {
            axis: Pitch::from_semitone(parse_required(parts.next())?),
        }),
        "shuffle" => Ok(MutationOp::ShuffleWithinBeat {
            beat: parse_time(parts.next().ok_or(PatternMutatorError::InvalidWire)?)?,
        }),
        "thin" => Ok(MutationOp::Thin {
            keep_percent: parse_required(parts.next())?,
        }),
        "thicken" => Ok(MutationOp::Thicken {
            semitones: parse_required(parts.next())?,
        }),
        "velocity" => Ok(MutationOp::VelocityRemap {
            low: parse_required(parts.next())?,
            high: parse_required(parts.next())?,
        }),
        "rhythm" => Ok(MutationOp::RhythmDisplace {
            offset: parse_time(parts.next().ok_or(PatternMutatorError::InvalidWire)?)?,
        }),
        "scale" => {
            let tonic = parse_required(parts.next())?;
            let mode = parse_mode(parts.next().ok_or(PatternMutatorError::InvalidWire)?)?;
            let tonic = PitchClass::new(tonic)
                .map_err(|_| PatternMutatorError::InvalidPitchClass(tonic))?;
            Ok(MutationOp::ScaleConform {
                scale: Scale::new(tonic, mode),
            })
        }
        _ => Err(PatternMutatorError::InvalidWire),
    }
}

fn parse_required<T: std::str::FromStr>(value: Option<&str>) -> Result<T, PatternMutatorError>
where
    PatternMutatorError: From<<T as std::str::FromStr>::Err>,
{
    value
        .ok_or(PatternMutatorError::InvalidWire)?
        .parse()
        .map_err(PatternMutatorError::from)
}

fn parse_number<T: std::str::FromStr>(value: &str) -> Result<T, PatternMutatorError>
where
    PatternMutatorError: From<<T as std::str::FromStr>::Err>,
{
    value.parse().map_err(PatternMutatorError::from)
}

impl From<ParseIntError> for PatternMutatorError {
    fn from(_: ParseIntError) -> Self {
        Self::InvalidNumber
    }
}

fn time_wire(time: Time) -> String {
    format!("{}/{}", time.numer(), time.denom())
}

fn parse_time(value: &str) -> Result<Time, PatternMutatorError> {
    let (numer, denom) = value
        .split_once('/')
        .ok_or(PatternMutatorError::InvalidWire)?;
    Ok(Time::new(parse_number(numer)?, parse_number(denom)?))
}

fn parse_mode(value: &str) -> Result<Mode, PatternMutatorError> {
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
        _ => Err(PatternMutatorError::InvalidMode(value.to_owned())),
    }
}

#[derive(Clone, Debug)]
struct PatternRng {
    state: u64,
}

impl PatternRng {
    fn new(seed: u64) -> Self {
        Self {
            state: seed ^ 0x9e37_79b9_7f4a_7c15,
        }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut value = self.state;
        value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        value ^ (value >> 31)
    }

    fn next_bool(&mut self) -> bool {
        self.next_u64() & 1 == 1
    }

    fn range(&mut self, upper: usize) -> usize {
        if upper == 0 {
            0
        } else {
            (self.next_u64() as usize) % upper
        }
    }

    fn chance(&mut self, percent: u8) -> bool {
        percent >= 100 || (percent > 0 && self.range(100) < usize::from(percent))
    }

    fn shuffle<T>(&mut self, values: &mut [T]) {
        for index in (1..values.len()).rev() {
            let swap_with = self.range(index + 1);
            values.swap(index, swap_with);
        }
    }
}
