use sim_lib_serial_core::{AggregateRule, AlphabetId, FiniteAlphabet, Series, SeriesTransform};

pub fn certified_transform_scenario() -> Result<(), Box<dyn std::error::Error>> {
    let alphabet = FiniteAlphabet::try_new(
        AlphabetId::try_new("gesture/five-v1")?,
        vec!["rise", "fall", "hold", "turn", "rest"],
    )?;
    let source = Series::try_new(
        alphabet,
        AggregateRule::exhaustive_exactly_once(),
        vec!["turn", "rise", "rest", "fall", "hold"],
    )?;

    let operation = SeriesTransform::retrograde(source.order().len())
        .compose(&SeriesTransform::rotation(source.order().len(), 2))?;
    let transformed = source.apply(&operation)?;
    let restored = transformed.series.apply(
        transformed
            .certificate
            .inverse
            .as_ref()
            .expect("the composed bijections have an inverse"),
    )?;

    assert_eq!(restored.series, source);
    assert!(transformed.certificate.aggregate_preserved);
    assert_eq!(
        transformed.certificate.order_map.output_to_input(),
        [2, 1, 0, 4, 3]
    );
    Ok(())
}
