use sim_lib_pitch_set::PitchClassMask;

/// One entry in the Forte table: a prime-form mask and its Forte set-class name.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ForteEntry {
    /// The pitch-class set in prime form.
    pub mask: PitchClassMask,
    /// The Forte set-class name (for example `"4-27"`).
    pub label: &'static str,
}

/// The table of known Forte set classes, keyed by prime-form mask.
pub const FORTE_TABLE: &[ForteEntry] = &[
    ForteEntry {
        mask: PitchClassMask(0b0000_0000_0001),
        label: "1-1",
    },
    ForteEntry {
        mask: PitchClassMask(0b0000_0000_0011),
        label: "2-1",
    },
    ForteEntry {
        mask: PitchClassMask(0b0000_0000_0101),
        label: "2-2",
    },
    ForteEntry {
        mask: PitchClassMask(0b0000_0000_0111),
        label: "3-1",
    },
    ForteEntry {
        mask: PitchClassMask(0b0000_0001_0001),
        label: "2-4",
    },
    ForteEntry {
        mask: PitchClassMask(0b0000_0001_0011),
        label: "3-3",
    },
    ForteEntry {
        mask: PitchClassMask(0b0000_0001_0101),
        label: "3-5",
    },
    ForteEntry {
        mask: PitchClassMask(0b0000_0001_0111),
        label: "4-3",
    },
    ForteEntry {
        mask: PitchClassMask(0b0000_0010_0101),
        label: "3-7",
    },
    ForteEntry {
        mask: PitchClassMask(0b0000_0010_1011),
        label: "4-11",
    },
    ForteEntry {
        mask: PitchClassMask(0b0000_0100_1001),
        label: "3-11",
    },
    ForteEntry {
        mask: PitchClassMask(0b0000_0100_1011),
        label: "4-18",
    },
    ForteEntry {
        mask: PitchClassMask(0b0000_1001_0001),
        label: "3-12",
    },
    ForteEntry {
        mask: PitchClassMask(0b0000_1001_0011),
        label: "4-20",
    },
    ForteEntry {
        mask: PitchClassMask(0b0000_1001_0101),
        label: "4-26",
    },
    ForteEntry {
        mask: PitchClassMask(0b0000_1001_1011),
        label: "5-27",
    },
    ForteEntry {
        mask: PitchClassMask(0b0001_0001_0101),
        label: "4-27",
    },
    ForteEntry {
        mask: PitchClassMask(0b0001_0001_1011),
        label: "5-28",
    },
    ForteEntry {
        mask: PitchClassMask(0b0001_0010_0101),
        label: "4-23",
    },
    ForteEntry {
        mask: PitchClassMask(0b0001_0010_1011),
        label: "5-23",
    },
    ForteEntry {
        mask: PitchClassMask(0b0010_0101_0101),
        label: "4-21",
    },
    ForteEntry {
        mask: PitchClassMask(0b0010_0101_1011),
        label: "5-z17",
    },
    ForteEntry {
        mask: PitchClassMask(0b0100_1001_0011),
        label: "5-22",
    },
];

/// Looks up the Forte set-class name for `mask`, normalizing it to prime form
/// first; returns `None` if the set class is not in [`FORTE_TABLE`].
///
/// # Examples
///
/// ```
/// use sim_lib_pitch_set::PitchClassMask;
/// use sim_lib_pitch_namer_forte::lookup_forte_label;
///
/// assert_eq!(lookup_forte_label(PitchClassMask(0b0001_0001_0101)), Some("4-27"));
/// assert_eq!(lookup_forte_label(PitchClassMask(0)), None);
/// ```
pub fn lookup_forte_label(mask: PitchClassMask) -> Option<&'static str> {
    let normalized = mask.normalize();
    FORTE_TABLE
        .iter()
        .find(|entry| entry.mask.normalize() == normalized)
        .map(|entry| entry.label)
}
