use sim_lib_music_core::{LaneId, NoteEvent, Pitch, PlayEvent, Tick};

use crate::arpeggio::{push_trace, render_engine};
use crate::{
    ArpEngineConfig, ArpInputNote, ArpRender, ArpStepTrace, ArpTraceAction, ArpTraceSource,
};

/// Rule selecting which chord notes act as held anchors.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AnchorPolicy {
    /// Take the given number of lowest-pitched notes as anchors.
    Lowest(usize),
    /// Take the given number of highest-pitched notes as anchors.
    Highest(usize),
    /// Take the notes at the given input indices as anchors.
    Indices(Vec<usize>),
}

impl Default for AnchorPolicy {
    fn default() -> Self {
        Self::Lowest(1)
    }
}

/// Pitch transform applied to movement notes before arpeggiation.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct MovementTransform {
    /// Semitones to transpose by.
    pub transpose_semitones: i32,
    /// Optional axis to invert pitches around, applied before transpose.
    pub invert_axis: Option<Pitch>,
}

impl MovementTransform {
    /// Builds a transform that only transposes by the given semitones.
    pub fn transpose(semitones: i32) -> Self {
        Self {
            transpose_semitones: semitones,
            invert_axis: None,
        }
    }

    /// Builds a transform that only inverts pitches around the given axis.
    pub fn invert(axis: Pitch) -> Self {
        Self {
            transpose_semitones: 0,
            invert_axis: Some(axis),
        }
    }

    /// Applies inversion (if any) then transposition to a pitch.
    pub fn apply(self, pitch: Pitch) -> Pitch {
        let pitch = self
            .invert_axis
            .map(|axis| pitch.invert(axis))
            .unwrap_or(pitch);
        pitch.transpose(self.transpose_semitones)
    }
}

/// One step of a movement pattern: a note selection plus a transposition.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct MovementStep {
    /// Index (modulo note count) of the movement note to play.
    pub note_index: usize,
    /// Extra semitones applied to the selected note.
    pub transpose_semitones: i32,
}

impl MovementStep {
    /// Creates a movement step from a note index and transposition.
    pub fn new(note_index: usize, transpose_semitones: i32) -> Self {
        Self {
            note_index,
            transpose_semitones,
        }
    }
}

/// Ordered list of movement steps applied to the non-anchor notes.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MovementPattern {
    /// Steps making up the pattern; empty means pass notes through.
    pub steps: Vec<MovementStep>,
}

impl MovementPattern {
    /// Creates a movement pattern from its steps.
    pub fn new(steps: Vec<MovementStep>) -> Self {
        Self { steps }
    }

    fn apply(&self, notes: &[ArpInputNote], transform: MovementTransform) -> Vec<ArpInputNote> {
        if notes.is_empty() {
            return Vec::new();
        }
        if self.steps.is_empty() {
            return notes
                .iter()
                .cloned()
                .map(|mut note| {
                    note.pitch = transform.apply(note.pitch);
                    note
                })
                .collect();
        }
        self.steps
            .iter()
            .map(|step| {
                let mut note = notes[step.note_index % notes.len()].clone();
                note.pitch = transform
                    .apply(note.pitch)
                    .transpose(step.transpose_semitones);
                note
            })
            .collect()
    }
}

/// Chord notes partitioned into held anchors and moving notes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArpRoleSplit {
    /// Notes held for the duration of the render.
    pub anchors: Vec<ArpInputNote>,
    /// Notes fed to the movement engine.
    pub movement: Vec<ArpInputNote>,
}

/// Configuration for an [`ArpLab`] anchor/movement arpeggiator.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArpLabConfig {
    /// Rule choosing which notes become anchors.
    pub anchor_policy: AnchorPolicy,
    /// Pattern applied to the movement notes.
    pub movement_pattern: MovementPattern,
    /// Transform applied to movement notes before patterning.
    pub movement_transform: MovementTransform,
    /// Engine that arpeggiates the movement notes.
    pub movement_engine: ArpEngineConfig,
    /// Lane carrying the held anchor notes.
    pub anchor_lane_id: LaneId,
    /// Lane carrying the lab's trace events.
    pub trace_lane_id: LaneId,
}

impl ArpLabConfig {
    /// Creates a config wrapping the given movement engine with defaults.
    pub fn new(movement_engine: ArpEngineConfig) -> Self {
        Self {
            anchor_policy: AnchorPolicy::default(),
            movement_pattern: MovementPattern::default(),
            movement_transform: MovementTransform::default(),
            movement_engine,
            anchor_lane_id: LaneId::new("arp-lab-anchor"),
            trace_lane_id: LaneId::new("arp-lab-trace"),
        }
    }
}

/// Arpeggiator that holds anchor notes while arpeggiating the rest.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArpLab {
    /// Configuration driving the render.
    pub config: ArpLabConfig,
}

impl ArpLab {
    /// Creates a lab arpeggiator from its config.
    pub fn new(config: ArpLabConfig) -> Self {
        Self { config }
    }

    /// Partitions a chord into anchors and movement notes per the policy.
    pub fn split_roles(&self, chord: &[ArpInputNote]) -> ArpRoleSplit {
        let mut indexed = chord.iter().cloned().enumerate().collect::<Vec<_>>();
        match &self.config.anchor_policy {
            AnchorPolicy::Lowest(_) | AnchorPolicy::Highest(_) => {
                indexed.sort_by_key(|(_, note)| note.pitch.semitone());
            }
            AnchorPolicy::Indices(_) => {}
        }

        let anchor_indices = match &self.config.anchor_policy {
            AnchorPolicy::Lowest(count) => indexed
                .iter()
                .take(*count)
                .map(|(index, _)| *index)
                .collect::<Vec<_>>(),
            AnchorPolicy::Highest(count) => indexed
                .iter()
                .rev()
                .take(*count)
                .map(|(index, _)| *index)
                .collect::<Vec<_>>(),
            AnchorPolicy::Indices(indices) => indices.clone(),
        };

        let mut anchors = Vec::new();
        let mut movement = Vec::new();
        for (index, note) in chord.iter().cloned().enumerate() {
            if anchor_indices.contains(&index) {
                anchors.push(note);
            } else {
                movement.push(note);
            }
        }
        ArpRoleSplit { anchors, movement }
    }

    /// Renders `step_count` steps starting at tick zero.
    pub fn render(&self, chord: &[ArpInputNote], step_count: usize) -> ArpLabRender {
        self.render_from(
            Tick {
                ticks: 0,
                tpq: self.config.movement_engine.rate.tpq.max(1),
            },
            chord,
            step_count,
        )
    }

    /// Renders the lab output; alias for [`ArpLab::render`].
    pub fn freeze(&self, chord: &[ArpInputNote], step_count: usize) -> ArpLabRender {
        self.render(chord, step_count)
    }

    /// Renders `step_count` steps starting at the given tick.
    pub fn render_from(
        &self,
        start: Tick,
        chord: &[ArpInputNote],
        step_count: usize,
    ) -> ArpLabRender {
        let roles = self.split_roles(chord);
        let mut output = ArpRender::default();
        let hold_duration = self
            .config
            .movement_engine
            .rate
            .mul_int(step_count.max(1) as i64);

        for (index, note) in roles.anchors.iter().enumerate() {
            output.events.push(PlayEvent::Note(NoteEvent {
                lane_id: self.config.anchor_lane_id.clone(),
                time: start,
                duration: hold_duration,
                pitch: note.pitch,
                velocity: note.velocity,
                channel: note.channel,
            }));
            push_trace(
                &mut output,
                ArpStepTrace {
                    source: ArpTraceSource::Lab,
                    lane_id: self.config.trace_lane_id.clone(),
                    time: start,
                    step: index as u64,
                    action: ArpTraceAction::HeldAnchor,
                    pitch: Some(note.pitch),
                    gate_open: true,
                    mask_open: true,
                },
            );
        }

        let movement = self
            .config
            .movement_pattern
            .apply(&roles.movement, self.config.movement_transform);
        output.extend(render_engine(
            ArpTraceSource::Lab,
            &self.config.movement_engine,
            &movement,
            start,
            step_count,
        ));

        ArpLabRender {
            roles,
            output: output.stable(),
        }
    }
}

/// Result of an [`ArpLab`] render: the role split and the rendered output.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArpLabRender {
    /// Anchor/movement partition that produced the output.
    pub roles: ArpRoleSplit,
    /// Rendered events, gate/mask frames, and traces.
    pub output: ArpRender,
}
