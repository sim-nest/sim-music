use sim_citizen_derive::Citizen;
use sim_kernel::Symbol;

/// Citizen descriptor identifying a filesystem-backed table by its root path.
#[derive(Clone, Debug, Default, PartialEq, Citizen)]
#[citizen(symbol = "table/FsDir", version = 0)]
pub struct FsDirDescriptor {
    /// Host filesystem path serving as the table root.
    pub root: String,
}

/// Returns the `table/FsDir` class symbol for the filesystem table.
///
/// # Examples
///
/// ```
/// use sim_table_fs::fs_dir_class_symbol;
///
/// assert_eq!(&*fs_dir_class_symbol().name, "FsDir");
/// ```
pub fn fs_dir_class_symbol() -> Symbol {
    Symbol::qualified("table", "FsDir")
}
