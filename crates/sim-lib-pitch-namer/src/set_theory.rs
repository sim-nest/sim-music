use sim_lib_pitch_set::PitchClassMask;

pub fn label_mask(mask: PitchClassMask) -> String {
    let pcs = mask
        .normalize()
        .pitch_classes()
        .into_iter()
        .map(|pc| pc.value().to_string())
        .collect::<Vec<_>>()
        .join(",");
    format!("set:[{pcs}]")
}
