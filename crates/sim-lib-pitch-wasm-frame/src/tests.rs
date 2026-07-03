use super::*;

#[test]
fn analyze_pitch_text_returns_five_labels_and_scores() {
    let report = analyze_pitch_text("C4 E4 G4 Bb4 D5 F5 A5").unwrap();
    assert_eq!(report.labels.len(), 5);
    assert_eq!(report.dissonance.len(), 4);
    assert!(
        report
            .canonical_mask
            .starts_with("#(pitch/PitchClassMask v1 ")
    );
    assert!(report.canonical_mask.contains("#(PitchClassMask "));
}

#[test]
fn analyze_pitch_text_rejects_empty_input() {
    let err = analyze_pitch_text("  ").unwrap_err();
    assert!(matches!(err, PitchWasmError::EmptyInput));
}
