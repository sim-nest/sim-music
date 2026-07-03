use crate::{
    INSTRUMENT_EDITOR_FIXTURE_NAMES, INSTRUMENT_EDITOR_ROUTE_NAMES, INSTRUMENT_EDITOR_VIEW_IDS,
    instrument_editor_descriptors, instrument_editor_fixture_names, instrument_editor_route_names,
    instrument_editor_view_ids,
};

#[test]
fn instrument_editor_descriptors_record_routes_views_and_fixtures() {
    assert_eq!(
        instrument_editor_route_names(),
        INSTRUMENT_EDITOR_ROUTE_NAMES.as_slice()
    );
    assert_eq!(
        instrument_editor_view_ids(),
        INSTRUMENT_EDITOR_VIEW_IDS.as_slice()
    );
    assert_eq!(
        instrument_editor_fixture_names(),
        INSTRUMENT_EDITOR_FIXTURE_NAMES.as_slice()
    );

    for descriptor in instrument_editor_descriptors() {
        assert!(
            instrument_editor_route_names().contains(&descriptor.route_name),
            "missing route {}",
            descriptor.route_name
        );
        assert!(
            instrument_editor_view_ids().contains(&descriptor.view_id),
            "missing view {}",
            descriptor.view_id
        );
        assert_eq!(
            descriptor.fixture_names.len(),
            4,
            "{} has default, empty, invalid, and representative fixtures",
            descriptor.instrument
        );
        for fixture in descriptor.fixture_names {
            assert!(
                instrument_editor_fixture_names().contains(fixture),
                "missing fixture {fixture}"
            );
        }
    }
}
