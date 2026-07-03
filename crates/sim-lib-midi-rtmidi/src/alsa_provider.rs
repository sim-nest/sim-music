use std::sync::Arc;

use sim_kernel::{Error, Result, Symbol};
use sim_lib_stream_host::{
    DeviceDirection, DeviceKind, DeviceProvider, DeviceRecord, HostDirection, Placement,
    StreamEvalSite,
};

use crate::{
    FixtureRtmidiProvider, NativeRtmidiProvider, RtmidiBackend, RtmidiHardwareConfig,
    RtmidiInputSource, RtmidiOutputSink, RtmidiPort, RtmidiProvider, input_ring,
};

macro_rules! native_midi_provider {
    (
        provider: $provider:ident,
        input_site: $input_site:ident,
        output_site: $output_site:ident,
        duplex_site: $duplex_site:ident,
        transport: $transport:literal,
        label: $label:literal,
        provider_doc: $provider_doc:literal,
        input_doc: $input_doc:literal,
        output_doc: $output_doc:literal,
        duplex_doc: $duplex_doc:literal
    ) => {
        #[doc = $provider_doc]
        pub struct $provider {
            backend: RtmidiBackend,
            provider: Arc<dyn RtmidiProvider>,
        }

        impl $provider {
            /// Builds a placement provider from an existing backend snapshot.
            pub fn from_backend(backend: RtmidiBackend) -> Self {
                let provider = FixtureRtmidiProvider::new(backend.list_ports().to_vec())
                    .with_timing(backend.timing());
                Self {
                    backend,
                    provider: Arc::new(provider),
                }
            }

            /// Builds a placement provider from deterministic fixture ports.
            pub fn from_fixture(provider: FixtureRtmidiProvider) -> Result<Self> {
                let backend = provider.enumerate_backend()?;
                Ok(Self {
                    backend,
                    provider: Arc::new(provider),
                })
            }

            fn record(port: &RtmidiPort) -> DeviceRecord {
                DeviceRecord {
                    id: port.id().clone(),
                    display_name: port.name().to_owned(),
                    kind: DeviceKind::Midi,
                    direction: device_direction(port.direction()),
                    placement: Placement::Hardware {
                        transport: Symbol::new($transport),
                    },
                }
            }
        }

        impl DeviceProvider for $provider {
            fn enumerate(&self) -> Result<Vec<DeviceRecord>> {
                Ok(self.backend.list_ports().iter().map(Self::record).collect())
            }

            fn open(&self, id: &Symbol) -> Result<Box<dyn StreamEvalSite>> {
                let port = self
                    .backend
                    .list_ports()
                    .iter()
                    .find(|port| port.id() == id)
                    .ok_or_else(|| Error::Eval(format!("{}: unknown port '{}'", $label, id)))?;
                let record = Self::record(port);
                match port.direction() {
                    HostDirection::Input => {
                        let timing = self.backend.timing();
                        let ring = input_ring(timing, 64)?;
                        let driver = self.provider.open_input(id, ring)?;
                        Ok(Box::new($input_site {
                            record,
                            source: RtmidiInputSource::new(driver, timing),
                        }))
                    }
                    HostDirection::Output => {
                        let timing = self.backend.timing();
                        let driver = self.provider.open_output(id)?;
                        Ok(Box::new($output_site {
                            record,
                            sink: RtmidiOutputSink::new(driver, timing),
                        }))
                    }
                    HostDirection::Duplex => Ok(Box::new($duplex_site { record })),
                }
            }
        }

        #[doc = $input_doc]
        pub struct $input_site {
            record: DeviceRecord,
            source: RtmidiInputSource,
        }

        #[doc = $output_doc]
        pub struct $output_site {
            record: DeviceRecord,
            sink: RtmidiOutputSink,
        }

        #[doc = $duplex_doc]
        pub struct $duplex_site {
            record: DeviceRecord,
        }

        impl StreamEvalSite for $input_site {
            fn placement(&self) -> &Placement {
                &self.record.placement
            }

            fn device_record(&self) -> &DeviceRecord {
                &self.record
            }

            fn close(self: Box<Self>) -> Result<()> {
                let _ = self.source;
                Ok(())
            }
        }

        impl StreamEvalSite for $output_site {
            fn placement(&self) -> &Placement {
                &self.record.placement
            }

            fn device_record(&self) -> &DeviceRecord {
                &self.record
            }

            fn close(mut self: Box<Self>) -> Result<()> {
                use sim_lib_midi_core::MidiSink;

                self.sink.flush()
            }
        }

        impl StreamEvalSite for $duplex_site {
            fn placement(&self) -> &Placement {
                &self.record.placement
            }

            fn device_record(&self) -> &DeviceRecord {
                &self.record
            }

            fn close(self: Box<Self>) -> Result<()> {
                Ok(())
            }
        }
    };
}

native_midi_provider! {
    provider: AlsaMidiProvider,
    input_site: AlsaMidiInputEvalSite,
    output_site: AlsaMidiOutputEvalSite,
    duplex_site: AlsaMidiDuplexEvalSite,
    transport: "alsa-seq",
    label: "AlsaMidiProvider",
    provider_doc: "ALSA-sequencer placement provider backed by RtMidi-compatible port metadata.",
    input_doc: "Input evaluation site for an ALSA MIDI port.",
    output_doc: "Output evaluation site for an ALSA MIDI port.",
    duplex_doc: "Duplex evaluation site shell for an ALSA MIDI port."
}

native_midi_provider! {
    provider: CoreMidiProvider,
    input_site: CoreMidiInputEvalSite,
    output_site: CoreMidiOutputEvalSite,
    duplex_site: CoreMidiDuplexEvalSite,
    transport: "coremidi",
    label: "CoreMidiProvider",
    provider_doc: "CoreMIDI placement provider backed by RtMidi-compatible port metadata.",
    input_doc: "Input evaluation site for a CoreMIDI port.",
    output_doc: "Output evaluation site for a CoreMIDI port.",
    duplex_doc: "Duplex evaluation site shell for a CoreMIDI port."
}

native_midi_provider! {
    provider: WinMmProvider,
    input_site: WinMmInputEvalSite,
    output_site: WinMmOutputEvalSite,
    duplex_site: WinMmDuplexEvalSite,
    transport: "winmm",
    label: "WinMmProvider",
    provider_doc: "Windows multimedia MIDI placement provider backed by RtMidi-compatible port metadata.",
    input_doc: "Input evaluation site for a Windows multimedia MIDI port.",
    output_doc: "Output evaluation site for a Windows multimedia MIDI port.",
    duplex_doc: "Duplex evaluation site shell for a Windows multimedia MIDI port."
}

impl AlsaMidiProvider {
    /// Creates an ALSA sequencer placement provider on Linux.
    #[cfg(target_os = "linux")]
    pub fn alsa_seq(config: RtmidiHardwareConfig) -> Result<Self> {
        let provider = NativeRtmidiProvider::alsa_seq(config);
        let backend = provider.enumerate_backend()?;
        Ok(Self {
            backend,
            provider: Arc::new(provider),
        })
    }
}

impl CoreMidiProvider {
    /// Creates a CoreMIDI placement provider on macOS.
    #[cfg(target_os = "macos")]
    pub fn coremidi(config: RtmidiHardwareConfig) -> Result<Self> {
        let provider = NativeRtmidiProvider::coremidi(config);
        let backend = provider.enumerate_backend()?;
        Ok(Self {
            backend,
            provider: Arc::new(provider),
        })
    }
}

impl WinMmProvider {
    /// Creates a Windows multimedia MIDI placement provider on Windows.
    #[cfg(target_os = "windows")]
    pub fn winmm(config: RtmidiHardwareConfig) -> Result<Self> {
        let provider = NativeRtmidiProvider::winmm(config);
        let backend = provider.enumerate_backend()?;
        Ok(Self {
            backend,
            provider: Arc::new(provider),
        })
    }
}

fn device_direction(direction: HostDirection) -> DeviceDirection {
    match direction {
        HostDirection::Input => DeviceDirection::Input,
        HostDirection::Output => DeviceDirection::Output,
        HostDirection::Duplex => DeviceDirection::Duplex,
    }
}
