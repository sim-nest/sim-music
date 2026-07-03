use sim_kernel::CapabilityName;

/// Returns the capability gating filesystem reads (`stream.file.read`).
///
/// # Examples
///
/// ```
/// use sim_lib_stream_file::stream_file_read_capability;
///
/// assert_eq!(stream_file_read_capability().as_str(), "stream.file.read");
/// ```
pub fn stream_file_read_capability() -> CapabilityName {
    CapabilityName::new("stream.file.read")
}

/// Returns the capability gating filesystem writes (`stream.file.write`).
pub fn stream_file_write_capability() -> CapabilityName {
    CapabilityName::new("stream.file.write")
}
