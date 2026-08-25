//! Provider-neutral plan and evidence for a resilient media-edge music session.

use sim_kernel::Symbol;

/// Stable role occupied by one route endpoint.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MusicRouteRole {
    /// Performance control source such as MIDI.
    Control,
    /// In-process SIM synthesis.
    Synthesis,
    /// Audio playback destination.
    Playback,
    /// Substitute host used when the preferred route fails.
    Fallback,
    /// Deterministic non-realtime rendering sink.
    OfflineRender,
}

/// Health evidence retained independently for every route.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RouteEvidence {
    /// Timing observed for a healthy route.
    Healthy { latency_us: u64, jitter_us: u64 },
    /// Frames were lost without erasing route identity.
    Dropout { lost_frames: u64 },
    /// A disconnected route returned.
    Reconnected { attempts: u32, downtime_ms: u64 },
    /// Discovery data exceeded its freshness bound.
    StaleObservation { age_ms: u64 },
    /// The provider cannot implement this route.
    Unsupported { reason: String },
    /// An otherwise supported route is currently absent.
    Disconnected { reason: String },
}

/// One named endpoint in a vertical plan.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MusicRouteEndpoint {
    /// Stable route identity.
    pub id: Symbol,
    /// Continuity role filled by the endpoint.
    pub role: MusicRouteRole,
    /// Whether the vertical remains useful without it.
    pub optional: bool,
}

/// Stable-identity session plan spanning hardware, synthesis, fallback, and render.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MediaEdgeMusicPlan {
    /// Stable identity retained across substitutions.
    pub session: Symbol,
    /// Ordered, digital-first route candidates.
    pub endpoints: Vec<MusicRouteEndpoint>,
}

impl MediaEdgeMusicPlan {
    /// Builds the reviewed vertical; every physical edge is optional.
    pub fn standard(session: Symbol) -> Self {
        Self {
            session,
            endpoints: vec![
                endpoint("oasys-control", MusicRouteRole::Control, true),
                endpoint("oasys-audio", MusicRouteRole::Playback, true),
                endpoint("sim-synthesis", MusicRouteRole::Synthesis, false),
                endpoint("rx-v777-digital", MusicRouteRole::Playback, true),
                endpoint("heavy-daw-host", MusicRouteRole::Fallback, true),
                endpoint("offline-render", MusicRouteRole::OfflineRender, false),
            ],
        }
    }

    /// Returns true when mandatory synthesis and offline-render roles survive.
    pub fn survives(&self, failed: &[Symbol]) -> bool {
        self.endpoints
            .iter()
            .filter(|endpoint| !endpoint.optional)
            .all(|endpoint| !failed.contains(&endpoint.id))
    }
}

fn endpoint(name: &str, role: MusicRouteRole, optional: bool) -> MusicRouteEndpoint {
    MusicRouteEndpoint {
        id: Symbol::qualified("music/route", name),
        role,
        optional,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn partial_physical_route_preserves_identity_and_offline_path() {
        let id = Symbol::qualified("music/session", "atelier");
        let plan = MediaEdgeMusicPlan::standard(id.clone());
        let failed = [
            "oasys-control",
            "oasys-audio",
            "rx-v777-digital",
            "heavy-daw-host",
        ]
        .map(|name| Symbol::qualified("music/route", name));
        assert!(plan.survives(&failed));
        assert_eq!(plan.session, id);
        assert!(!plan.survives(&[Symbol::qualified("music/route", "offline-render")]));
        let evidence = [
            RouteEvidence::Healthy {
                latency_us: 1_400,
                jitter_us: 90,
            },
            RouteEvidence::Dropout { lost_frames: 32 },
            RouteEvidence::Reconnected {
                attempts: 2,
                downtime_ms: 80,
            },
            RouteEvidence::StaleObservation { age_ms: 500 },
            RouteEvidence::Unsupported {
                reason: "OASYS absent".into(),
            },
            RouteEvidence::Disconnected {
                reason: "cable removed".into(),
            },
        ];
        assert_eq!(evidence.len(), 6);
        assert_ne!(evidence[4], evidence[5]);
    }
}
