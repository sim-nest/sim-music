use sim_kernel::{Result, Symbol};

use crate::arranger::{
    Arranger, ArrangerDiagnostic, ArrangerPlacement, ArrangerRender, PitchRemap,
    PlacementTransform, PlayableRef, StretchPolicy, TracePolicy, music_err,
};
use crate::{
    AtomRef, DiagnosticEvent, LaneId, MusicObject, NoteEvent, Pitch, PlayContext, PlayEvent, Time,
    TimedNote, TraceEvent, time_to_tick,
};

#[derive(Clone, Debug)]
struct ArrangedNote {
    placement_id: Symbol,
    lane_id: LaneId,
    order: usize,
    item: TimedNote,
}

struct NoteRender {
    notes: Vec<ArrangedNote>,
    diagnostics: Vec<ArrangerDiagnostic>,
    traces: Vec<ArrangerTrace>,
}

struct ArrangerTrace {
    at: Time,
    step: u64,
}

impl Arranger {
    /// Renders the arrangement into clipped, stable-ordered play events.
    ///
    /// Applies each placement's stretch, transforms, pitch remap, and filter,
    /// clips events to the context range, and appends diagnostics and traces.
    pub fn render_arrangement(&self, cx: &PlayContext) -> Result<ArrangerRender> {
        let notes = self.render_notes();
        let mut events = cx.upstream.clone();
        for arranged in notes.notes {
            let onset = time_to_tick(arranged.item.onset, cx.ppq).map_err(music_err)?;
            let duration = time_to_tick(arranged.item.note.duration, cx.ppq).map_err(music_err)?;
            let Some((time, duration)) = cx.range.clip_span(onset, duration) else {
                continue;
            };
            events.push(PlayEvent::Note(NoteEvent {
                lane_id: arranged.lane_id,
                time,
                duration,
                pitch: arranged.item.note.pitch,
                velocity: arranged.item.note.velocity,
                channel: arranged.item.note.channel,
            }));
        }
        for diagnostic in &notes.diagnostics {
            let time = time_to_tick(diagnostic.at, cx.ppq).map_err(music_err)?;
            events.push(PlayEvent::Diagnostic(DiagnosticEvent {
                lane_id: LaneId::new("arranger-diagnostics"),
                time,
                message: diagnostic.message.clone(),
            }));
        }
        for trace in notes.traces {
            let time = time_to_tick(trace.at, cx.ppq).map_err(music_err)?;
            events.push(PlayEvent::Trace(TraceEvent {
                lane_id: LaneId::new("arranger-trace"),
                time,
                step: trace.step,
            }));
        }
        crate::stable_event_order(&mut events);
        Ok(ArrangerRender {
            events,
            diagnostics: notes.diagnostics,
        })
    }

    /// Renders the arrangement and returns only its timed notes.
    pub fn rendered_notes(&self) -> Vec<TimedNote> {
        self.render_notes()
            .notes
            .into_iter()
            .map(|note| note.item)
            .collect()
    }

    /// Renders the arrangement and returns only the diagnostics it raises.
    pub fn diagnostics(&self) -> Vec<ArrangerDiagnostic> {
        self.render_notes().diagnostics
    }

    fn render_notes(&self) -> NoteRender {
        let mut notes = Vec::new();
        let mut diagnostics = Vec::new();
        let mut traces = Vec::new();
        for (order, placement) in self.placements.iter().enumerate() {
            if placement.trace == TracePolicy::Full {
                traces.push(ArrangerTrace {
                    at: placement.at,
                    step: order as u64,
                });
            }
            let mut local = match &placement.playable {
                PlayableRef::Inline(music) => notes_from_object(music.as_ref()),
                PlayableRef::Symbol(symbol) => {
                    push_diagnostic(
                        &mut diagnostics,
                        placement,
                        format!("playable reference {symbol} is not resolved"),
                    );
                    continue;
                }
            };
            if let Some(duration) = placement.duration {
                local = clip_notes(local, duration);
            }
            apply_stretch(&mut local, placement, &mut diagnostics);
            for transform in &placement.transform {
                apply_transform(&mut local, transform, placement, &mut diagnostics);
            }
            apply_pitch_remap(
                &mut local,
                &placement.remap_pitch,
                placement,
                &mut diagnostics,
            );
            if !filter_notes(&mut local, placement, &mut diagnostics) {
                continue;
            }
            for mut item in local {
                item.onset += placement.at;
                notes.push(ArrangedNote {
                    placement_id: placement.id.clone(),
                    lane_id: placement.lane.clone(),
                    order,
                    item,
                });
            }
        }
        stable_note_order(&mut notes);
        NoteRender {
            notes,
            diagnostics,
            traces,
        }
    }
}

fn apply_stretch(
    notes: &mut [TimedNote],
    placement: &ArrangerPlacement,
    diagnostics: &mut Vec<ArrangerDiagnostic>,
) {
    let factor = match placement.stretch {
        StretchPolicy::None => return,
        StretchPolicy::TempoRatio(ratio) if ratio > Time::from_integer(0) => ratio.recip(),
        StretchPolicy::TimeRatio(ratio) if ratio > Time::from_integer(0) => ratio,
        StretchPolicy::TempoRatio(_) | StretchPolicy::TimeRatio(_) => {
            push_diagnostic(diagnostics, placement, "stretch ratio must be positive");
            return;
        }
        StretchPolicy::FitToDuration => {
            let Some(target) = placement.duration else {
                push_diagnostic(
                    diagnostics,
                    placement,
                    "fit stretch needs a placement duration",
                );
                return;
            };
            let span = note_span(notes);
            if target <= Time::from_integer(0) || span <= Time::from_integer(0) {
                push_diagnostic(
                    diagnostics,
                    placement,
                    "fit stretch needs positive source and target spans",
                );
                return;
            }
            target / span
        }
    };
    for note in notes {
        note.onset *= factor;
        note.note.duration *= factor;
    }
}

fn apply_transform(
    notes: &mut [TimedNote],
    transform: &PlacementTransform,
    placement: &ArrangerPlacement,
    diagnostics: &mut Vec<ArrangerDiagnostic>,
) {
    match transform {
        PlacementTransform::TransposeSemitones(semitones) => {
            for note in notes {
                note.note.pitch = note.note.pitch.transpose(*semitones);
            }
        }
        PlacementTransform::TransposeOctaves(octaves) => {
            let semitones = i32::from(*octaves) * 12;
            for note in notes {
                note.note.pitch = note.note.pitch.transpose(semitones);
            }
        }
        PlacementTransform::InvertAroundPitch(axis) => {
            for note in notes {
                note.note.pitch = note.note.pitch.invert(*axis);
            }
        }
        PlacementTransform::InvertAroundPitchClass(axis) => {
            for note in notes {
                note.note.pitch = Pitch {
                    class: note.note.pitch.class.invert(*axis),
                    octave: note.note.pitch.octave,
                };
            }
        }
        PlacementTransform::Retrograde => {
            let total = placement.duration.unwrap_or_else(|| note_span(notes));
            if total <= Time::from_integer(0) {
                push_diagnostic(diagnostics, placement, "retrograde needs a positive span");
                return;
            }
            for note in notes {
                note.onset = total - note.onset - note.note.duration;
            }
        }
    }
}

fn apply_pitch_remap(
    notes: &mut [TimedNote],
    remap: &PitchRemap,
    placement: &ArrangerPlacement,
    diagnostics: &mut Vec<ArrangerDiagnostic>,
) {
    match remap {
        PitchRemap::None => {}
        PitchRemap::Chromatic(semitones) => {
            for note in notes {
                note.note.pitch = note.note.pitch.transpose(*semitones);
            }
        }
        PitchRemap::PitchClass { from, to } => {
            for note in notes {
                if note.note.pitch.class == *from {
                    note.note.pitch = Pitch {
                        class: *to,
                        octave: note.note.pitch.octave,
                    };
                }
            }
        }
        PitchRemap::DrumKey(map) => {
            for note in notes {
                let Some(key) = note.note.pitch.to_midi() else {
                    push_diagnostic(diagnostics, placement, "drum-key remap needs MIDI pitches");
                    continue;
                };
                if let Some((_, target)) = map.iter().find(|(source, _)| *source == key) {
                    note.note.pitch = Pitch::from_midi(*target);
                }
            }
        }
        PitchRemap::ScaleDegree(symbol)
        | PitchRemap::ChordTone(symbol)
        | PitchRemap::Tuning(symbol)
        | PitchRemap::Vector(symbol)
        | PitchRemap::Matrix(symbol)
        | PitchRemap::Callable(symbol) => push_diagnostic(
            diagnostics,
            placement,
            format!("pitch remap {symbol} needs a host resolver"),
        ),
    }
}

fn filter_notes(
    notes: &mut Vec<TimedNote>,
    placement: &ArrangerPlacement,
    diagnostics: &mut Vec<ArrangerDiagnostic>,
) -> bool {
    let Some(filter) = &placement.filter else {
        return true;
    };
    if filter.keep_lanes.is_empty() {
        push_diagnostic(
            diagnostics,
            placement,
            format!("filter {} evaluated as identity", filter.id),
        );
        return true;
    }
    if filter.keep_lanes.iter().any(|lane| lane == &placement.lane) {
        true
    } else {
        notes.clear();
        push_diagnostic(
            diagnostics,
            placement,
            format!("filter {} removed lane {}", filter.id, placement.lane.0),
        );
        false
    }
}

fn clip_notes(notes: Vec<TimedNote>, duration: Time) -> Vec<TimedNote> {
    notes
        .into_iter()
        .filter_map(|mut item| {
            let start = item.onset.max(Time::from_integer(0));
            let end = (item.onset + item.note.duration).min(duration);
            (start < end).then(|| {
                item.onset = start;
                item.note.duration = end - start;
                item
            })
        })
        .collect()
}

fn notes_from_object(object: &dyn MusicObject) -> Vec<TimedNote> {
    let mut atoms = Vec::new();
    object.voices(Time::from_integer(0), &mut atoms);
    atoms
        .into_iter()
        .filter_map(|atom| match atom.atom {
            AtomRef::Note(note) => Some(TimedNote {
                onset: atom.onset,
                note,
            }),
            AtomRef::Rest(_) | AtomRef::Phantom(_) => None,
        })
        .collect()
}

fn note_span(notes: &[TimedNote]) -> Time {
    notes
        .iter()
        .map(|note| note.onset + note.note.duration)
        .max()
        .unwrap_or_else(|| Time::from_integer(0))
}

fn stable_note_order(notes: &mut [ArrangedNote]) {
    notes.sort_by(|left, right| {
        left.item
            .onset
            .cmp(&right.item.onset)
            .then_with(|| left.lane_id.cmp(&right.lane_id))
            .then_with(|| {
                left.item
                    .note
                    .pitch
                    .semitone()
                    .cmp(&right.item.note.pitch.semitone())
            })
            .then_with(|| left.placement_id.cmp(&right.placement_id))
            .then_with(|| left.order.cmp(&right.order))
    });
}

fn push_diagnostic(
    diagnostics: &mut Vec<ArrangerDiagnostic>,
    placement: &ArrangerPlacement,
    message: impl Into<String>,
) {
    diagnostics.push(ArrangerDiagnostic {
        placement_id: placement.id.clone(),
        at: placement.at,
        message: message.into(),
    });
}
