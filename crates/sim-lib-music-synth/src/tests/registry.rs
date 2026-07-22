use sim_kernel::{Expr, Symbol};

use crate::{
    ComponentCapability, ComponentRegistry, ComponentRegistryCategory, InstrumentWrapperCategory,
    component_graph_registry_entry, default_audio_synth_registry, dx7_component_id,
    ps_3300_component_id, subtractive_synth_component_id, system_55_component_id,
    system_700_component_id,
};

#[test]
fn registry_lookup_covers_exact_and_compatible_entries() {
    let registry = default_audio_synth_registry();
    let synth = registry.get(&subtractive_synth_component_id()).unwrap();
    assert_eq!(synth.category(), ComponentRegistryCategory::Exact);
    assert_eq!(synth.wrapper(), InstrumentWrapperCategory::FixedPolysynth);
    assert!(synth.is_implemented());
    assert!(synth.instantiate().is_ok());

    let dx7 = registry.get(&dx7_component_id()).expect("dx7 entry");
    assert_eq!(dx7.category(), ComponentRegistryCategory::Exact);
    assert_eq!(dx7.wrapper(), InstrumentWrapperCategory::Dx7);
    assert!(dx7.is_implemented());
    assert!(dx7.instantiate().is_ok());

    let system_700 = registry
        .get(&system_700_component_id())
        .expect("System 700 entry");
    assert_eq!(system_700.category(), ComponentRegistryCategory::Exact);
    assert_eq!(
        system_700.wrapper(),
        InstrumentWrapperCategory::ModularAnalog
    );
    assert!(system_700.is_implemented());
    assert!(system_700.instantiate().is_ok());

    let system_55 = registry
        .get(&system_55_component_id())
        .expect("System 55 entry");
    assert_eq!(system_55.category(), ComponentRegistryCategory::Exact);
    assert_eq!(
        system_55.wrapper(),
        InstrumentWrapperCategory::ModularAnalog
    );
    assert!(system_55.is_implemented());
    assert!(system_55.instantiate().is_ok());

    let ps_3300 = registry
        .get(&ps_3300_component_id())
        .expect("PS-3300 entry");
    assert_eq!(ps_3300.category(), ComponentRegistryCategory::Exact);
    assert_eq!(ps_3300.wrapper(), InstrumentWrapperCategory::FixedPolysynth);
    assert!(ps_3300.is_implemented());
    assert!(ps_3300.instantiate().is_ok());
}

#[test]
fn registry_rejects_duplicate_ids() {
    let mut registry = ComponentRegistry::new();
    registry
        .register(component_graph_registry_entry())
        .expect("first registration");
    let err = registry
        .register(component_graph_registry_entry())
        .expect_err("duplicate should fail");

    assert!(format!("{err}").contains("duplicate audio synth component registry id"));
}

#[test]
fn registry_filters_by_capability_and_category() {
    let registry = default_audio_synth_registry();
    let realtime = registry.by_capability(ComponentCapability::RealtimeSafe);
    assert!(
        realtime
            .iter()
            .any(|entry| entry.id() == &subtractive_synth_component_id())
    );
    assert!(
        realtime
            .iter()
            .any(|entry| entry.id().as_qualified_str() == "audio-synth/DiscreteComponentGraph")
    );
    assert!(realtime.iter().all(|entry| entry.is_implemented()));

    let compatible = registry.by_category(ComponentRegistryCategory::Compatible);
    assert!(compatible.is_empty());
}

#[test]
fn inventory_serializes_for_recipe_and_view_consumers() {
    let registry = default_audio_synth_registry();
    let inventory = registry.inventory();
    let system_700 = inventory
        .items()
        .iter()
        .find(|item| item.id == system_700_component_id())
        .expect("system 700 inventory");

    assert_eq!(system_700.label, "Roland System 700");
    assert_eq!(system_700.wrapper, InstrumentWrapperCategory::ModularAnalog);
    assert!(system_700.implemented);

    let expr = inventory.to_expr();
    let Expr::Map(entries) = expr else {
        panic!("inventory should serialize as a map");
    };
    assert!(entries.iter().any(|(key, value)| key == &field("tag")
        && value == &Expr::Symbol(Symbol::qualified("audio-synth", "component-inventory"))));
    let items = entries
        .iter()
        .find_map(|(key, value)| (key == &field("items")).then_some(value))
        .expect("items field");
    let Expr::Vector(items) = items else {
        panic!("items should serialize as a vector");
    };
    assert_eq!(items.len(), inventory.items().len());
}

#[test]
fn palette_expr_filters_by_category_and_capability_for_builder_views() {
    let registry = default_audio_synth_registry();
    let palette = registry.component_palette_expr(
        Some(ComponentRegistryCategory::Exact),
        Some(ComponentCapability::Editable),
    );

    assert_eq!(
        builder_field_value(&palette, "patch-format"),
        Some(&Expr::String("component-builder-patch-v1".to_owned()))
    );
    assert_eq!(
        builder_field_value(&palette, "category-filter"),
        Some(&Expr::Symbol(ComponentRegistryCategory::Exact.symbol()))
    );
    assert_eq!(
        builder_field_value(&palette, "capability-filter"),
        Some(&Expr::Symbol(ComponentCapability::Editable.symbol()))
    );
    let items = builder_vector_field(&palette, "items").expect("palette items");
    assert!(!items.is_empty());
    assert!(items.iter().all(|item| {
        builder_field_value(item, "category")
            == Some(&Expr::Symbol(ComponentRegistryCategory::Exact.symbol()))
    }));
    assert!(
        items
            .iter()
            .all(|item| has_builder_capability(item, ComponentCapability::Editable))
    );
    assert!(
        items
            .iter()
            .any(|item| builder_field_value(item, "descriptor").is_some())
    );

    let specialized =
        registry.component_palette_expr(None, Some(ComponentCapability::SpecializedView));
    let specialized_items = builder_vector_field(&specialized, "items").expect("specialized items");
    assert!(
        specialized_items
            .iter()
            .any(|item| builder_field_value(item, "id") == Some(&Expr::Symbol(dx7_component_id())))
    );
    assert!(
        specialized_items
            .iter()
            .all(|item| has_builder_capability(item, ComponentCapability::SpecializedView))
    );
    assert!(
        specialized_items
            .iter()
            .all(|item| builder_field_value(item, "id")
                != Some(&Expr::Symbol(subtractive_synth_component_id())))
    );
}

#[test]
fn registry_entry_serializes_generic_component_editor_descriptor() {
    let registry = default_audio_synth_registry();
    let dx7 = registry.get(&dx7_component_id()).expect("dx7 entry");
    let expr = dx7.to_editor_descriptor_expr();
    let Expr::Map(entries) = &expr else {
        panic!("descriptor should serialize as a map");
    };
    assert!(entries.iter().any(|(key, value)| key == &plain_key("tag")
        && value
            == &Expr::Symbol(Symbol::qualified(
                "audio-synth",
                "component-editor-descriptor"
            ))));
    assert!(
        entries
            .iter()
            .any(|(key, value)| key == &plain_key("trace-available") && value == &Expr::Bool(true))
    );
    assert!(entries.iter().any(|(key, value)| key == &plain_key("specialized-view")
        && matches!(value, Expr::Symbol(symbol) if symbol.as_qualified_str() == "view/component/dx7")));

    let groups = plain_key_value(&expr, "parameter-groups").expect("parameter groups");
    let Expr::Vector(groups) = groups else {
        panic!("parameter groups should serialize as a vector");
    };
    assert!(!groups.is_empty());

    let ports = plain_key_value(&expr, "ports").expect("ports");
    let Expr::Vector(ports) = ports else {
        panic!("ports should serialize as a vector");
    };
    assert!(!ports.is_empty());

    let current = plain_key_value(&expr, "current-values").expect("current values");
    assert!(matches!(current, Expr::Map(entries) if !entries.is_empty()));
}

fn field(name: &'static str) -> Expr {
    sim_value::build::qsym("audio-synth/inventory", name)
}

fn plain_key(name: &'static str) -> Expr {
    sim_value::build::sym(name)
}

fn plain_key_value<'a>(expr: &'a Expr, name: &'static str) -> Option<&'a Expr> {
    let Expr::Map(entries) = expr else {
        return None;
    };
    entries
        .iter()
        .find_map(|(key, value)| (key == &plain_key(name)).then_some(value))
}

fn builder_field(name: &'static str) -> Expr {
    sim_value::build::qsym("audio-synth/component-builder", name)
}

fn builder_field_value<'a>(expr: &'a Expr, name: &'static str) -> Option<&'a Expr> {
    let Expr::Map(entries) = expr else {
        return None;
    };
    entries
        .iter()
        .find_map(|(key, value)| (key == &builder_field(name)).then_some(value))
}

fn builder_vector_field<'a>(expr: &'a Expr, name: &'static str) -> Option<&'a [Expr]> {
    let Expr::Vector(items) = builder_field_value(expr, name)? else {
        return None;
    };
    Some(items)
}

fn has_builder_capability(item: &Expr, capability: ComponentCapability) -> bool {
    matches!(
        builder_field_value(item, "capabilities"),
        Some(Expr::Vector(capabilities))
            if capabilities
                .iter()
                .any(|value| value == &Expr::Symbol(capability.symbol()))
    )
}
