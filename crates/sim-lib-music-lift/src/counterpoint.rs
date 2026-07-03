use std::collections::{BTreeMap, BTreeSet};

use sim_kernel::{Diagnostic, Severity};
use sim_lib_music_core::{Counterpoint, Melody, MelodyItem, Rest, Time};

use crate::collect::{CollectedMidi, CollectedNote, collect_midi};
use crate::{CounterpointLiftOpts, LiftError, LiftReport, VoiceAssignment};

type CounterpointVoices = (Vec<Melody>, Vec<String>, Vec<Diagnostic>);

pub(crate) fn lift_counterpoint_impl(
    file: &sim_lib_midi_smf::SmfFile,
    opts: &CounterpointLiftOpts,
) -> Result<LiftReport<Counterpoint>, LiftError> {
    if opts.max_voices_per_track == 0 {
        return Err(LiftError::InvalidVoiceLimit);
    }
    if opts.min_rest_to_close < Time::from_integer(0) {
        return Err(LiftError::InvalidRestThreshold);
    }

    let collected = collect_midi(file);
    let (voices, names, diagnostics) = lift_to_counterpoint_voices(&collected, opts)?;
    let mut all_diagnostics = collected.diagnostics;
    all_diagnostics.extend(diagnostics);
    Ok(LiftReport {
        value: Counterpoint::new(voices, names)?,
        diagnostics: all_diagnostics,
    })
}

pub(crate) fn lift_to_counterpoint_voices(
    collected: &CollectedMidi,
    opts: &CounterpointLiftOpts,
) -> Result<CounterpointVoices, LiftError> {
    let mut diagnostics = Vec::new();
    let mut melodies = Vec::new();
    let mut names = Vec::new();

    for group in candidate_groups(&collected.notes, opts.voice_assignment) {
        let base_name = voice_base_name(&group, opts.voice_assignment);
        let split = match opts.voice_assignment {
            VoiceAssignment::HighestFirst => {
                greedy_split(&group.notes, true, opts, &mut diagnostics)
            }
            VoiceAssignment::LowestFirst => {
                greedy_split(&group.notes, false, opts, &mut diagnostics)
            }
            VoiceAssignment::ChannelOnly | VoiceAssignment::TrackThenChannel => {
                if has_overlap(&group.notes) {
                    diagnostics.push(warning(format!(
                        "{base_name} rejected because notes are not monophonic"
                    )));
                    Vec::new()
                } else {
                    vec![group.notes.clone()]
                }
            }
        };

        let needs_suffix = split.len() > 1;
        for (index, voice) in split.into_iter().enumerate() {
            if let Some(melody) = build_melody(&voice, opts, &mut diagnostics)? {
                let name = if needs_suffix {
                    format!("{} {}", base_name, index + 1)
                } else {
                    base_name.clone()
                };
                melodies.push(melody);
                names.push(name);
            }
        }
    }

    Ok((melodies, names, diagnostics))
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Group {
    notes: Vec<CollectedNote>,
    track: usize,
    channel: u8,
    track_name: Option<String>,
}

fn candidate_groups(notes: &[CollectedNote], mode: VoiceAssignment) -> Vec<Group> {
    let mut groups: BTreeMap<(usize, u8), Vec<CollectedNote>> = BTreeMap::new();
    match mode {
        VoiceAssignment::ChannelOnly => {
            for note in notes {
                groups
                    .entry((usize::MAX, note.note.channel.0))
                    .or_default()
                    .push(note.clone());
            }
        }
        VoiceAssignment::TrackThenChannel => {
            for note in notes {
                groups
                    .entry((note.track, note.note.channel.0))
                    .or_default()
                    .push(note.clone());
            }
        }
        VoiceAssignment::HighestFirst | VoiceAssignment::LowestFirst => {
            let multiple_tracks = notes
                .iter()
                .map(|note| note.track)
                .collect::<BTreeSet<_>>()
                .len()
                > 1;
            for note in notes {
                let key = if multiple_tracks {
                    (note.track, u8::MAX)
                } else {
                    (0, u8::MAX)
                };
                groups.entry(key).or_default().push(note.clone());
            }
        }
    }

    groups
        .into_iter()
        .map(|((track, channel), grouped)| Group {
            track,
            channel,
            track_name: grouped.first().and_then(|note| note.track_name.clone()),
            notes: grouped,
        })
        .collect()
}

fn voice_base_name(group: &Group, mode: VoiceAssignment) -> String {
    match mode {
        VoiceAssignment::ChannelOnly => format!("Channel {}", group.channel),
        VoiceAssignment::TrackThenChannel => group
            .track_name
            .as_deref()
            .map(|name| format!("{name} ch{}", group.channel))
            .unwrap_or_else(|| format!("Track {} ch{}", group.track, group.channel)),
        VoiceAssignment::HighestFirst | VoiceAssignment::LowestFirst => group
            .track_name
            .clone()
            .unwrap_or_else(|| format!("Voice {}", group.track)),
    }
}

fn has_overlap(notes: &[CollectedNote]) -> bool {
    let mut sorted = notes.iter().collect::<Vec<_>>();
    sorted.sort_by(|left, right| {
        left.onset
            .cmp(&right.onset)
            .then_with(|| left.order.cmp(&right.order))
    });
    let mut cursor = Time::from_integer(0);
    for note in sorted {
        if note.onset < cursor {
            return true;
        }
        cursor = note.onset + note.duration;
    }
    false
}

fn greedy_split(
    notes: &[CollectedNote],
    highest_first: bool,
    opts: &CounterpointLiftOpts,
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<Vec<CollectedNote>> {
    let mut ordered = notes.to_vec();
    ordered.sort_by(|left, right| {
        left.onset
            .cmp(&right.onset)
            .then_with(|| {
                let ordering = left.note.pitch.semitone().cmp(&right.note.pitch.semitone());
                if highest_first {
                    ordering.reverse()
                } else {
                    ordering
                }
            })
            .then_with(|| left.order.cmp(&right.order))
    });
    let mut voices: Vec<Vec<CollectedNote>> = Vec::new();
    let mut ends: Vec<Time> = Vec::new();
    for note in ordered {
        if let Some(index) = ends.iter().position(|end| *end <= note.onset) {
            ends[index] = note.onset + note.duration;
            voices[index].push(note);
        } else if voices.len() < opts.max_voices_per_track {
            ends.push(note.onset + note.duration);
            voices.push(vec![note]);
        } else {
            diagnostics.push(warning(format!(
                "dropped note {} because voice limit {} was exceeded",
                note.note.pitch.semitone(),
                opts.max_voices_per_track
            )));
        }
    }
    voices
}

fn build_melody(
    notes: &[CollectedNote],
    opts: &CounterpointLiftOpts,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<Option<Melody>, LiftError> {
    if notes.is_empty() {
        return Ok(None);
    }
    let mut ordered = notes.to_vec();
    ordered.sort_by(|left, right| {
        left.onset
            .cmp(&right.onset)
            .then_with(|| left.order.cmp(&right.order))
    });
    let mut items = Vec::new();
    let mut cursor = Time::from_integer(0);
    for note in ordered {
        if note.onset < cursor {
            diagnostics.push(warning(format!(
                "voice containing {} remained polyphonic and was rejected",
                note.note.pitch.semitone()
            )));
            return Ok(None);
        }
        let gap = note.onset - cursor;
        if gap > Time::from_integer(0) {
            if gap >= opts.min_rest_to_close || items.is_empty() {
                items.push(MelodyItem::Rest(Rest::new(gap)?));
            } else if let Some(MelodyItem::Note(previous)) = items.last_mut() {
                previous.duration += gap;
            }
        }
        let mut lifted = note.note.clone();
        lifted.duration = note.duration;
        items.push(MelodyItem::Note(lifted));
        cursor = note.onset + note.duration;
    }
    Ok(Some(Melody::new(items)?))
}

fn warning(message: String) -> Diagnostic {
    Diagnostic {
        severity: Severity::Warning,
        message,
        source: None,
        span: None,
        code: None,
        related: Vec::new(),
    }
}
