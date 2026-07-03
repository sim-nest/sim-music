use sim_lib_music_core::{Music, MusicObject};

use crate::PatternMutatorConfig;

/// Player that applies a [`PatternMutatorConfig`] to incoming material.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PatternMutatorPlayer {
    /// Mutator configuration this player applies.
    pub config: PatternMutatorConfig,
}

impl PatternMutatorPlayer {
    /// Wraps a mutator configuration in a player.
    pub fn new(config: PatternMutatorConfig) -> Self {
        Self { config }
    }

    /// Applies the configured mutation to the input and returns the result.
    pub fn play(&self, input: &dyn MusicObject) -> Music {
        self.config.apply(input)
    }

    /// Serializes the underlying configuration to its wire string.
    pub fn to_wire(&self) -> String {
        self.config.to_wire()
    }
}

/// Builds a [`PatternMutatorPlayer`] from a mutator configuration.
pub fn player_mutator(config: PatternMutatorConfig) -> PatternMutatorPlayer {
    PatternMutatorPlayer::new(config)
}
