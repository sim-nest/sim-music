//! Instrument editor descriptors shared with WebUI lenses.

/// DX7 editor route name.
pub const DX7_EDITOR_ROUTE_NAME: &str = "instrument-editor/dx7";
/// DX7 editor view id.
pub const DX7_EDITOR_VIEW_ID: &str = "view:instrument-dx7-editor";
/// DX7 editor fixture names.
pub const DX7_EDITOR_FIXTURE_NAMES: [&str; 4] = [
    "dx7-default-patch",
    "dx7-empty-patch",
    "dx7-invalid-patch",
    "dx7-all-algorithms",
];

/// Roland System 700 editor route name.
pub const SYSTEM700_EDITOR_ROUTE_NAME: &str = "instrument-editor/system700";
/// Roland System 700 editor view id.
pub const SYSTEM700_EDITOR_VIEW_ID: &str = "view:instrument-system700-editor";
/// Roland System 700 editor fixture names.
pub const SYSTEM700_EDITOR_FIXTURE_NAMES: [&str; 4] = [
    "system700-default-patch",
    "system700-empty-patch",
    "system700-invalid-patch",
    "system700-sequencer-patch",
];

/// Moog System 55 editor route name.
pub const SYSTEM55_EDITOR_ROUTE_NAME: &str = "instrument-editor/system55";
/// Moog System 55 editor view id.
pub const SYSTEM55_EDITOR_VIEW_ID: &str = "view:instrument-system55-editor";
/// Moog System 55 editor fixture names.
pub const SYSTEM55_EDITOR_FIXTURE_NAMES: [&str; 4] = [
    "system55-default-patch",
    "system55-empty-patch",
    "system55-invalid-patch",
    "system55-filter-bank-patch",
];

/// Korg PS-3300 editor route name.
pub const PS3300_EDITOR_ROUTE_NAME: &str = "instrument-editor/ps3300";
/// Korg PS-3300 editor view id.
pub const PS3300_EDITOR_VIEW_ID: &str = "view:instrument-ps3300-editor";
/// Korg PS-3300 editor fixture names.
pub const PS3300_EDITOR_FIXTURE_NAMES: [&str; 4] = [
    "ps3300-default-patch",
    "ps3300-empty-patch",
    "ps3300-invalid-patch",
    "ps3300-three-section-patch",
];

/// Route names for all instrument editors.
pub const INSTRUMENT_EDITOR_ROUTE_NAMES: [&str; 4] = [
    DX7_EDITOR_ROUTE_NAME,
    SYSTEM700_EDITOR_ROUTE_NAME,
    SYSTEM55_EDITOR_ROUTE_NAME,
    PS3300_EDITOR_ROUTE_NAME,
];

/// View ids for all instrument editors.
pub const INSTRUMENT_EDITOR_VIEW_IDS: [&str; 4] = [
    DX7_EDITOR_VIEW_ID,
    SYSTEM700_EDITOR_VIEW_ID,
    SYSTEM55_EDITOR_VIEW_ID,
    PS3300_EDITOR_VIEW_ID,
];

/// Fixture names for all instrument editors.
pub const INSTRUMENT_EDITOR_FIXTURE_NAMES: [&str; 16] = [
    DX7_EDITOR_FIXTURE_NAMES[0],
    DX7_EDITOR_FIXTURE_NAMES[1],
    DX7_EDITOR_FIXTURE_NAMES[2],
    DX7_EDITOR_FIXTURE_NAMES[3],
    SYSTEM700_EDITOR_FIXTURE_NAMES[0],
    SYSTEM700_EDITOR_FIXTURE_NAMES[1],
    SYSTEM700_EDITOR_FIXTURE_NAMES[2],
    SYSTEM700_EDITOR_FIXTURE_NAMES[3],
    SYSTEM55_EDITOR_FIXTURE_NAMES[0],
    SYSTEM55_EDITOR_FIXTURE_NAMES[1],
    SYSTEM55_EDITOR_FIXTURE_NAMES[2],
    SYSTEM55_EDITOR_FIXTURE_NAMES[3],
    PS3300_EDITOR_FIXTURE_NAMES[0],
    PS3300_EDITOR_FIXTURE_NAMES[1],
    PS3300_EDITOR_FIXTURE_NAMES[2],
    PS3300_EDITOR_FIXTURE_NAMES[3],
];

/// An instrument editor declaration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InstrumentEditorDescriptor {
    /// User-facing instrument id.
    pub instrument: &'static str,
    /// Route name used by WebUI navigation.
    pub route_name: &'static str,
    /// Scene view id.
    pub view_id: &'static str,
    /// Snapshot fixture names for this editor.
    pub fixture_names: &'static [&'static str],
}

/// All instrument editor declarations.
pub const INSTRUMENT_EDITOR_DESCRIPTORS: [InstrumentEditorDescriptor; 4] = [
    InstrumentEditorDescriptor {
        instrument: "dx7",
        route_name: DX7_EDITOR_ROUTE_NAME,
        view_id: DX7_EDITOR_VIEW_ID,
        fixture_names: &DX7_EDITOR_FIXTURE_NAMES,
    },
    InstrumentEditorDescriptor {
        instrument: "system700",
        route_name: SYSTEM700_EDITOR_ROUTE_NAME,
        view_id: SYSTEM700_EDITOR_VIEW_ID,
        fixture_names: &SYSTEM700_EDITOR_FIXTURE_NAMES,
    },
    InstrumentEditorDescriptor {
        instrument: "system55",
        route_name: SYSTEM55_EDITOR_ROUTE_NAME,
        view_id: SYSTEM55_EDITOR_VIEW_ID,
        fixture_names: &SYSTEM55_EDITOR_FIXTURE_NAMES,
    },
    InstrumentEditorDescriptor {
        instrument: "ps3300",
        route_name: PS3300_EDITOR_ROUTE_NAME,
        view_id: PS3300_EDITOR_VIEW_ID,
        fixture_names: &PS3300_EDITOR_FIXTURE_NAMES,
    },
];

/// Return all instrument editor declarations.
pub fn instrument_editor_descriptors() -> &'static [InstrumentEditorDescriptor] {
    &INSTRUMENT_EDITOR_DESCRIPTORS
}

/// Return every route name.
pub fn instrument_editor_route_names() -> &'static [&'static str] {
    &INSTRUMENT_EDITOR_ROUTE_NAMES
}

/// Return every view id.
pub fn instrument_editor_view_ids() -> &'static [&'static str] {
    &INSTRUMENT_EDITOR_VIEW_IDS
}

/// Return every fixture name.
pub fn instrument_editor_fixture_names() -> &'static [&'static str] {
    &INSTRUMENT_EDITOR_FIXTURE_NAMES
}
