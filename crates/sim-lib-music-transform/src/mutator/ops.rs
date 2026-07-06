use std::collections::BTreeMap;

use sim_lib_music_core::{Music, MusicObject, Time, TimedNote};

use crate::{
    RetrogradeMode, canonical_roll, chord_tones_in, pitch_invert, retrograde_with_mode, transpose,
};

use super::{MutationOp, PatternLockSet, PatternNote, PatternRng};

pub(super) fn apply_op(
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

pub(super) fn restore_locks(
    notes: &mut Vec<PatternNote>,
    original: &[PatternNote],
    locks: &PatternLockSet,
) {
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
