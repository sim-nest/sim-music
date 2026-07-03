use sim_kernel::Symbol;

use crate::{ComponentPortDescriptor, ComponentPortDirection, ComponentPortMedia};

pub(super) fn input(channels: &[&[f32]], channel: usize, frame: usize) -> f32 {
    channels
        .get(channel)
        .and_then(|samples| samples.get(frame))
        .copied()
        .unwrap_or(0.0)
}

pub(super) fn write_outputs(outputs: &mut [&mut [f32]], frame: usize, samples: &[f32]) {
    for (channel, output) in outputs.iter_mut().enumerate() {
        output[frame] = samples
            .get(channel)
            .copied()
            .or_else(|| samples.last().copied())
            .unwrap_or(0.0);
    }
}

pub(super) fn input_port(name: &'static str, media: ComponentPortMedia) -> ComponentPortDescriptor {
    ComponentPortDescriptor::new(port_key(name), media, ComponentPortDirection::Input, 1)
}

pub(super) fn output_port(
    name: &'static str,
    media: ComponentPortMedia,
) -> ComponentPortDescriptor {
    ComponentPortDescriptor::new(port_key(name), media, ComponentPortDirection::Output, 1)
}

pub(super) fn port_key(name: &'static str) -> Symbol {
    Symbol::qualified("audio-synth/r700-port", name)
}

pub(super) fn param_key(name: &'static str) -> Symbol {
    Symbol::qualified("audio-synth/r700-param", name)
}

pub(super) fn inspect_key(name: &'static str) -> Symbol {
    Symbol::qualified("audio-synth/r700-inspect", name)
}

pub(super) fn trace_key(name: &'static str) -> Symbol {
    Symbol::qualified("audio-synth/r700-trace", name)
}
