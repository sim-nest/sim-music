use sim_lib_music_core::{Music, Time};
use sim_lib_pitch_core::{OctaveSpace, Pitch, PitchClass};
use sim_lib_pitch_scale::{Mode, Scale};

use crate::{
    MapCompositionWitness, MapError, MapWitness, PitchMap, PitchMapPolicy, PitchRemap,
    TransformDiagnosticCode, TuningRemap, compose_pitch_map_report, compose_pitch_maps, quarter,
    simple_melody,
};

// conformance: partial pitch maps compose and expose reversible witnesses.

fn roll_midis(music: &Music) -> Vec<u8> {
    let Music::PianoRoll(roll) = music else {
        panic!("piano roll");
    };
    roll.items
        .iter()
        .map(|item| item.note.pitch.to_midi().expect("midi pitch"))
        .collect()
}

fn sparse_map(pairs: &[(u16, i32)], policy: PitchMapPolicy) -> PitchMap {
    let domain = OctaveSpace::twelve_tone();
    let mut image = vec![None; usize::from(domain.len())];
    for (source, target) in pairs {
        image[usize::from(*source)] = Some(*target);
    }
    PitchMap::new(domain, image, policy).expect("pitch map")
}

#[test]
fn scale_pitch_map_policies_report_holes_and_inverse_witnesses() {
    let scale = Scale::major(PitchClass::C);
    let c_sharp = Pitch {
        class: PitchClass::CS,
        octave: 4,
    };

    let nearest = PitchMap::from_scale(scale, PitchMapPolicy::Nearest);
    let nearest_result = nearest.map_pitch(c_sharp).expect("nearest");
    assert_eq!(nearest_result.pitch, Pitch::from_midi(62));
    assert_eq!(
        nearest_result.witness,
        MapWitness::Nudged {
            source_class: 1,
            chosen_class: 2,
            target_value: 62,
            policy: PitchMapPolicy::Nearest,
        }
    );

    let clamp = PitchMap::from_scale(scale, PitchMapPolicy::Clamp);
    let clamp_result = clamp.map_pitch(c_sharp).expect("clamp");
    assert_eq!(clamp_result.pitch, Pitch::from_midi(60));
    assert_eq!(
        clamp_result.witness,
        MapWitness::Nudged {
            source_class: 1,
            chosen_class: 0,
            target_value: 60,
            policy: PitchMapPolicy::Clamp,
        }
    );

    let unmapped = PitchMap::from_scale(scale, PitchMapPolicy::Unmapped);
    let unmapped_result = unmapped.map_pitch(c_sharp).expect("unmapped");
    assert_eq!(unmapped_result.pitch, c_sharp);
    assert_eq!(
        unmapped_result.witness,
        MapWitness::Unmapped { source_class: 1 }
    );

    let rejected = PitchMap::from_scale(scale, PitchMapPolicy::Reject)
        .map_pitch(c_sharp)
        .unwrap_err();
    assert_eq!(rejected, MapError::Unmapped { class: 1 });

    let inverses = nearest.inverse_witnesses();
    assert_eq!(inverses.len(), 7);
    assert!(nearest.has_partial_inverse());
}

#[test]
fn scale_derivation_maps_expose_their_partiality() {
    let derivations = [
        (Scale::major(PitchClass::C), 7),
        (Scale::new(PitchClass::A, Mode::MinorNatural), 7),
        (Scale::whole_tone(PitchClass::C), 6),
        (Scale::chromatic(PitchClass::C), 12),
    ];

    for (scale, mapped_count) in derivations {
        let map = PitchMap::from_scale(scale, PitchMapPolicy::Reject);
        assert_eq!(map.inverse_witnesses().len(), mapped_count);
        assert_eq!(
            map.image.iter().filter(|target| target.is_some()).count(),
            mapped_count
        );
        assert_eq!(map.is_partial(), mapped_count != 12);
    }
}

#[test]
fn composed_pitch_maps_are_associative_where_defined_and_report_loss() {
    let a = sparse_map(&[(0, 2), (1, 3), (11, 12)], PitchMapPolicy::Reject);
    let b = sparse_map(&[(0, 0), (2, 4)], PitchMapPolicy::Reject);
    let c = sparse_map(&[(0, 2), (4, 7)], PitchMapPolicy::Reject);

    let ab = compose_pitch_maps(&a, &b).expect("a then b");
    let bc = compose_pitch_maps(&b, &c).expect("b then c");
    let left = compose_pitch_maps(&ab, &c).expect("(a b) c");
    let right = compose_pitch_maps(&a, &bc).expect("a (b c)");
    assert_eq!(left.image, right.image);

    let report = compose_pitch_map_report(&a, &b).expect("report");
    assert!(
        report
            .witnesses
            .contains(&MapCompositionWitness::Undefined {
                source_class: 1,
                reason: "right map has no image",
            })
    );
    assert!(report.witnesses.contains(&MapCompositionWitness::Direct {
        source_class: 0,
        via_value: 2,
        target_value: 4,
    }));
}

#[test]
fn pitch_remap_map_variant_preserves_time_and_reports_rejections() {
    let melody = simple_melody(&[
        (60, quarter()),
        (61, Time::from_integer(2)),
        (62, quarter()),
    ]);
    let scale = Scale::major(PitchClass::C);
    let nearest = PitchRemap::Map(PitchMap::from_scale(scale, PitchMapPolicy::Nearest))
        .apply_report(&melody)
        .expect("nearest report");
    assert_eq!(roll_midis(&nearest.music), vec![60, 62, 62]);
    assert!(!nearest.has_diagnostics());

    let rejected = PitchRemap::Map(PitchMap::from_scale(scale, PitchMapPolicy::Reject))
        .apply_report(&melody)
        .expect("reject report");
    assert_eq!(roll_midis(&rejected.music), vec![60, 61, 62]);
    assert_eq!(rejected.diagnostics.len(), 1);
    assert_eq!(
        rejected.diagnostics[0].code,
        TransformDiagnosticCode::UnsupportedMapping
    );

    let Music::PianoRoll(roll) = nearest.music else {
        panic!("piano roll");
    };
    assert_eq!(roll.items[1].note.duration, Time::from_integer(2));
}

#[test]
fn map_helpers_cover_transpose_inversion_rotation_and_negative_octaves() {
    let transpose = PitchMap::chromatic_delta(2);
    assert_eq!(
        transpose.map_pitch(Pitch::from_midi(60)).unwrap().pitch,
        Pitch::from_midi(62)
    );

    let inversion = PitchMap::inversion(PitchClass::C);
    assert_eq!(
        inversion.map_pitch(Pitch::from_midi(64)).unwrap().pitch,
        Pitch::from_midi(68)
    );

    let rotation = PitchMap::rotation(OctaveSpace::twelve_tone(), 5, PitchMapPolicy::Reject);
    assert_eq!(
        rotation.map_pitch(Pitch::from_midi(60)).unwrap().pitch,
        Pitch::from_midi(65)
    );

    let tuning = TuningRemap::new(200).pitch_map();
    assert_eq!(
        tuning.map_pitch(Pitch::from_midi(60)).unwrap().pitch,
        Pitch::from_midi(62)
    );
    let tuning_then_rotation = compose_pitch_maps(&tuning, &rotation).unwrap();
    assert_eq!(
        tuning_then_rotation
            .map_pitch(Pitch::from_midi(60))
            .unwrap()
            .pitch,
        Pitch::from_midi(67)
    );
    assert_eq!(
        PitchRemap::PitchClass {
            from: PitchClass::C,
            to: PitchClass::G,
        }
        .as_pitch_map()
        .unwrap()
        .map_pitch(Pitch::from_midi(60))
        .unwrap()
        .pitch,
        Pitch::from_midi(67)
    );

    let negative = sparse_map(&[(11, 12)], PitchMapPolicy::Reject);
    assert_eq!(
        negative
            .map_pitch(Pitch::from_semitone(-1))
            .expect("negative octave")
            .pitch,
        Pitch::from_semitone(0)
    );
}
