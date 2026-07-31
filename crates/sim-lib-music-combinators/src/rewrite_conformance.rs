use sim_lib_discrete_search::{SearchControl, SearchStatus};
use sim_lib_music_core::{Pitch, PitchClass};
use sim_lib_music_transform::PitchMapPolicy;
use sim_lib_pitch_scale::Scale;

use crate::{
    Alphabet, ExpansionLimits, LSystem, LSystemError, Production, ProductionContext,
    ProgressionTree, ProgressionTreeCatalog, ProgressionTreeFilter, ProgressionTreeTopology,
    ScaleFollowingState, ScaleRewriteError, ScaleRewriteProgram, SymbolRole, derive,
};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Branch {
    Grow,
    Leaf,
}

fn search_control(seed: u64) -> SearchControl {
    SearchControl::default()
        .with_seed(seed)
        .with_max_work(100_000)
        .with_max_results(128)
        .with_max_frontier(4_096)
        .with_max_memory_nodes(8_192)
}

fn context_system() -> LSystem<Branch> {
    LSystem::new(
        Alphabet::new([
            (Branch::Grow, SymbolRole::Variable),
            (Branch::Leaf, SymbolRole::Constant),
        ])
        .expect("alphabet"),
        vec![Branch::Grow],
        vec![
            Production::new(
                "grow-next-to-leaf",
                Branch::Grow,
                ProductionContext::new(None, Some(Branch::Leaf)),
                vec![Branch::Leaf, Branch::Leaf],
            ),
            Production::new(
                "grow-fallback",
                Branch::Grow,
                ProductionContext::free(),
                vec![Branch::Grow, Branch::Leaf],
            ),
        ],
        ExpansionLimits::new(3, 8, 24),
    )
    .expect("productive finite system")
}

#[test]
fn typed_contextual_derivation_is_bounded_and_inspectable() {
    let run = derive(&context_system(), 2, search_control(17)).expect("bounded derivation");

    assert_eq!(run.receipt.status, SearchStatus::Complete);
    assert_eq!(run.receipt.seed, 17);
    assert_eq!(run.outputs.len(), 1);
    let result = &run.outputs[0];
    assert_eq!(
        result
            .generations
            .iter()
            .map(|generation| generation.symbols.clone())
            .collect::<Vec<_>>(),
        vec![
            vec![Branch::Grow],
            vec![Branch::Grow, Branch::Leaf],
            vec![Branch::Leaf, Branch::Leaf, Branch::Leaf],
        ]
    );
    assert_eq!(result.receipt.total_symbols, 6);
    assert_eq!(result.receipt.production_uses["grow-fallback"], 1);
    assert_eq!(result.receipt.production_uses["grow-next-to-leaf"], 1);
    assert_eq!(
        result.forest[0].production_id.as_deref(),
        Some("grow-fallback")
    );
    assert_eq!(
        result.forest[0].children[0].production_id.as_deref(),
        Some("grow-next-to-leaf")
    );
    assert_eq!(result.forest[0].children[0].children.len(), 2);

    let repeated = derive(&context_system(), 2, search_control(17)).expect("repeat");
    assert_eq!(run, repeated);
}

#[test]
fn invalid_productivity_and_unbounded_policies_fail_before_search() {
    let alphabet = Alphabet::new([(Branch::Grow, SymbolRole::Variable)]).expect("alphabet");
    let no_fallback = LSystem::new(
        alphabet.clone(),
        vec![Branch::Grow],
        vec![Production::new(
            "context-only",
            Branch::Grow,
            ProductionContext::new(Some(Branch::Grow), None),
            vec![Branch::Grow],
        )],
        ExpansionLimits::new(2, 4, 8),
    );
    assert_eq!(no_fallback, Err(LSystemError::VariableWithoutFallback));

    let erasing = LSystem::new(
        alphabet,
        vec![Branch::Grow],
        vec![Production::new(
            "erase",
            Branch::Grow,
            ProductionContext::free(),
            Vec::new(),
        )],
        ExpansionLimits::new(2, 4, 8),
    );
    assert_eq!(
        erasing,
        Err(LSystemError::NonProductiveProduction {
            id: "erase".to_owned()
        })
    );

    assert_eq!(
        derive(&context_system(), 1, SearchControl::default()),
        Err(LSystemError::UnboundedSearch { field: "max_work" })
    );
    let tight = LSystem::new(
        Alphabet::new([
            (Branch::Grow, SymbolRole::Variable),
            (Branch::Leaf, SymbolRole::Constant),
        ])
        .expect("alphabet"),
        vec![Branch::Grow],
        vec![Production::new(
            "double",
            Branch::Grow,
            ProductionContext::free(),
            vec![Branch::Grow, Branch::Grow],
        )],
        ExpansionLimits::new(2, 3, 8),
    )
    .expect("policy is structurally valid");
    assert!(matches!(
        derive(&tight, 2, search_control(0)),
        Err(LSystemError::ExpansionCouldExceed {
            generation: 2,
            symbols: Some(4)
        })
    ));
}

fn tree() -> ProgressionTree<&'static str> {
    ProgressionTree::new(
        "I",
        vec![
            ProgressionTree::new("IV", Vec::new()),
            ProgressionTree::new("V", vec![ProgressionTree::new("I", Vec::new())]),
        ],
    )
}

struct NoRepeatedPreorderValue;

impl ProgressionTreeFilter<&'static str> for NoRepeatedPreorderValue {
    fn allows_prefix(&self, prefix: &[&'static str], _topology: &ProgressionTreeTopology) -> bool {
        prefix.windows(2).all(|pair| pair[0] != pair[1])
    }
}

#[test]
fn progression_trees_rank_round_trip_and_search_through_shared_engines() {
    let topology = ProgressionTreeTopology::new(vec![2, 0, 1, 0]).expect("topology");
    let catalog =
        ProgressionTreeCatalog::new(vec!["I", "IV", "V"], topology).expect("tree catalog");
    let rank = catalog.rank(&tree()).expect("rank");

    assert_eq!(catalog.unrank(&rank).expect("unrank"), tree());
    assert_eq!(catalog.rank(&catalog.unrank(&rank).unwrap()).unwrap(), rank);
    assert!(matches!(
        catalog.explore(SearchControl::default(), &NoRepeatedPreorderValue),
        Err(crate::ProgressionTreeError::UnboundedSearch { field: "max_work" })
    ));

    let control = search_control(23).with_max_results(7);
    let run = catalog
        .explore(control, &NoRepeatedPreorderValue)
        .expect("bounded tree search");
    assert_eq!(run.receipt.status, SearchStatus::Partial);
    assert_eq!(run.receipt.reason.as_deref(), Some("result bound reached"));
    assert_eq!(run.outputs.len(), 7);
    for output in &run.outputs {
        assert_eq!(
            catalog.rank(&output.tree).expect("rank explored tree"),
            output.rank
        );
        let mut values = Vec::new();
        flatten(&output.tree, &mut values);
        assert!(values.windows(2).all(|pair| pair[0] != pair[1]));
    }
}

#[test]
fn scale_modulation_is_rewrite_data_over_pitch_maps() {
    let c = ScaleFollowingState::from_scale(
        "c-major",
        Scale::major(PitchClass::C),
        PitchMapPolicy::Nearest,
    )
    .expect("C state");
    let g = ScaleFollowingState::from_scale(
        "g-major",
        Scale::major(PitchClass::G),
        PitchMapPolicy::Nearest,
    )
    .expect("G state");
    let d = ScaleFollowingState::from_scale(
        "d-major",
        Scale::major(PitchClass::D),
        PitchMapPolicy::Nearest,
    )
    .expect("D state");
    let program = ScaleRewriteProgram::new(
        vec![c, g, d],
        vec!["c-major".to_owned()],
        vec![
            Production::new(
                "c-to-dominant-neighbors",
                "c-major".to_owned(),
                ProductionContext::free(),
                vec!["g-major".to_owned(), "d-major".to_owned()],
            ),
            Production::new(
                "g-to-d",
                "g-major".to_owned(),
                ProductionContext::free(),
                vec!["d-major".to_owned()],
            ),
            Production::new(
                "d-to-c",
                "d-major".to_owned(),
                ProductionContext::free(),
                vec!["c-major".to_owned()],
            ),
        ],
        ExpansionLimits::new(2, 4, 8),
    )
    .expect("scale rewrite data");

    let run = program
        .derive(1, search_control(31))
        .expect("scale rewrite");
    assert_eq!(run.outputs.len(), 1);
    let output = &run.outputs[0];
    assert_eq!(
        output.generations[1]
            .iter()
            .map(|state| state.id.as_str())
            .collect::<Vec<_>>(),
        vec!["g-major", "d-major"]
    );
    for state in &output.generations[1] {
        let mapped = state
            .pitch_map
            .map_pitch(Pitch::from_midi(61))
            .expect("nearest scale pitch");
        assert!(state.scale.degree_of(mapped.pitch.class).is_some());
    }
    assert_eq!(
        output.derivation.receipt.production_uses["c-to-dominant-neighbors"],
        1
    );
    assert_eq!(output.program_digest.len(), 16);

    assert_eq!(
        ScaleFollowingState::from_scale(
            "unsafe",
            Scale::major(PitchClass::C),
            PitchMapPolicy::Unmapped,
        ),
        Err(ScaleRewriteError::NonFollowingMapPolicy)
    );
}

fn flatten<'a>(tree: &'a ProgressionTree<&'static str>, values: &mut Vec<&'a str>) {
    values.push(tree.value);
    for child in &tree.children {
        flatten(child, values);
    }
}
