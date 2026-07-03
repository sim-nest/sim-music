use std::collections::BTreeMap;

use sim_kernel::{Error, Expr, Result, Symbol};

use crate::{
    MusicCapability, MusicComponentDescriptor, arpeggio_lab_player_descriptor,
    automation_curve_modulator_descriptor, bassline_player_descriptor, beat_map_player_descriptor,
    chord_sequencer_player_descriptor, default_instrument_descriptor,
    drum_key_map_player_descriptor, dual_arpeggio_player_descriptor, envelope_modulator_descriptor,
    euclid_player_descriptor, keyboard_performance_source_descriptor, lfo_modulator_descriptor,
    note_echo_player_descriptor, oscillator_modulator_descriptor,
    pattern_mutator_player_descriptor, polystep_player_descriptor, quad_note_player_descriptor,
    random_walk_modulator_descriptor, scales_chords_player_descriptor, tempo_lfo_descriptor,
};

/// A single registered music component, wrapping its descriptor.
///
/// Each entry holds one [`MusicComponentDescriptor`] and exposes the lookup
/// keys and capability checks the registry needs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MusicComponentRegistryEntry {
    descriptor: MusicComponentDescriptor,
}

impl MusicComponentRegistryEntry {
    /// Wraps a descriptor in a registry entry.
    pub fn new(descriptor: MusicComponentDescriptor) -> Self {
        Self { descriptor }
    }

    /// Returns the component's unique identifier symbol.
    pub fn id(&self) -> &Symbol {
        &self.descriptor.id
    }

    /// Returns the wrapped component descriptor.
    pub fn descriptor(&self) -> &MusicComponentDescriptor {
        &self.descriptor
    }

    /// Reports whether the component declares the given capability.
    pub fn has_capability(&self, capability: MusicCapability) -> bool {
        self.descriptor.has_capability(capability)
    }

    /// Encodes the entry's descriptor as an [`Expr`].
    pub fn to_expr(&self) -> Expr {
        self.descriptor.to_expr()
    }
}

/// An ordered collection of music components keyed by qualified id.
///
/// Entries are stored in a [`BTreeMap`] so iteration order is deterministic.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MusicComponentRegistry {
    entries: BTreeMap<String, MusicComponentRegistryEntry>,
}

impl MusicComponentRegistry {
    /// Creates an empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers an entry, failing if its id is already present.
    pub fn register(&mut self, entry: MusicComponentRegistryEntry) -> Result<()> {
        let key = entry.id().as_qualified_str();
        if self.entries.contains_key(&key) {
            return Err(Error::Eval(format!(
                "duplicate music component registry id: {key}"
            )));
        }
        self.entries.insert(key, entry);
        Ok(())
    }

    /// Looks up an entry by its qualified id, if registered.
    pub fn get(&self, id: &Symbol) -> Option<&MusicComponentRegistryEntry> {
        self.entries.get(&id.as_qualified_str())
    }

    /// Iterates over all registered entries in id order.
    pub fn entries(&self) -> impl Iterator<Item = &MusicComponentRegistryEntry> {
        self.entries.values()
    }

    /// Collects every entry that declares the given capability.
    pub fn by_capability(&self, capability: MusicCapability) -> Vec<&MusicComponentRegistryEntry> {
        self.entries
            .values()
            .filter(|entry| entry.has_capability(capability))
            .collect()
    }

    /// Looks up an entry and requires it to declare the given capability.
    ///
    /// Returns an error if the id is not registered or the component lacks the
    /// capability.
    pub fn require_capability(
        &self,
        id: &Symbol,
        capability: MusicCapability,
    ) -> Result<&MusicComponentRegistryEntry> {
        let entry = self
            .get(id)
            .ok_or_else(|| Error::Eval(format!("music component not registered: {id}")))?;
        if !entry.has_capability(capability) {
            return Err(Error::Eval(format!(
                "music component {id} missing capability {}",
                capability.wire_label()
            )));
        }
        Ok(entry)
    }

    /// Encodes the full registry as a tagged inventory [`Expr`] map.
    pub fn inventory_expr(&self) -> Expr {
        Expr::Map(vec![
            (
                field("tag"),
                Expr::Symbol(Symbol::qualified("music", "component-registry")),
            ),
            (
                field("entries"),
                Expr::Vector(
                    self.entries()
                        .map(MusicComponentRegistryEntry::to_expr)
                        .collect(),
                ),
            ),
        ])
    }
}

/// Builds a registry populated with every built-in music component descriptor.
pub fn default_music_component_registry() -> MusicComponentRegistry {
    let mut registry = MusicComponentRegistry::new();
    for descriptor in [
        scales_chords_player_descriptor(),
        dual_arpeggio_player_descriptor(),
        arpeggio_lab_player_descriptor(),
        note_echo_player_descriptor(),
        beat_map_player_descriptor(),
        euclid_player_descriptor(),
        drum_key_map_player_descriptor(),
        chord_sequencer_player_descriptor(),
        bassline_player_descriptor(),
        polystep_player_descriptor(),
        quad_note_player_descriptor(),
        pattern_mutator_player_descriptor(),
        default_instrument_descriptor(),
        keyboard_performance_source_descriptor(),
        tempo_lfo_descriptor(),
        lfo_modulator_descriptor(),
        envelope_modulator_descriptor(),
        oscillator_modulator_descriptor(),
        random_walk_modulator_descriptor(),
        automation_curve_modulator_descriptor(),
    ] {
        registry
            .register(MusicComponentRegistryEntry::new(
                descriptor.expect("default music component descriptors are valid"),
            ))
            .expect("default music component registry ids are unique");
    }
    registry
}

/// Returns the registry id of the dual-arpeggio player.
pub fn dual_arpeggio_player_id() -> Symbol {
    Symbol::qualified("music/player-family", "dual-arpeggio")
}

/// Returns the registry id of the arpeggio-lab player.
pub fn arpeggio_lab_player_id() -> Symbol {
    Symbol::qualified("music/player-family", "arpeggio-lab")
}

/// Returns the registry id of the scales-chords player.
pub fn scales_chords_player_id() -> Symbol {
    Symbol::qualified("music/player-family", "scales-chords")
}

/// Returns the registry id of the note-echo player.
pub fn note_echo_player_id() -> Symbol {
    Symbol::qualified("music/player-family", "note-echo")
}

/// Returns the registry id of the beat-map player.
pub fn beat_map_player_id() -> Symbol {
    Symbol::qualified("music/player-family", "beat-map")
}

/// Returns the registry id of the euclid player.
pub fn euclid_player_id() -> Symbol {
    Symbol::qualified("music/player-family", "euclid")
}

/// Returns the registry id of the drum-key-map player.
pub fn drum_key_map_player_id() -> Symbol {
    Symbol::qualified("music/player-family", "drum-key-map")
}

/// Returns the registry id of the chord-sequencer player.
pub fn chord_sequencer_player_id() -> Symbol {
    Symbol::qualified("music/player-family", "chord-sequencer")
}

/// Returns the registry id of the bassline-generator player.
pub fn bassline_player_id() -> Symbol {
    Symbol::qualified("music/player-family", "bassline-generator")
}

/// Returns the registry id of the polystep player.
pub fn polystep_player_id() -> Symbol {
    Symbol::qualified("music/player-family", "polystep")
}

/// Returns the registry id of the quad-note-generator player.
pub fn quad_note_player_id() -> Symbol {
    Symbol::qualified("music/player-family", "quad-note-generator")
}

/// Returns the registry id of the pattern-mutator player.
pub fn pattern_mutator_player_id() -> Symbol {
    Symbol::qualified("music/player-family", "pattern-mutator")
}

/// Returns the registry id of the LFO modulator.
pub fn lfo_modulator_id() -> Symbol {
    Symbol::qualified("music/modulator", "lfo")
}

/// Returns the registry id of the envelope modulator.
pub fn envelope_modulator_id() -> Symbol {
    Symbol::qualified("music/modulator", "envelope")
}

/// Returns the registry id of the oscillator modulator.
pub fn oscillator_modulator_id() -> Symbol {
    Symbol::qualified("music/modulator", "oscillator")
}

/// Returns the registry id of the random-walk modulator.
pub fn random_walk_modulator_id() -> Symbol {
    Symbol::qualified("music/modulator", "random-walk")
}

/// Returns the registry id of the automation-curve modulator.
pub fn automation_curve_modulator_id() -> Symbol {
    Symbol::qualified("music/modulator", "automation-curve")
}

fn field(name: &'static str) -> Expr {
    sim_value::build::qsym("music/component-registry", name)
}
