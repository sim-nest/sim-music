use crate::{ArpLab, ArpLabConfig, DualArpMode, DualArpeggiator};

/// Constructs a [`DualArpeggiator`] from two engine configs and a dual mode.
pub fn player_arp_dual(
    engine_a: crate::ArpEngineConfig,
    engine_b: crate::ArpEngineConfig,
    mode: DualArpMode,
) -> DualArpeggiator {
    DualArpeggiator::new(engine_a, engine_b, mode)
}

/// Constructs an [`ArpLab`] anchor/movement arpeggiator from its config.
pub fn player_arp_lab(config: ArpLabConfig) -> ArpLab {
    ArpLab::new(config)
}

/// Constructs a [`crate::BeatMapPlayer`] drum-pattern player from its config.
pub fn player_beat_map(config: crate::BeatMapConfig) -> crate::BeatMapPlayer {
    crate::BeatMapPlayer::new(config)
}

/// Constructs a [`crate::BasslinePlayer`] from its config.
pub fn player_bassline(config: crate::BasslineConfig) -> crate::BasslinePlayer {
    crate::BasslinePlayer::new(config)
}

/// Constructs an [`crate::EuclideanPlayer`] from its config.
pub fn player_euclid(config: crate::EuclidConfig) -> crate::EuclideanPlayer {
    crate::EuclideanPlayer::new(config)
}

/// Constructs a [`crate::PolyStepPlayer`] from its config.
pub fn player_polystep(config: crate::PolyStepConfig) -> crate::PolyStepPlayer {
    crate::PolyStepPlayer::new(config)
}

/// Constructs a [`crate::QuadNotePlayer`] from its config.
pub fn player_quad_note(config: crate::QuadNoteConfig) -> crate::QuadNotePlayer {
    crate::QuadNotePlayer::new(config)
}
