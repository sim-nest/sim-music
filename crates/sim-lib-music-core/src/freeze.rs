use sim_kernel::Symbol;

use crate::{ChainPlacementPlan, ChainTraceRecord, PlayContext, PlayEvent};

/// Reproducibility metadata for a frozen player chain.
///
/// Records the identity and hashes that pin a render to its inputs, so a freeze
/// can be matched against the chain, context, and output it was produced from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FreezeMeta {
    /// Identifier of the source the chain rendered.
    pub source_id: Symbol,
    /// Stable hash of the rendering chain.
    pub chain_hash: String,
    /// Stable hash of the play context.
    pub context_hash: String,
    /// Random seed used for the render.
    pub seed: u64,
    /// Placement plan the chain resolved to.
    pub placement: ChainPlacementPlan,
    /// Stable hash of the rendered events and traces.
    pub output_hash: String,
}

/// Identifying record for a recorded source, without its rendered output.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceRecording {
    /// Identifier of the recorded source.
    pub source_id: Symbol,
    /// Stable hash of the rendering chain.
    pub chain_hash: String,
    /// Stable hash of the play context.
    pub context_hash: String,
    /// Random seed used for the render.
    pub seed: u64,
    /// Placement plan the chain resolved to.
    pub placement: ChainPlacementPlan,
}

/// A directly captured render: metadata plus its events and traces.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DirectRecording {
    /// Reproducibility metadata for the render.
    pub meta: FreezeMeta,
    /// Rendered play events.
    pub events: Vec<PlayEvent>,
    /// Chain trace records produced during the render.
    pub traces: Vec<ChainTraceRecord>,
}

/// A frozen player chain: cached render output keyed by its [`FreezeMeta`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FrozenPlayerChain {
    /// Reproducibility metadata for the frozen render.
    pub meta: FreezeMeta,
    /// Frozen play events.
    pub events: Vec<PlayEvent>,
    /// Chain trace records captured with the freeze.
    pub traces: Vec<ChainTraceRecord>,
}

impl SourceRecording {
    pub(crate) fn new(
        source_id: Symbol,
        chain_hash: String,
        context_hash: String,
        seed: u64,
        placement: ChainPlacementPlan,
    ) -> Self {
        Self {
            source_id,
            chain_hash,
            context_hash,
            seed,
            placement,
        }
    }
}

pub(crate) fn freeze_meta(
    source_id: Symbol,
    chain_hash: String,
    cx: &PlayContext,
    placement: ChainPlacementPlan,
    events: &[PlayEvent],
    traces: &[ChainTraceRecord],
) -> FreezeMeta {
    FreezeMeta {
        source_id,
        chain_hash,
        context_hash: context_hash(cx),
        seed: cx.seed,
        placement,
        output_hash: stable_hash("player-output", &(events, traces)),
    }
}

pub(crate) fn context_hash(cx: &PlayContext) -> String {
    stable_hash(
        "play-context",
        &(
            &cx.transport,
            &cx.tempo,
            cx.sample_rate,
            cx.ppq,
            cx.range,
            cx.seed,
            &cx.capabilities,
            cx.site,
        ),
    )
}

pub(crate) fn stable_hash<T: core::fmt::Debug>(label: &str, value: &T) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in format!("{label}:{value:?}").bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("fnv1a64:{hash:016x}")
}
