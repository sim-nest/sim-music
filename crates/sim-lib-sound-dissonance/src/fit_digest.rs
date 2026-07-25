//! Stable fitting digest helpers.

use sim_lib_discrete_search::SearchReceipt;

use crate::{SonanceCorpusMeta, SonanceFitCandidate, SonanceObservation};

pub(crate) fn corpus_digest(id: &str, observations: &[SonanceObservation]) -> String {
    let mut parts = vec![id.to_owned()];
    parts.extend(observations.iter().map(|observation| {
        let bins = observation
            .bins
            .iter()
            .map(|(frequency, amplitude)| format!("{:.7}@{:.3}", frequency.0, amplitude.0))
            .collect::<Vec<_>>()
            .join(",");
        format!("{}|{}|{:.3}", observation.id, bins, observation.target_rank)
    }));
    stable_digest_string(&parts.iter().map(String::as_str).collect::<Vec<_>>())
}

pub(crate) fn report_digest(
    receipt: &SearchReceipt,
    corpora: &[SonanceCorpusMeta],
    candidates: &[SonanceFitCandidate],
) -> String {
    let mut parts = vec![receipt.digest.as_str()];
    parts.extend(corpora.iter().map(|meta| meta.corpus_hash.as_str()));
    let candidate_parts = candidates
        .iter()
        .map(|candidate| {
            format!(
                "{:.6}:{:.6}:{:.6}:{:.6}:{:.6}",
                candidate.parameters.a,
                candidate.parameters.b,
                candidate.training.rank_correlation,
                candidate.validation.rank_correlation,
                candidate.locked_conformance.rank_correlation,
            )
        })
        .collect::<Vec<_>>();
    parts.extend(candidate_parts.iter().map(String::as_str));
    stable_digest_string(&parts)
}

fn stable_digest_string(parts: &[&str]) -> String {
    format!("fnv1a64:{:016x}", stable_digest_value(parts))
}

pub(crate) fn stable_digest_value(parts: &[&str]) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for part in parts {
        for byte in part.as_bytes() {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
        hash ^= 0xff;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}
