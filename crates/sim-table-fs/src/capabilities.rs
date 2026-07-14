//! Capability names used by filesystem-backed tables.

use sim_kernel::CapabilityName;

/// The capability gating filesystem-backed tables (`table.fs`).
pub fn table_fs_capability() -> CapabilityName {
    CapabilityName::new("table.fs")
}

/// The capability gating filesystem table reads (`table.fs.read`).
pub fn table_fs_read_capability() -> CapabilityName {
    CapabilityName::new("table.fs.read")
}

/// The capability gating filesystem table writes (`table.fs.write`).
pub fn table_fs_write_capability() -> CapabilityName {
    CapabilityName::new("table.fs.write")
}

/// The capability gating filesystem table directory creation (`table.fs.mkdir`).
pub fn table_fs_mkdir_capability() -> CapabilityName {
    CapabilityName::new("table.fs.mkdir")
}

/// The capability gating filesystem table directory removal (`table.fs.rmdir`).
pub fn table_fs_rmdir_capability() -> CapabilityName {
    CapabilityName::new("table.fs.rmdir")
}

#[cfg(test)]
mod tests {
    use super::{
        table_fs_capability, table_fs_mkdir_capability, table_fs_read_capability,
        table_fs_rmdir_capability, table_fs_write_capability,
    };

    #[test]
    fn capability_tokens_are_stable() {
        assert_eq!(table_fs_capability().as_str(), "table.fs");
        assert_eq!(table_fs_read_capability().as_str(), "table.fs.read");
        assert_eq!(table_fs_write_capability().as_str(), "table.fs.write");
        assert_eq!(table_fs_mkdir_capability().as_str(), "table.fs.mkdir");
        assert_eq!(table_fs_rmdir_capability().as_str(), "table.fs.rmdir");
    }
}
