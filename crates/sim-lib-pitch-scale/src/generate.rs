//! Bounded scale generation from tetrachords and tertian fixtures.

use sim_lib_discrete_search::{
    NeverInterrupt, SearchControl, SearchProblem, SearchRun, SearchStep, solve,
};
use sim_lib_pitch_core::PitchClass;
use sim_lib_pitch_set::{
    PitchClassMask, PitchSetGraphError, PitchSetMovePolicy, PitchSetNeighborhood, PitchSetSpace,
    ThirdStackSignature, ThirdStep,
};

use crate::PitchScaleError;

/// Join interval between the lower tetrachord end and the upper tetrachord root.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TetrachordJoin {
    /// Join by a whole tone.
    Whole,
    /// Join by a semitone.
    Half,
}

impl TetrachordJoin {
    /// Returns the join width in semitones.
    pub const fn semitones(self) -> u8 {
        match self {
            Self::Whole => 2,
            Self::Half => 1,
        }
    }
}

/// Four scale degrees contained in one tetrachord, rooted at zero.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Tetrachord {
    /// Ascending semitone offsets from the tetrachord root.
    pub offsets: [u8; 4],
}

impl Tetrachord {
    /// Constructs a validated rooted tetrachord.
    pub fn new(offsets: [u8; 4]) -> Result<Self, PitchScaleError> {
        if offsets[0] != 0 {
            return Err(PitchScaleError::InvalidScaleInterval(offsets[0]));
        }
        for offset in offsets {
            if offset >= 12 {
                return Err(PitchScaleError::InvalidScaleInterval(offset));
            }
        }
        if offsets.windows(2).any(|window| window[0] >= window[1]) {
            return Err(PitchScaleError::InvalidScaleDegree(4));
        }
        Ok(Self { offsets })
    }
}

/// One generated scale candidate with its source choices and derived evidence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GeneratedScale {
    /// Stable zero-based ordinal in emission order.
    pub ordinal: usize,
    /// Lower tetrachord.
    pub lower: Tetrachord,
    /// Upper tetrachord.
    pub upper: Tetrachord,
    /// Join between the tetrachords.
    pub join: TetrachordJoin,
    /// Ascending semitone offsets from the scale root.
    pub intervals: Vec<u8>,
    /// Pitch-class set identity for the scale.
    pub mask: PitchClassMask,
    /// Tertian decoding from the scale, when every adjacent third is major or minor.
    pub third_stack: Option<ThirdStackSignature>,
    /// Matching catalog fixture name, if this is one of the preserved w/x/y/z scales.
    pub fixture: Option<char>,
}

/// A preserved catalog fixture for the four named seven-note scale derivations.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ScaleFixture {
    /// Fixture name: w, x, y, or z.
    pub name: char,
    /// Third-stack gap string recorded by the catalog.
    pub third_stack_gaps: &'static [ThirdStep],
    /// Ascending scale intervals rooted at zero.
    pub intervals: &'static [u8],
}

/// Catalog fixture w: the major scale.
pub const SCALE_FIXTURE_W: ScaleFixture = ScaleFixture {
    name: 'w',
    third_stack_gaps: &[
        ThirdStep::Major,
        ThirdStep::Minor,
        ThirdStep::Major,
        ThirdStep::Minor,
        ThirdStep::Minor,
        ThirdStep::Major,
        ThirdStep::Minor,
    ],
    intervals: &[0, 2, 4, 5, 7, 9, 11],
};

/// Catalog fixture x.
pub const SCALE_FIXTURE_X: ScaleFixture = ScaleFixture {
    name: 'x',
    third_stack_gaps: &[
        ThirdStep::Major,
        ThirdStep::Minor,
        ThirdStep::Major,
        ThirdStep::Minor,
        ThirdStep::Minor,
        ThirdStep::Minor,
        ThirdStep::Major,
    ],
    intervals: &[0, 2, 4, 5, 7, 8, 11],
};

/// Catalog fixture y.
pub const SCALE_FIXTURE_Y: ScaleFixture = ScaleFixture {
    name: 'y',
    third_stack_gaps: &[
        ThirdStep::Major,
        ThirdStep::Major,
        ThirdStep::Minor,
        ThirdStep::Minor,
        ThirdStep::Minor,
        ThirdStep::Major,
        ThirdStep::Minor,
    ],
    intervals: &[0, 2, 4, 5, 8, 9, 11],
};

/// Catalog fixture z.
pub const SCALE_FIXTURE_Z: ScaleFixture = ScaleFixture {
    name: 'z',
    third_stack_gaps: &[
        ThirdStep::Minor,
        ThirdStep::Major,
        ThirdStep::Major,
        ThirdStep::Minor,
        ThirdStep::Minor,
        ThirdStep::Major,
        ThirdStep::Minor,
    ],
    intervals: &[0, 2, 3, 5, 7, 9, 11],
};

/// All preserved catalog scale fixtures in their stable name order.
pub const SCALE_FIXTURES: &[ScaleFixture] = &[
    SCALE_FIXTURE_W,
    SCALE_FIXTURE_X,
    SCALE_FIXTURE_Y,
    SCALE_FIXTURE_Z,
];

/// Generate scales from lower and upper tetrachords under explicit search controls.
///
/// Choices are emitted in input order, with join choices as the final axis. The
/// generic discrete-search engine owns work charging, result limits, seed
/// recording, and the receipt.
pub fn generate_scales(
    tetrachords: &[Tetrachord],
    joins: &[TetrachordJoin],
    control: SearchControl,
) -> SearchRun<GeneratedScale> {
    solve(
        &ScaleGenerationProblem { tetrachords, joins },
        control,
        &NeverInterrupt,
    )
}

/// Decode a rooted seven-note scale into its cyclic stack of thirds.
pub fn decode_scale_third_stack(intervals: &[u8]) -> Option<ThirdStackSignature> {
    if intervals.len() != 7 || intervals.first().copied() != Some(0) {
        return None;
    }
    let mut sorted = intervals.to_vec();
    sorted.sort_unstable();
    sorted.dedup();
    if sorted != intervals {
        return None;
    }
    let mut steps = Vec::new();
    let mut index = 0usize;
    for _ in 0..intervals.len() {
        let next = (index + 2) % intervals.len();
        let delta = (i16::from(intervals[next]) - i16::from(intervals[index])).rem_euclid(12);
        steps.push(match delta {
            3 => ThirdStep::Minor,
            4 => ThirdStep::Major,
            _ => return None,
        });
        index = next;
    }
    Some(ThirdStackSignature {
        root: PitchClass::C,
        steps,
        guard: true,
    })
}

/// Return neighboring scale masks under the prior pitch-set neighborhood API.
pub fn nudge_scale_neighborhood(
    scale: PitchClassMask,
    move_policy: PitchSetMovePolicy,
    control: SearchControl,
) -> Result<Vec<PitchClassMask>, PitchSetGraphError> {
    let graph = PitchSetNeighborhood::new(
        PitchSetSpace::chromatic(scale.count_bits() as u8),
        move_policy,
    )
    .materialize(control)?;
    let Some(source) = graph.nodes.iter().position(|candidate| *candidate == scale) else {
        return Ok(Vec::new());
    };
    let mut neighbors = graph
        .edges
        .iter()
        .filter_map(|edge| {
            if edge.source == source {
                Some(graph.nodes[edge.target])
            } else if edge.target == source {
                Some(graph.nodes[edge.source])
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    neighbors.sort_by_key(|mask| mask.bits());
    neighbors.dedup();
    Ok(neighbors)
}

struct ScaleGenerationProblem<'a> {
    tetrachords: &'a [Tetrachord],
    joins: &'a [TetrachordJoin],
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct ScaleGenerationState {
    lower: Option<usize>,
    upper: Option<usize>,
    join: Option<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum ScaleGenerationChoice {
    Lower(usize),
    Upper(usize),
    Join(usize),
}

impl SearchProblem for ScaleGenerationProblem<'_> {
    type State = ScaleGenerationState;
    type Choice = ScaleGenerationChoice;
    type Output = GeneratedScale;

    fn initial_state(&self) -> Self::State {
        ScaleGenerationState::default()
    }

    fn expand(&self, state: &Self::State, out: &mut Vec<Self::Choice>) {
        if state.lower.is_none() {
            out.extend((0..self.tetrachords.len()).map(ScaleGenerationChoice::Lower));
        } else if state.upper.is_none() {
            out.extend((0..self.tetrachords.len()).map(ScaleGenerationChoice::Upper));
        } else if state.join.is_none() {
            out.extend((0..self.joins.len()).map(ScaleGenerationChoice::Join));
        }
    }

    fn apply(&self, state: &Self::State, choice: &Self::Choice) -> SearchStep<Self::State> {
        let mut next = state.clone();
        match *choice {
            ScaleGenerationChoice::Lower(index) if index < self.tetrachords.len() => {
                next.lower = Some(index);
            }
            ScaleGenerationChoice::Upper(index) if index < self.tetrachords.len() => {
                next.upper = Some(index);
            }
            ScaleGenerationChoice::Join(index) if index < self.joins.len() => {
                next.join = Some(index);
            }
            _ => return SearchStep::infeasible("scale generation choice index outside axis"),
        }
        SearchStep::Continue(next)
    }

    fn finish(&self, state: &Self::State) -> Option<Self::Output> {
        let lower_index = state.lower?;
        let upper_index = state.upper?;
        let join_index = state.join?;
        let lower = self.tetrachords[lower_index];
        let upper = self.tetrachords[upper_index];
        let join = self.joins[join_index];
        let upper_root = lower.offsets[3].checked_add(join.semitones())?;
        let mut intervals = lower.offsets.to_vec();
        for offset in upper.offsets {
            let interval = upper_root.checked_add(offset)?;
            if interval == 12 {
                continue;
            }
            if interval > 12 {
                return None;
            }
            intervals.push(interval);
        }
        intervals.sort_unstable();
        intervals.dedup();
        if intervals.len() < 2 {
            return None;
        }
        let pitch_classes = intervals
            .iter()
            .map(|offset| PitchClass::C.transpose(i32::from(*offset)))
            .collect::<Vec<_>>();
        let mask = PitchClassMask::from_pitch_classes(&pitch_classes);
        let third_stack = decode_scale_third_stack(&intervals);
        let fixture = fixture_for(&intervals, third_stack.as_ref());
        Some(GeneratedScale {
            ordinal: lower_index * self.tetrachords.len() * self.joins.len()
                + upper_index * self.joins.len()
                + join_index,
            lower,
            upper,
            join,
            intervals,
            mask,
            third_stack,
            fixture,
        })
    }
}

fn fixture_for(intervals: &[u8], stack: Option<&ThirdStackSignature>) -> Option<char> {
    let stack = stack?;
    SCALE_FIXTURES
        .iter()
        .find(|fixture| fixture.intervals == intervals && fixture.third_stack_gaps == stack.steps)
        .map(|fixture| fixture.name)
}
