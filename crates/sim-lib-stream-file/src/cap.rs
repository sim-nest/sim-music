use sim_kernel::{CapabilityName, Cx};

/// Returns the capability gating filesystem reads (`fs/read`).
///
/// # Examples
///
/// ```
/// use sim_lib_stream_file::stream_file_read_capability;
///
/// assert_eq!(stream_file_read_capability().as_str(), "fs/read");
/// ```
pub fn stream_file_read_capability() -> CapabilityName {
    CapabilityName::new("fs/read")
}

/// Returns the capability gating filesystem writes (`fs/write`).
pub fn stream_file_write_capability() -> CapabilityName {
    CapabilityName::new("fs/write")
}

pub(crate) fn stream_file_read_effect_capability(cx: &Cx) -> CapabilityName {
    granted_capability_or_alias(cx, stream_file_read_capability(), fs_read_aliases())
        .unwrap_or_else(stream_file_read_capability)
}

pub(crate) fn stream_file_write_effect_capability(cx: &Cx) -> CapabilityName {
    granted_capability_or_alias(cx, stream_file_write_capability(), fs_write_aliases())
        .unwrap_or_else(stream_file_write_capability)
}

fn fs_read_aliases() -> &'static [&'static str] {
    &["table.fs.read", "stream.file.read", "file-read"]
}

fn fs_write_aliases() -> &'static [&'static str] {
    &[
        "table.fs.write",
        "table.fs.mkdir",
        "table.fs.rmdir",
        "stream.file.write",
        "file-write",
    ]
}

fn granted_capability_or_alias(
    cx: &Cx,
    canonical: CapabilityName,
    aliases: &'static [&'static str],
) -> Option<CapabilityName> {
    if cx.capabilities().contains(&canonical) {
        return Some(canonical);
    }
    aliases
        .iter()
        .copied()
        .map(CapabilityName::new)
        .find(|alias| cx.capabilities().contains(alias))
}
