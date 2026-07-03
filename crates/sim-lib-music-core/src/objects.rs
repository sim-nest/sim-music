use std::any::Any;
use std::collections::BTreeMap;

use crate::arranger::Arranger;
use crate::model::*;
use crate::piano_roll::PianoRoll;

impl MusicObject for Note {
    fn kind(&self) -> &'static str {
        "Note"
    }

    fn duration(&self) -> Time {
        self.duration
    }

    fn voices<'a>(&'a self, offset: Time, out: &mut Vec<TimedAtom<'a>>) {
        out.push(TimedAtom {
            onset: offset,
            atom: AtomRef::Note(self.clone()),
        });
    }

    fn clone_box(&self) -> Box<dyn MusicObject> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl MusicObject for Rest {
    fn kind(&self) -> &'static str {
        "Rest"
    }

    fn duration(&self) -> Time {
        self.duration
    }

    fn voices<'a>(&'a self, offset: Time, out: &mut Vec<TimedAtom<'a>>) {
        out.push(TimedAtom {
            onset: offset,
            atom: AtomRef::Rest(self.clone()),
        });
    }

    fn clone_box(&self) -> Box<dyn MusicObject> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl MusicObject for Par {
    fn kind(&self) -> &'static str {
        "Par"
    }

    fn duration(&self) -> Time {
        self.children
            .iter()
            .map(|child| child.duration())
            .max()
            .unwrap_or_else(|| Time::from_integer(0))
    }

    fn voices<'a>(&'a self, offset: Time, out: &mut Vec<TimedAtom<'a>>) {
        for child in &self.children {
            child.voices(offset, out);
        }
    }

    fn clone_box(&self) -> Box<dyn MusicObject> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl MusicObject for Seq {
    fn kind(&self) -> &'static str {
        "Seq"
    }

    fn duration(&self) -> Time {
        self.children
            .iter()
            .fold(Time::from_integer(0), |sum, child| sum + child.duration())
    }

    fn voices<'a>(&'a self, offset: Time, out: &mut Vec<TimedAtom<'a>>) {
        let mut cursor = offset;
        for child in &self.children {
            child.voices(cursor, out);
            cursor += child.duration();
        }
    }

    fn clone_box(&self) -> Box<dyn MusicObject> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl MusicObject for Chord {
    fn kind(&self) -> &'static str {
        "Chord"
    }

    fn duration(&self) -> Time {
        self.duration
    }

    fn voices<'a>(&'a self, offset: Time, out: &mut Vec<TimedAtom<'a>>) {
        for pitch in &self.pitches {
            out.push(TimedAtom {
                onset: offset,
                atom: AtomRef::Note(Note {
                    duration: self.duration,
                    pitch: *pitch,
                    velocity: self.velocity,
                    channel: self.channel,
                    articulation: Articulation::Normal,
                }),
            });
        }
    }

    fn clone_box(&self) -> Box<dyn MusicObject> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl MusicObject for Melody {
    fn kind(&self) -> &'static str {
        "Melody"
    }

    fn duration(&self) -> Time {
        self.total_duration()
    }

    fn voices<'a>(&'a self, offset: Time, out: &mut Vec<TimedAtom<'a>>) {
        let mut cursor = offset;
        for item in &self.items {
            match item {
                MelodyItem::Note(note) => note.voices(cursor, out),
                MelodyItem::Rest(rest) => rest.voices(cursor, out),
            }
            cursor += item.duration();
        }
    }

    fn clone_box(&self) -> Box<dyn MusicObject> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl MusicObject for Progression {
    fn kind(&self) -> &'static str {
        "Progression"
    }

    fn duration(&self) -> Time {
        self.chords
            .iter()
            .fold(Time::from_integer(0), |sum, chord| sum + chord.duration)
    }

    fn voices<'a>(&'a self, offset: Time, out: &mut Vec<TimedAtom<'a>>) {
        let mut cursor = offset;
        for chord in &self.chords {
            chord.voices(cursor, out);
            cursor += chord.duration;
        }
    }

    fn clone_box(&self) -> Box<dyn MusicObject> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl MusicObject for Counterpoint {
    fn kind(&self) -> &'static str {
        "Counterpoint"
    }

    fn duration(&self) -> Time {
        self.voices
            .iter()
            .map(Melody::total_duration)
            .max()
            .unwrap_or_else(|| Time::from_integer(0))
    }

    fn voices<'a>(&'a self, offset: Time, out: &mut Vec<TimedAtom<'a>>) {
        for voice in &self.voices {
            voice.voices(offset, out);
        }
    }

    fn clone_box(&self) -> Box<dyn MusicObject> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl MusicObject for PianoRoll {
    fn kind(&self) -> &'static str {
        "PianoRoll"
    }

    fn duration(&self) -> Time {
        self.items
            .iter()
            .map(|item| item.onset + item.note.duration)
            .max()
            .unwrap_or_else(|| Time::from_integer(0))
    }

    fn voices<'a>(&'a self, offset: Time, out: &mut Vec<TimedAtom<'a>>) {
        for item in &self.items {
            out.push(TimedAtom {
                onset: offset + item.onset,
                atom: AtomRef::Note(item.note.clone()),
            });
        }
    }

    fn clone_box(&self) -> Box<dyn MusicObject> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl MusicObject for Arranger {
    fn kind(&self) -> &'static str {
        "Arranger"
    }

    fn duration(&self) -> Time {
        self.rendered_notes()
            .into_iter()
            .map(|item| item.onset + item.note.duration)
            .max()
            .unwrap_or_else(|| Time::from_integer(0))
    }

    fn voices<'a>(&'a self, offset: Time, out: &mut Vec<TimedAtom<'a>>) {
        for item in self.rendered_notes() {
            out.push(TimedAtom {
                onset: offset + item.onset,
                atom: AtomRef::Note(item.note),
            });
        }
    }

    fn clone_box(&self) -> Box<dyn MusicObject> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl MusicObject for MidiTrackObj {
    fn kind(&self) -> &'static str {
        "MidiTrackObj"
    }

    fn duration(&self) -> Time {
        self.events
            .iter()
            .map(|event| tick_time_to_time(event.time))
            .max()
            .unwrap_or_else(|| Time::from_integer(0))
    }

    fn voices<'a>(&'a self, offset: Time, out: &mut Vec<TimedAtom<'a>>) {
        emit_midi_track_voices(&self.events, offset, out);
    }

    fn clone_box(&self) -> Box<dyn MusicObject> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl MusicObject for MidiFileObj {
    fn kind(&self) -> &'static str {
        "MidiFileObj"
    }

    fn duration(&self) -> Time {
        self.file
            .tracks
            .iter()
            .flat_map(|track| track.events.iter())
            .map(|event| tick_time_to_time(event.time))
            .max()
            .unwrap_or_else(|| Time::from_integer(0))
    }

    fn voices<'a>(&'a self, offset: Time, out: &mut Vec<TimedAtom<'a>>) {
        for track in &self.file.tracks {
            emit_midi_track_voices(&track.events, offset, out);
        }
    }

    fn clone_box(&self) -> Box<dyn MusicObject> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl MusicObject for Score {
    fn kind(&self) -> &'static str {
        "Score"
    }

    fn duration(&self) -> Time {
        self.body.duration()
    }

    fn voices<'a>(&'a self, offset: Time, out: &mut Vec<TimedAtom<'a>>) {
        self.body.voices(offset, out);
    }

    fn clone_box(&self) -> Box<dyn MusicObject> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl MusicObject for Music {
    fn kind(&self) -> &'static str {
        match self {
            Self::Note(note) => note.kind(),
            Self::Rest(rest) => rest.kind(),
            Self::Par(par) => par.kind(),
            Self::Seq(seq) => seq.kind(),
            Self::Chord(chord) => chord.kind(),
            Self::Melody(melody) => melody.kind(),
            Self::Progression(progression) => progression.kind(),
            Self::Counterpoint(counterpoint) => counterpoint.kind(),
            Self::PianoRoll(roll) => roll.kind(),
            Self::Arranger(arranger) => arranger.kind(),
            Self::MidiTrack(track) => track.kind(),
            Self::MidiFile(file) => file.kind(),
        }
    }

    fn duration(&self) -> Time {
        match self {
            Self::Note(note) => note.duration(),
            Self::Rest(rest) => rest.duration(),
            Self::Par(par) => par.duration(),
            Self::Seq(seq) => seq.duration(),
            Self::Chord(chord) => chord.duration(),
            Self::Melody(melody) => melody.duration(),
            Self::Progression(progression) => progression.duration(),
            Self::Counterpoint(counterpoint) => counterpoint.duration(),
            Self::PianoRoll(roll) => roll.duration(),
            Self::Arranger(arranger) => arranger.duration(),
            Self::MidiTrack(track) => track.duration(),
            Self::MidiFile(file) => file.duration(),
        }
    }

    fn voices<'a>(&'a self, offset: Time, out: &mut Vec<TimedAtom<'a>>) {
        match self {
            Self::Note(note) => note.voices(offset, out),
            Self::Rest(rest) => rest.voices(offset, out),
            Self::Par(par) => par.voices(offset, out),
            Self::Seq(seq) => seq.voices(offset, out),
            Self::Chord(chord) => chord.voices(offset, out),
            Self::Melody(melody) => melody.voices(offset, out),
            Self::Progression(progression) => progression.voices(offset, out),
            Self::Counterpoint(counterpoint) => counterpoint.voices(offset, out),
            Self::PianoRoll(roll) => roll.voices(offset, out),
            Self::Arranger(arranger) => arranger.voices(offset, out),
            Self::MidiTrack(track) => track.voices(offset, out),
            Self::MidiFile(file) => file.voices(offset, out),
        }
    }

    fn clone_box(&self) -> Box<dyn MusicObject> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

fn tick_time_to_time(time: TickTime) -> Time {
    Time::new(time.ticks, i64::from(time.tpq) * 4)
}

fn emit_midi_track_voices<'a>(events: &[MidiEvent], offset: Time, out: &mut Vec<TimedAtom<'a>>) {
    let mut active: BTreeMap<(u8, u8), Vec<(Time, u8)>> = BTreeMap::new();
    let mut sorted = events.to_vec();
    sorted.sort_by_key(|event| event.time);
    for event in sorted {
        match event.payload {
            MidiPayload::Channel(ChannelMessage::NoteOn { ch, key, vel }) if vel.0 > 0 => {
                active
                    .entry((ch.0, key.0))
                    .or_default()
                    .push((tick_time_to_time(event.time), vel.0));
            }
            MidiPayload::Channel(ChannelMessage::NoteOff { ch, key, .. })
            | MidiPayload::Channel(ChannelMessage::NoteOn {
                ch,
                key,
                vel: sim_lib_midi_core::U7(0),
            }) => {
                if let Some(entries) = active.get_mut(&(ch.0, key.0)) {
                    if let Some((start, velocity)) = entries.pop() {
                        let duration = tick_time_to_time(event.time) - start;
                        out.push(TimedAtom {
                            onset: offset + start,
                            atom: AtomRef::Note(Note {
                                duration,
                                pitch: Pitch::from_midi(key.0),
                                velocity,
                                channel: ch,
                                articulation: Articulation::Normal,
                            }),
                        });
                    }
                    if entries.is_empty() {
                        active.remove(&(ch.0, key.0));
                    }
                }
            }
            _ => {}
        }
    }
}
