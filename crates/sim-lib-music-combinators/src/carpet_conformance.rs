use std::collections::BTreeMap;

use sim_lib_music_core::{Articulation, Channel, Music, MusicObject, Note, Pitch, Time};
use sim_lib_music_transform::{PitchDelta, TransformChain, TransformStep, TransposeTransform};
use sim_lib_rank::Nat;

use crate::{
    CarpetAxis, CarpetError, CarpetIndex, CarpetPolicy, EmptyPolicy, MusicCarpet, OutOfRangePolicy,
    OverlayPolicy, RaggedPolicy, SlicePolicy,
};

fn axis(name: &str, labels: &[&str], cyclic: bool) -> CarpetAxis {
    CarpetAxis::new(
        name,
        labels.iter().map(|label| (*label).to_owned()).collect(),
        cyclic,
    )
}

fn note(midi: u8) -> Music {
    Music::Note(
        Note::new(
            Time::new(1, 4),
            Pitch::from_midi(midi),
            100,
            Channel::new(0).expect("channel"),
            Articulation::Normal,
        )
        .expect("note"),
    )
}

fn full_carpet() -> MusicCarpet {
    let axes = vec![
        axis("row", &["north", "south"], false),
        axis("column", &["west", "center", "east"], false),
    ];
    let cells = (0..2)
        .flat_map(|row| {
            (0..3).map(move |column| {
                (
                    CarpetIndex::new(vec![row, column]),
                    note(60 + (row * 3 + column) as u8),
                )
            })
        })
        .collect();
    MusicCarpet::new(axes, cells, CarpetPolicy::STRICT).expect("full carpet")
}

fn midi_signature(carpet: &MusicCarpet) -> Vec<(Vec<usize>, Vec<u8>)> {
    carpet
        .cells
        .iter()
        .map(|(index, music)| {
            let mut atoms = Vec::new();
            music.voices(Time::from_integer(0), &mut atoms);
            let pitches = atoms
                .into_iter()
                .filter_map(|atom| match atom.atom {
                    sim_lib_music_core::AtomRef::Note(note) => note.pitch.to_midi(),
                    _ => None,
                })
                .collect();
            (index.coordinates.clone(), pitches)
        })
        .collect()
}

#[test]
fn mixed_radix_addresses_are_stable_and_round_trip() {
    let carpet = full_carpet();
    let index = CarpetIndex::new(vec![1, 2]);
    let ordinal = carpet.rank_index(&index).expect("rank");

    assert_eq!(ordinal.to_decimal_string(), "5");
    assert_eq!(carpet.index_at_rank(&ordinal).expect("unrank"), index);
    assert!(matches!(
        carpet.index_at_rank(&Nat::from(6_u64)),
        Err(CarpetError::Rank(_))
    ));
}

#[test]
fn empty_ragged_and_out_of_range_policies_are_explicit() {
    assert_eq!(
        MusicCarpet::new(Vec::new(), BTreeMap::new(), CarpetPolicy::STRICT)
            .expect_err("strict empty"),
        CarpetError::EmptyCarpet
    );
    let empty_policy = CarpetPolicy {
        empty: EmptyPolicy::Allow,
        ragged: RaggedPolicy::Sparse,
        out_of_range: OutOfRangePolicy::Reject,
    };
    let empty = MusicCarpet::new(Vec::new(), BTreeMap::new(), empty_policy).expect("empty value");
    assert!(matches!(
        empty.rank_index(&CarpetIndex::default()),
        Err(CarpetError::Rank(_))
    ));

    let axes = vec![
        axis("cycle", &["a", "b"], true),
        axis("line", &["x", "y"], false),
    ];
    let one_cell = BTreeMap::from([(CarpetIndex::new(vec![0, 0]), note(60))]);
    assert!(matches!(
        MusicCarpet::new(axes.clone(), one_cell.clone(), CarpetPolicy::STRICT),
        Err(CarpetError::Ragged {
            expected: 4,
            actual: 1
        })
    ));
    MusicCarpet::new(axes.clone(), one_cell, CarpetPolicy::SPARSE).expect("sparse carpet");

    let wrapped_policy = CarpetPolicy {
        out_of_range: OutOfRangePolicy::WrapCyclic,
        ..CarpetPolicy::SPARSE
    };
    let wrapped = MusicCarpet::new(
        axes.clone(),
        BTreeMap::from([(CarpetIndex::new(vec![2, 1]), note(61))]),
        wrapped_policy,
    )
    .expect("cyclic coordinate");
    assert!(wrapped.cells.contains_key(&CarpetIndex::new(vec![0, 1])));

    assert!(matches!(
        MusicCarpet::new(
            axes,
            BTreeMap::from([(CarpetIndex::new(vec![0, 2]), note(61))]),
            wrapped_policy,
        ),
        Err(CarpetError::CoordinateOutOfRange { axis: 1, .. })
    ));
}

#[test]
fn rotations_reflections_and_slices_obey_algebraic_fixtures() {
    let carpet = full_carpet();
    let rotated = carpet.rotate(0, 1, 4).expect("four rotations");
    assert_eq!(rotated.axes, carpet.axes);
    assert_eq!(midi_signature(&rotated), midi_signature(&carpet));

    let quarter = carpet.rotate(0, 1, 1).expect("quarter turn");
    assert_eq!(quarter.axes[0].name, "column");
    assert_eq!(quarter.axes[1].name, "row");
    assert_eq!(quarter.axes[1].labels, vec!["south", "north"]);
    assert_eq!(
        quarter
            .cell(&CarpetIndex::new(vec![0, 1]))
            .map(|music| music.kind()),
        Some("Note")
    );

    let reflected = carpet
        .reflect(1)
        .and_then(|value| value.reflect(1))
        .expect("double reflection");
    assert_eq!(reflected.axes, carpet.axes);
    assert_eq!(midi_signature(&reflected), midi_signature(&carpet));

    let cyclic = MusicCarpet::new(
        vec![axis("cycle", &["a", "b", "c"], true)],
        BTreeMap::from([
            (CarpetIndex::new(vec![0]), note(60)),
            (CarpetIndex::new(vec![1]), note(61)),
            (CarpetIndex::new(vec![2]), note(62)),
        ]),
        CarpetPolicy::STRICT,
    )
    .expect("cyclic carpet");
    let wrapped = cyclic
        .slice(0, 2, 4, SlicePolicy::WrapCyclic)
        .expect("wrapped slice");
    assert_eq!(wrapped.axes[0].labels, vec!["c", "a", "b", "c"]);
    assert_eq!(
        midi_signature(&wrapped),
        vec![
            (vec![0], vec![62]),
            (vec![1], vec![60]),
            (vec![2], vec![61]),
            (vec![3], vec![62]),
        ]
    );
    assert!(matches!(
        cyclic.slice(0, 2, 4, SlicePolicy::Strict),
        Err(CarpetError::InvalidSlice { .. })
    ));
    assert_eq!(
        cyclic
            .slice(0, 2, 4, SlicePolicy::Clamp)
            .expect("clamped")
            .axes[0]
            .labels,
        vec!["c"]
    );
}

#[test]
fn overlays_and_music_transform_chains_compose_existing_music() {
    let axes = vec![axis("x", &["only"], false)];
    let base = MusicCarpet::new(
        axes.clone(),
        BTreeMap::from([(CarpetIndex::new(vec![0]), note(60))]),
        CarpetPolicy::STRICT,
    )
    .expect("base");
    let top = MusicCarpet::new(
        axes,
        BTreeMap::from([(CarpetIndex::new(vec![0]), note(67))]),
        CarpetPolicy::STRICT,
    )
    .expect("top");

    assert!(matches!(
        base.overlay(&top, OverlayPolicy::Reject),
        Err(CarpetError::OverlayCollision { .. })
    ));
    let parallel = base
        .overlay(&top, OverlayPolicy::Parallel)
        .expect("parallel overlay");
    assert_eq!(midi_signature(&parallel), vec![(vec![0], vec![60, 67])]);

    let chain = TransformChain::new(vec![TransformStep::Transpose(TransposeTransform::new(
        PitchDelta::Semitones(12),
    ))]);
    let transformed = parallel.apply_transform(&chain).expect("transform chain");
    assert!(transformed.diagnostics.is_empty());
    assert_eq!(
        midi_signature(&transformed.carpet),
        vec![(vec![0], vec![72, 79])]
    );
}
