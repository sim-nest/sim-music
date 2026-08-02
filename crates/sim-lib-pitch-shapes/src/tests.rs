use super::*;
use sim_codec::{Input, decode_eval_expr_with_codec};
use sim_codec_lisp::LispCodecLib;
use sim_kernel::{CapabilitySet, ReadPolicy, TrustLevel};
use sim_kernel::{Cx, DefaultFactory, EagerPolicy, Expr, Symbol, read_construct_capability};
use sim_lib_pitch_core::PitchClass;
use sim_lib_pitch_scale::{Key, Mode, Scale};
use sim_lib_pitch_set::PitchClassMask;
use std::sync::Arc;

#[test]
fn pitch_round_trip() {
    let pitch = decode_pitch("Eb5").unwrap();
    assert_eq!(pitch.class, PitchClass::DS);
    assert_eq!(decode_pitch(&encode_pitch(pitch)).unwrap(), pitch);
}

#[test]
fn interval_round_trip() {
    let interval = decode_interval("P5").unwrap();
    assert_eq!(
        decode_interval(&encode_interval(interval)).unwrap(),
        interval
    );
}

#[test]
fn pitch_class_mask_round_trip() {
    let mask = PitchClassMask::new(145).unwrap();
    assert_eq!(
        decode_pitch_class_mask(&encode_pitch_class_mask(mask)).unwrap(),
        mask
    );
}

#[test]
fn pitch_class_mask_decode_rejects_high_bits() {
    assert!(matches!(
        decode_pitch_class_mask("#(PitchClassMask 4096)"),
        Err(PitchShapeError::InvalidPitchClassMask)
    ));
}

#[test]
fn scale_and_key_round_trip() {
    let scale = Scale::major(PitchClass::C);
    assert_eq!(decode_scale(&encode_scale(scale)).unwrap(), scale);
    let key = Key {
        tonic: PitchClass::D,
        mode: Mode::Dorian,
    };
    assert_eq!(decode_key(&encode_key(key)).unwrap(), key);
}

#[test]
fn chord_and_symbol_round_trip() {
    let symbol = decode_chord_symbol("Cm7/G").unwrap();
    assert_eq!(symbol.root, PitchClass::C);
    assert_eq!(
        decode_chord_symbol(&encode_chord_symbol(&symbol)).unwrap(),
        symbol
    );
    let chord = symbol.to_chord(4);
    assert_eq!(
        decode_chord(&encode_chord(&chord)).unwrap().pitches(),
        chord.pitches()
    );
}

#[test]
fn tone_row_round_trip_and_aggregate_refusal() {
    let form = "4,5,7,1,6,3,8,2,11,0,9,10";
    let row = decode_tone_row(form).unwrap();
    assert_eq!(encode_tone_row(&row), form);
    assert!(matches!(
        decode_tone_row("0,1,2"),
        Err(PitchShapeError::InvalidToneRow)
    ));
    assert!(matches!(
        decode_tone_row("0,0,0,0,0,0,0,0,0,0,0,0"),
        Err(PitchShapeError::Row(_))
    ));
}

#[test]
fn install_pitch_shapes_lib_registers_runtime_shape_exports() {
    let mut cx = Cx::new(Arc::new(EagerPolicy), Arc::new(DefaultFactory));
    install_pitch_shapes_lib(&mut cx).unwrap();
    install_pitch_shapes_lib(&mut cx).unwrap();
    let shape = cx
        .registry()
        .shape_by_symbol(&Symbol::qualified("pitch", "Pitch"))
        .expect("pitch shape")
        .clone();
    let doc = shape
        .object()
        .as_shape()
        .expect("shape protocol")
        .describe(&mut cx)
        .unwrap();
    assert_eq!(doc.name, "Pitch");
}

#[test]
fn pitch_shapes_reject_invalid_values() {
    let mut cx = Cx::new(Arc::new(EagerPolicy), Arc::new(DefaultFactory));
    install_pitch_shapes_lib(&mut cx).unwrap();

    let pitch = cx
        .registry()
        .shape_by_symbol(&Symbol::qualified("pitch", "Pitch"))
        .expect("pitch shape")
        .clone();
    let pitch_shape = pitch.object().as_shape().expect("shape protocol");
    assert!(!pitch_shape.is_total());
    assert!(
        pitch_shape
            .check_expr(&mut cx, &Expr::String("C4".to_owned()))
            .unwrap()
            .accepted
    );
    assert!(
        !pitch_shape
            .check_expr(&mut cx, &Expr::Bool(false))
            .unwrap()
            .accepted
    );

    let mask = cx
        .registry()
        .shape_by_symbol(&Symbol::qualified("pitch", "PitchClassMask"))
        .expect("pitch-class mask shape")
        .clone();
    let mask_shape = mask.object().as_shape().expect("shape protocol");
    assert!(
        mask_shape
            .check_expr(&mut cx, &Expr::String("#(PitchClassMask 145)".to_owned()))
            .unwrap()
            .accepted
    );
    assert!(
        !mask_shape
            .check_expr(&mut cx, &Expr::String("#(Interval 7)".to_owned()))
            .unwrap()
            .accepted
    );
}

#[test]
fn pitch_citizens_accept_legacy_text_and_read_construct() {
    let mut cx = cx_with_citizens();

    let pitch = read_construct::<PitchDescriptor>(&mut cx, pitch_class_symbol(), "Eb5");
    assert_eq!(pitch.pitch().unwrap(), decode_pitch("Eb5").unwrap());
    assert_eq!(
        PitchDescriptor::read_construct_expr_from_text("Eb5").unwrap(),
        read_construct_expr(pitch_class_symbol(), pitch.as_text())
    );

    let interval =
        read_construct::<PitchIntervalDescriptor>(&mut cx, pitch_interval_class_symbol(), "P5");
    assert_eq!(interval.interval().unwrap(), decode_interval("P5").unwrap());

    let mask = read_construct::<PitchClassMaskDescriptor>(
        &mut cx,
        pitch_class_mask_class_symbol(),
        "#(PitchClassMask 145)",
    );
    assert_eq!(
        mask.mask().unwrap(),
        decode_pitch_class_mask("#(PitchClassMask 145)").unwrap()
    );

    let scale =
        read_construct::<PitchScaleDescriptor>(&mut cx, pitch_scale_class_symbol(), "C:major");
    assert_eq!(scale.scale().unwrap(), decode_scale("C:major").unwrap());

    let chord =
        read_construct::<PitchChordDescriptor>(&mut cx, pitch_chord_class_symbol(), "C4,E4,G4");
    assert_eq!(
        chord.chord().unwrap().pitches(),
        decode_chord("C4,E4,G4").unwrap().pitches()
    );

    let row = read_construct::<ToneRowDescriptor>(
        &mut cx,
        tone_row_class_symbol(),
        "4,5,7,1,6,3,8,2,11,0,9,10",
    );
    assert_eq!(
        encode_tone_row(&row.row().unwrap()),
        "4,5,7,1,6,3,8,2,11,0,9,10"
    );
}

#[test]
fn lisp_serial_surface_builds_rows_matrices_and_reports() {
    let mut cx = Cx::new(Arc::new(EagerPolicy), Arc::new(DefaultFactory));
    install_pitch_shapes_lib(&mut cx).unwrap();
    let lisp = LispCodecLib::new(cx.registry_mut().fresh_codec_id()).unwrap();
    cx.load_lib(&lisp).unwrap();

    let expr = decode_eval_expr_with_codec(
        &mut cx,
        &Symbol::qualified("codec", "lisp"),
        Input::Text("(serial/analyze-row (serial/row '(4 5 7 1 6 3 8 2 11 0 9 10)))".to_owned()),
        ReadPolicy {
            trust: TrustLevel::TrustedSource,
            capabilities: CapabilitySet::new(),
        },
    )
    .unwrap();
    let report = cx.eval_expr(expr).unwrap();
    let Expr::Map(fields) = report.object().as_expr(&mut cx).unwrap() else {
        panic!("serial/analyze-row must return a report map");
    };
    assert_eq!(
        fields
            .iter()
            .find(|(key, _)| key == &Expr::Symbol(Symbol::new("form")))
            .map(|(_, value)| value),
        Some(&Expr::String("RowClassReport".to_owned()))
    );
    assert_eq!(
        fields
            .iter()
            .find(|(key, _)| key == &Expr::Symbol(Symbol::new("row")))
            .map(|(_, value)| value),
        Some(&Expr::String("4,5,7,1,6,3,8,2,11,0,9,10".to_owned()))
    );

    let expr = decode_eval_expr_with_codec(
        &mut cx,
        &Symbol::qualified("codec", "lisp"),
        Input::Text(
            "(serial/matrix (serial/row '(4 5 7 1 6 3 8 2 11 0 9 10)) :labels :first-last-pitch)"
                .to_owned(),
        ),
        ReadPolicy {
            trust: TrustLevel::TrustedSource,
            capabilities: CapabilitySet::new(),
        },
    )
    .unwrap();
    let matrix = cx.eval_expr(expr).unwrap();
    let Expr::Map(fields) = matrix.object().as_expr(&mut cx).unwrap() else {
        panic!("serial/matrix must return a matrix map");
    };
    assert_eq!(
        fields
            .iter()
            .find(|(key, _)| key == &Expr::Symbol(Symbol::new("label-convention")))
            .map(|(_, value)| value),
        Some(&Expr::String("first-last-pitch".to_owned()))
    );
}

fn cx_with_citizens() -> Cx {
    let mut cx = Cx::new(Arc::new(EagerPolicy), Arc::new(DefaultFactory));
    cx.load_lib(&sim_citizen::CitizenLib::all()).unwrap();
    cx.grant(read_construct_capability());
    cx
}

fn read_construct<T>(cx: &mut Cx, class: Symbol, form: &str) -> T
where
    T: Clone + 'static,
{
    let args = [
        Expr::Symbol(Symbol::new("v1")),
        Expr::String(form.to_owned()),
    ]
    .iter()
    .map(|expr| sim_citizen::value_from_expr(cx, expr))
    .collect::<sim_kernel::Result<Vec<_>>>()
    .unwrap();
    cx.read_construct(&class, args)
        .unwrap()
        .object()
        .downcast_ref::<T>()
        .unwrap()
        .clone()
}

fn read_construct_expr(class: Symbol, form: &str) -> Expr {
    Expr::Extension {
        tag: Symbol::qualified("citizen", "read-construct"),
        payload: Box::new(Expr::Vector(vec![
            Expr::Symbol(class),
            Expr::Symbol(Symbol::new("v1")),
            Expr::String(form.to_owned()),
        ])),
    }
}
