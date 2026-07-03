use sim_lib_pitch_set::PitchClassMask;

use crate::{FORTE_TABLE, lookup_forte_label};

#[test]
fn vendored_prime_forms_all_label() {
    for entry in FORTE_TABLE {
        assert_eq!(lookup_forte_label(entry.mask), Some(entry.label));
        assert_eq!(
            lookup_forte_label(entry.mask.normalize()),
            Some(entry.label)
        );
    }
}

#[test]
fn unknown_mask_is_unlabeled() {
    assert_eq!(lookup_forte_label(PitchClassMask(0b1111_1111_1111)), None);
}
