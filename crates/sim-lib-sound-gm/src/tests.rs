use crate::{DrumKeyMap, DrumSound, general_midi_bank};

#[test]
fn default_bank_covers_general_midi_programs() {
    let bank = general_midi_bank();
    for program in [0, 32, 64, 96, 127] {
        assert!(!bank.get(0, 0, program).name.is_empty());
    }
}

#[test]
fn drum_key_map_resolves_gm_aliases_named_kits_and_custom_maps() {
    let gm = DrumKeyMap::gm();
    assert_eq!(gm.resolve("kick"), Some(36));
    assert_eq!(gm.resolve("Closed Hi-Hat"), Some(42));
    assert_eq!(gm.resolve("midi-38"), Some(38));
    assert!(gm.aliases_for(36).contains(&"bd"));

    let kit = DrumKeyMap::named_kit("four-on-floor").expect("named kit");
    assert_eq!(kit.resolve("open-hat"), Some(46));

    let custom = DrumKeyMap::custom(
        "custom",
        [DrumSound::new(40, "Deep Snare", ["snare", "backbeat"])],
    );
    assert_eq!(custom.remap("backbeat", 38), 40);
    assert_eq!(custom.remap("missing", 38), 38);
}
