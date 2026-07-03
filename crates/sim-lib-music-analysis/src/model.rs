use sim_lib_music_core::{PianoRoll, Time};
use sim_lib_pitch_core::PitchClass;
use sim_lib_pitch_set::{BitChord, PitchClassMask, PitchRangeMask};

/// Event-aligned view of a piano roll as a sequence of difference frames.
///
/// Holds one [`DiffFrame`] per distinct onset or release time, describing how
/// the sounding pitches change across the roll's timeline.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiffRoll {
    /// Frames ordered by time, one per distinct onset or release instant.
    pub frames: Vec<DiffFrame>,
}

/// Snapshot of pitch activity at one instant of a [`DiffRoll`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiffFrame {
    /// Time of this frame.
    pub at: Time,
    /// Pitches sounding across this frame.
    pub sounding: PitchRangeMask,
    /// Pitches whose onset is exactly at this time.
    pub started: PitchRangeMask,
    /// Pitches whose release is exactly at this time.
    pub ended: PitchRangeMask,
    /// Pitches sounding but not newly started here (held over from before).
    pub slurred: PitchRangeMask,
}

/// Selects which pitches a chord window draws from each frame.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ChordWindowMode {
    /// Use every pitch sounding in the frame.
    SoundingNotes,
    /// Use only pitches whose onset is at the frame's time.
    StartingNotes,
}

/// A time interval treated as a single chord, with derived pitch masks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChordWindow {
    /// Start time of the window.
    pub at: Time,
    /// End time of the window (the next frame's time).
    pub until: Time,
    /// Mode that produced this window's pitch selection.
    pub mode: ChordWindowMode,
    /// Sounding/starting pitches as an absolute pitch-range mask.
    pub range_mask: PitchRangeMask,
    /// The window's pitches folded to pitch classes.
    pub pitch_class_mask: PitchClassMask,
    /// Pitch-class mask plus chosen root, as a bit-chord.
    pub bit_chord: BitChord,
}

impl DiffRoll {
    /// Builds a difference roll from a piano roll's onsets and releases.
    pub fn from_piano_roll(roll: &PianoRoll) -> Self {
        let mut times: Vec<Time> = roll
            .items
            .iter()
            .flat_map(|item| [item.onset, item.onset + item.note.duration])
            .collect();
        times.sort();
        times.dedup();

        let frames = times
            .into_iter()
            .map(|at| {
                let mut sounding = PitchRangeMask::default();
                let mut started = PitchRangeMask::default();
                let mut ended = PitchRangeMask::default();
                for item in &roll.items {
                    let start = item.onset;
                    let end = item.onset + item.note.duration;
                    let midi = item
                        .note
                        .pitch
                        .to_midi()
                        .expect("analysis expects MIDI range");
                    if start == at {
                        started.set(midi);
                    }
                    if end == at {
                        ended.set(midi);
                    }
                    if start <= at && at < end {
                        sounding.set(midi);
                    }
                }
                let slurred = sounding.difference(started);
                DiffFrame {
                    at,
                    sounding,
                    started,
                    ended,
                    slurred,
                }
            })
            .collect();
        Self { frames }
    }
}

/// Computes chord windows directly from a piano roll under the given mode.
pub fn chord_windows_from_piano_roll(roll: &PianoRoll, mode: ChordWindowMode) -> Vec<ChordWindow> {
    chord_windows_from_diff_roll(&DiffRoll::from_piano_roll(roll), mode)
}

/// Computes chord windows from an existing difference roll under the given mode.
///
/// Each adjacent frame pair with a non-empty pitch-class mask yields one window.
pub fn chord_windows_from_diff_roll(diff: &DiffRoll, mode: ChordWindowMode) -> Vec<ChordWindow> {
    diff.frames
        .windows(2)
        .filter_map(|pair| {
            let frame = &pair[0];
            let range_mask = match mode {
                ChordWindowMode::SoundingNotes => frame.sounding,
                ChordWindowMode::StartingNotes => frame.started,
            };
            let pitch_class_mask = pitch_class_mask(range_mask);
            (pitch_class_mask.0 != 0).then(|| ChordWindow {
                at: frame.at,
                until: pair[1].at,
                mode,
                range_mask,
                pitch_class_mask,
                bit_chord: BitChord {
                    mask: pitch_class_mask,
                    root: choose_root(range_mask),
                },
            })
        })
        .collect()
}

fn pitch_class_mask(range_mask: PitchRangeMask) -> PitchClassMask {
    let pitch_classes = range_mask
        .to_pitches()
        .into_iter()
        .map(|pitch| pitch.class)
        .collect::<Vec<_>>();
    PitchClassMask::from_pitch_classes(&pitch_classes)
}

fn choose_root(range_mask: PitchRangeMask) -> Option<PitchClass> {
    range_mask
        .to_pitches()
        .into_iter()
        .next()
        .map(|pitch| pitch.class)
}
