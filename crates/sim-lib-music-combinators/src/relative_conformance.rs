use std::collections::BTreeMap;

use sim_lib_music_core::{
    Articulation, AtomRef, Channel, Chord, ConversionLossKind, Music, MusicObject, Note, PianoRoll,
    Pitch, Time, TimedNote,
};

use crate::{
    CarpetAxis, CarpetIndex, CarpetPolicy, MusicCarpet, RelativeOrigin, RelativePolicy,
    RelativeReference, RelativeScope, decode_relative, encode_relative,
};

fn timed(midi: u8, onset: Time) -> TimedNote {
    TimedNote {
        onset,
        note: Note::new(
            Time::new(1, 4),
            Pitch::from_midi(midi),
            90,
            Channel::new(2).expect("channel"),
            Articulation::Legato,
        )
        .expect("note"),
    }
}

fn canonical_carpet() -> MusicCarpet {
    let cells = BTreeMap::from([
        (
            CarpetIndex::new(vec![0]),
            Music::PianoRoll(
                PianoRoll::new(vec![
                    timed(60, Time::from_integer(0)),
                    timed(64, Time::new(1, 4)),
                ])
                .expect("roll"),
            ),
        ),
        (
            CarpetIndex::new(vec![1]),
            Music::PianoRoll(PianoRoll::new(vec![timed(67, Time::new(1, 2))]).expect("roll")),
        ),
    ]);
    MusicCarpet::new(
        vec![CarpetAxis::new(
            "phrase",
            vec!["opening".to_owned(), "answer".to_owned()],
            false,
        )],
        cells,
        CarpetPolicy::STRICT,
    )
    .expect("carpet")
}

fn assert_canonical_cells_equal(left: &MusicCarpet, right: &MusicCarpet) {
    assert_eq!(left.axes, right.axes);
    assert_eq!(left.cells.len(), right.cells.len());
    for (index, left) in &left.cells {
        let right = right.cells.get(index).expect("same cell");
        match (left, right) {
            (Music::PianoRoll(left), Music::PianoRoll(right)) => assert_eq!(left, right),
            pair => panic!("expected canonical rolls, got {pair:?}"),
        }
    }
}

fn semantic_notes(music: &Music) -> Vec<(Time, Pitch, Time)> {
    let mut atoms = Vec::new();
    music.voices(Time::from_integer(0), &mut atoms);
    let mut notes = atoms
        .into_iter()
        .filter_map(|atom| match atom.atom {
            AtomRef::Note(note) => Some((atom.onset, note.pitch, note.duration)),
            _ => None,
        })
        .collect::<Vec<_>>();
    notes.sort();
    notes
}

#[test]
fn cell_previous_deltas_round_trip_canonical_music_exactly() {
    let source = canonical_carpet();
    let encoded = encode_relative(&source, RelativePolicy::CELL_DELTAS).expect("encode");
    assert!(encoded.is_lossless());

    let opening = &encoded.value.cells[&CarpetIndex::new(vec![0])];
    assert_eq!(
        opening.origin,
        Some(RelativeOrigin::new(
            Pitch::from_midi(60),
            Time::from_integer(0)
        ))
    );
    assert_eq!(opening.events[0].pitch_delta, 0);
    assert_eq!(opening.events[0].onset_delta, Time::from_integer(0));
    assert_eq!(opening.events[1].pitch_delta, 4);
    assert_eq!(opening.events[1].onset_delta, Time::new(1, 4));
    assert_eq!(
        encoded.value.cells[&CarpetIndex::new(vec![1])].events[0].pitch_delta,
        0
    );

    let decoded = decode_relative(&encoded.value, None).expect("decode");
    assert!(decoded.is_lossless());
    assert_canonical_cells_equal(&source, &decoded.value);
}

#[test]
fn carpet_anchor_uses_stable_rank_order_across_cells() {
    let source = canonical_carpet();
    let policy = RelativePolicy {
        scope: RelativeScope::Carpet,
        pitch: RelativeReference::Anchor,
        time: RelativeReference::Anchor,
    };
    let encoded = encode_relative(&source, policy).expect("encode");
    let opening = &encoded.value.cells[&CarpetIndex::new(vec![0])];
    let answer = &encoded.value.cells[&CarpetIndex::new(vec![1])];

    assert!(opening.origin.is_some());
    assert!(answer.origin.is_none());
    assert_eq!(
        opening
            .events
            .iter()
            .map(|event| event.pitch_delta)
            .chain(answer.events.iter().map(|event| event.pitch_delta))
            .collect::<Vec<_>>(),
        vec![0, 4, 7]
    );
    assert_eq!(answer.events[0].onset_delta, Time::new(1, 2));

    let decoded = decode_relative(&encoded.value, None).expect("decode");
    assert_canonical_cells_equal(&source, &decoded.value);
}

#[test]
fn external_origin_reports_loss_and_can_be_restored_by_context() {
    let source = canonical_carpet();
    let policy = RelativePolicy {
        scope: RelativeScope::External,
        pitch: RelativeReference::Previous,
        time: RelativeReference::Previous,
    };
    let encoded = encode_relative(&source, policy).expect("encode");
    assert_eq!(encoded.losses.len(), 1);
    assert_eq!(encoded.losses[0].kind, ConversionLossKind::RelativeAnchor);
    assert!(
        encoded
            .value
            .cells
            .values()
            .all(|cell| cell.origin.is_none())
    );

    let origin = RelativeOrigin::new(Pitch::from_midi(60), Time::from_integer(0));
    let restored = decode_relative(&encoded.value, Some(origin)).expect("context decode");
    assert!(restored.is_lossless());
    assert_canonical_cells_equal(&source, &restored.value);

    let normalized = decode_relative(&encoded.value, None).expect("zero decode");
    assert_eq!(
        normalized.losses[0].kind,
        ConversionLossKind::RelativeAnchor
    );
    let first = normalized.value.cells[&CarpetIndex::new(vec![0])].clone();
    assert_eq!(semantic_notes(&first)[0].1, Pitch::from_semitone(0));
}

#[test]
fn noncanonical_music_reports_structure_loss_but_preserves_semantics() {
    let chord = Music::Chord(
        Chord::new(
            Time::new(1, 2),
            "C",
            vec![Pitch::from_midi(60), Pitch::from_midi(64)],
            88,
            Channel::new(1).expect("channel"),
        )
        .expect("chord"),
    );
    let source = MusicCarpet::new(
        vec![CarpetAxis::new("x", vec!["one".to_owned()], false)],
        BTreeMap::from([(CarpetIndex::new(vec![0]), chord.clone())]),
        CarpetPolicy::STRICT,
    )
    .expect("carpet");

    let encoded = encode_relative(&source, RelativePolicy::CELL_DELTAS).expect("encode");
    assert_eq!(encoded.losses.len(), 1);
    assert_eq!(encoded.losses[0].kind, ConversionLossKind::SourceStructure);
    let decoded = decode_relative(&encoded.value, None).expect("decode");
    let reconstructed = &decoded.value.cells[&CarpetIndex::new(vec![0])];
    assert_eq!(semantic_notes(&chord), semantic_notes(reconstructed));
    assert_eq!(chord.duration(), reconstructed.duration());
}
// conformance: relative-music tests prove context resolution and transposition laws.
