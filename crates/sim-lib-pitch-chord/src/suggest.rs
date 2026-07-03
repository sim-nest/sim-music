use sim_lib_pitch_core::PitchClass;
use sim_lib_pitch_namer_roman::label_roman;
use sim_lib_pitch_scale::{Key, Scale};

use crate::ChordSymbol;

/// The tonal function a suggested chord plays within the key.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum HarmonicFunction {
    /// Tonic function (rest, home).
    Tonic,
    /// Predominant function (motion toward the dominant).
    Predominant,
    /// Subdominant function.
    Subdominant,
    /// Dominant function (tension resolving to tonic).
    Dominant,
    /// Mediant function.
    Mediant,
    /// Leading-tone function.
    LeadingTone,
    /// A chromatic substitution outside the diatonic functions.
    ChromaticSubstitution,
}

impl HarmonicFunction {
    /// Returns the canonical wire label for this function (for example `"tonic"`).
    pub fn wire_label(self) -> &'static str {
        match self {
            Self::Tonic => "tonic",
            Self::Predominant => "predominant",
            Self::Subdominant => "subdominant",
            Self::Dominant => "dominant",
            Self::Mediant => "mediant",
            Self::LeadingTone => "leading-tone",
            Self::ChromaticSubstitution => "chromatic-substitution",
        }
    }
}

/// A suggested next chord, with its roman label, ranking score, function, and
/// whether it is a substitution.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct HarmonicSuggestion {
    /// The suggested chord.
    pub chord: ChordSymbol,
    /// The roman-numeral label of the chord in the key.
    pub roman: String,
    /// The ranking score; higher is a stronger suggestion.
    pub score: u16,
    /// The chord's tonal function.
    pub function: HarmonicFunction,
    /// Whether this is a substitution rather than a plain diatonic move.
    pub substitution: bool,
}

/// The input to [`suggest_harmony`]: the key, the current chord, and how many
/// candidates to return.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct HarmonicSuggestionContext {
    /// The key the progression is in.
    pub key: Key,
    /// The chord currently sounding.
    pub current: ChordSymbol,
    /// The maximum number of suggestions to return.
    pub max_candidates: usize,
}

impl HarmonicSuggestionContext {
    /// Constructs a suggestion context for `key` and the `current` chord, defaulting
    /// to six candidates.
    pub fn new(key: Key, current: ChordSymbol) -> Self {
        Self {
            key,
            current,
            max_candidates: 6,
        }
    }

    /// Sets the maximum number of suggestions to return.
    pub fn with_max_candidates(mut self, max_candidates: usize) -> Self {
        self.max_candidates = max_candidates;
        self
    }
}

/// Suggests likely next chords for the `context`, ranked by tonal-function
/// heuristics, returning at most `max_candidates` suggestions.
pub fn suggest_harmony(context: HarmonicSuggestionContext) -> Vec<HarmonicSuggestion> {
    if context.max_candidates == 0 {
        return Vec::new();
    }
    let scale = Scale::new(context.key.tonic, context.key.mode);
    let current_degree = scale.degree_of(context.current.root).unwrap_or(1);
    let mut suggestions = Vec::new();
    for candidate in degree_candidates(current_degree) {
        let chord = diatonic_chord(scale, candidate.degree);
        push_suggestion(
            &mut suggestions,
            context.key,
            chord,
            candidate.score,
            function_for_degree(candidate.degree),
            candidate.substitution,
            None,
        );
    }
    for candidate in substitution_candidates(current_degree, context.current.root) {
        push_suggestion(
            &mut suggestions,
            context.key,
            candidate.chord,
            candidate.score,
            candidate.function,
            true,
            Some(candidate.label),
        );
    }
    suggestions.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then_with(|| left.chord.root.cmp(&right.chord.root))
            .then_with(|| left.chord.quality.cmp(right.chord.quality))
            .then_with(|| left.roman.cmp(&right.roman))
    });
    suggestions.truncate(context.max_candidates);
    suggestions
}

fn push_suggestion(
    suggestions: &mut Vec<HarmonicSuggestion>,
    key: Key,
    chord: ChordSymbol,
    score: u16,
    function: HarmonicFunction,
    substitution: bool,
    label: Option<&'static str>,
) {
    if suggestions
        .iter()
        .any(|entry| entry.chord.root == chord.root && entry.chord.quality == chord.quality)
    {
        return;
    }
    let roman = label
        .map(str::to_owned)
        .unwrap_or_else(|| roman_label(key, &chord));
    suggestions.push(HarmonicSuggestion {
        chord,
        roman,
        score,
        function,
        substitution,
    });
}

fn roman_label(key: Key, chord: &ChordSymbol) -> String {
    label_roman(
        chord.to_chord(4).pitch_classes(),
        Some(key),
        Some(chord.root),
    )
    .unwrap_or_else(|_| chord.wire_label())
}

#[derive(Copy, Clone)]
struct DegreeCandidate {
    degree: usize,
    score: u16,
    substitution: bool,
}

fn degree_candidates(degree: usize) -> Vec<DegreeCandidate> {
    match degree {
        1 => vec![
            DegreeCandidate::new(5, 100, false),
            DegreeCandidate::new(4, 92, false),
            DegreeCandidate::new(6, 84, true),
            DegreeCandidate::new(2, 78, true),
        ],
        2 => vec![
            DegreeCandidate::new(5, 100, false),
            DegreeCandidate::new(7, 88, true),
            DegreeCandidate::new(4, 82, true),
            DegreeCandidate::new(1, 72, false),
        ],
        3 => vec![
            DegreeCandidate::new(6, 98, false),
            DegreeCandidate::new(4, 88, true),
            DegreeCandidate::new(1, 78, true),
            DegreeCandidate::new(5, 72, false),
        ],
        4 => vec![
            DegreeCandidate::new(5, 100, false),
            DegreeCandidate::new(2, 90, true),
            DegreeCandidate::new(1, 82, false),
            DegreeCandidate::new(6, 74, true),
        ],
        5 => vec![
            DegreeCandidate::new(1, 110, false),
            DegreeCandidate::new(6, 94, true),
            DegreeCandidate::new(4, 82, true),
            DegreeCandidate::new(2, 74, false),
        ],
        6 => vec![
            DegreeCandidate::new(2, 100, false),
            DegreeCandidate::new(4, 90, true),
            DegreeCandidate::new(5, 84, false),
            DegreeCandidate::new(1, 76, true),
        ],
        7 => vec![
            DegreeCandidate::new(1, 110, false),
            DegreeCandidate::new(5, 88, true),
            DegreeCandidate::new(3, 76, false),
        ],
        _ => vec![DegreeCandidate::new(1, 100, false)],
    }
}

impl DegreeCandidate {
    const fn new(degree: usize, score: u16, substitution: bool) -> Self {
        Self {
            degree,
            score,
            substitution,
        }
    }
}

#[derive(Clone)]
struct SubstitutionCandidate {
    chord: ChordSymbol,
    score: u16,
    function: HarmonicFunction,
    label: &'static str,
}

fn substitution_candidates(degree: usize, root: PitchClass) -> Vec<SubstitutionCandidate> {
    if degree != 5 {
        return Vec::new();
    }
    vec![SubstitutionCandidate {
        chord: ChordSymbol {
            root: root.transpose(6),
            quality: "7",
            slash_bass: None,
        },
        score: 70,
        function: HarmonicFunction::ChromaticSubstitution,
        label: "subV7",
    }]
}

fn diatonic_chord(scale: Scale, degree: usize) -> ChordSymbol {
    let root = scale.pitch_at_degree(degree);
    ChordSymbol {
        root,
        quality: triad_quality(scale, degree),
        slash_bass: None,
    }
}

fn triad_quality(scale: Scale, degree: usize) -> &'static str {
    let root = scale.pitch_at_degree(degree);
    let third = scale.pitch_at_degree(degree + 2);
    let fifth = scale.pitch_at_degree(degree + 4);
    match (interval(root, third), interval(root, fifth)) {
        (4, 7) => "maj",
        (3, 7) => "m",
        (3, 6) => "dim",
        _ => "maj",
    }
}

fn interval(root: PitchClass, pitch: PitchClass) -> u8 {
    (i32::from(pitch.0) - i32::from(root.0)).rem_euclid(12) as u8
}

fn function_for_degree(degree: usize) -> HarmonicFunction {
    match degree {
        1 | 6 => HarmonicFunction::Tonic,
        2 => HarmonicFunction::Predominant,
        3 => HarmonicFunction::Mediant,
        4 => HarmonicFunction::Subdominant,
        5 => HarmonicFunction::Dominant,
        7 => HarmonicFunction::LeadingTone,
        _ => HarmonicFunction::Tonic,
    }
}
