use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
    sync::Arc,
};

use sim_codec::{Input, Output, decode_with_codec, encode_with_codec};
use sim_kernel::{
    Cx, EncodeOptions, Error, Expr, Object, ObjectEncode, ObjectEncoding, ReadPolicy, Result,
    Symbol, Value,
    id::CORE_TABLE_CLASS_ID,
    object::ClassRef,
    table::{Dir, Table},
};

use crate::{
    citizen::fs_dir_class_symbol,
    roadmap11::{decode_expr_for_ext, encode_expr_for_ext, infer_ext_from_expr, known_exts},
    table_fs_capability, table_fs_mkdir_capability, table_fs_read_capability,
    table_fs_rmdir_capability, table_fs_write_capability,
};

const DEFAULT_EXT: &str = "siml";

/// A SIM table backed by a host directory rooted at a canonical path.
#[derive(Clone)]
pub struct FsDir {
    root: PathBuf,
}

impl FsDir {
    /// Opens (creating if needed) the directory at `root` as a filesystem table.
    ///
    /// The root is created if it does not exist and then canonicalized; an I/O
    /// failure on either step is reported as an error.
    pub fn open(root: PathBuf) -> Result<Self> {
        std::fs::create_dir_all(&root)
            .map_err(|err| Error::Eval(format!("table/fs: cannot open root: {err}")))?;
        let root = std::fs::canonicalize(&root)
            .map_err(|err| Error::Eval(format!("table/fs: cannot open root: {err}")))?;
        Ok(Self { root })
    }

    fn segment(&self, name: &Symbol) -> Result<PathBuf> {
        let segment = name.name.as_ref();
        // The shared predicate rejects empty/`.`/`..`/`/`/`\`; table-fs keeps the
        // additional, stricter `is_absolute()` guard on top of it.
        if !sim_table_core::is_legal_table_segment(segment) || Path::new(segment).is_absolute() {
            return Err(Error::Eval(format!("table/fs: illegal name {segment:?}")));
        }
        let path = self.root.join(segment);
        self.ensure_within_root(&path)?;
        Ok(path)
    }

    fn ensure_within_root(&self, path: &Path) -> Result<()> {
        let candidate = if path.exists() {
            std::fs::canonicalize(path)
                .map_err(|err| Error::Eval(format!("table/fs: path check {err}")))?
        } else {
            path.to_path_buf()
        };
        if candidate.starts_with(&self.root) {
            Ok(())
        } else {
            Err(Error::Eval(format!(
                "table/fs: path escapes root: {}",
                path.display()
            )))
        }
    }

    fn leaf_candidates(&self, name: &Symbol) -> Result<Vec<(PathBuf, &'static str)>> {
        let base = self.segment(name)?;
        let mut matches = Vec::new();
        for ext in known_exts() {
            let path = base.with_extension(ext);
            self.ensure_within_root(&path)?;
            if path.is_file() {
                matches.push((path, ext));
            }
        }
        Ok(matches)
    }

    fn leaf_path_for_read(&self, name: &Symbol) -> Result<Option<(PathBuf, &'static str)>> {
        let matches = self.leaf_candidates(name)?;
        match matches.len() {
            0 => Ok(None),
            1 => Ok(matches.into_iter().next()),
            _ => Err(Error::Eval(format!(
                "table/fs: multiple leaf files found for key {name}"
            ))),
        }
    }

    fn codec_for_ext(ext: &str) -> Result<Symbol> {
        match ext {
            "siml" => Ok(Symbol::qualified("codec", "lisp")),
            "simb" => Ok(Symbol::qualified("codec", "binary")),
            "simb64" => Ok(Symbol::qualified("codec", "binary-base64")),
            "simj" => Ok(Symbol::qualified("codec", "json")),
            "sima" => Ok(Symbol::qualified("codec", "algol")),
            other => Err(Error::Eval(format!("table/fs: unknown extension {other}"))),
        }
    }

    fn decode_expr_bytes(cx: &mut Cx, codec: &Symbol, bytes: &[u8]) -> Result<Expr> {
        decode_with_codec(
            cx,
            codec,
            Input::Bytes(bytes.to_vec()),
            ReadPolicy::default(),
        )
    }

    fn encode_expr_bytes(cx: &mut Cx, codec: &Symbol, expr: &Expr) -> Result<Vec<u8>> {
        match encode_with_codec(cx, codec, expr, EncodeOptions::default())? {
            Output::Text(text) => Ok(text.into_bytes()),
            Output::Bytes(bytes) => Ok(bytes),
        }
    }
}

impl Object for FsDir {
    fn display(&self, _cx: &mut Cx) -> Result<String> {
        Ok(format!("table/fs[{}]", self.root.display()))
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl sim_kernel::ObjectCompat for FsDir {
    fn class(&self, cx: &mut Cx) -> Result<ClassRef> {
        let symbol = fs_dir_class_symbol();
        if let Some(value) = cx.registry().class_by_symbol(&symbol) {
            return Ok(value.clone());
        }
        let symbol = Symbol::qualified("core", "Table");
        if let Some(value) = cx.registry().class_by_symbol(&symbol) {
            return Ok(value.clone());
        }
        cx.factory().class_stub(CORE_TABLE_CLASS_ID, symbol)
    }
    fn as_expr(&self, cx: &mut Cx) -> Result<Expr> {
        self.as_table_expr(cx)
    }
    fn truth(&self, cx: &mut Cx) -> Result<bool> {
        Ok(!self.is_empty(cx)?)
    }
    fn as_table_impl(&self) -> Option<&dyn Table> {
        Some(self)
    }
    fn as_dir(&self) -> Option<&dyn Dir> {
        Some(self)
    }
    fn as_object_encoder(&self) -> Option<&dyn ObjectEncode> {
        Some(self)
    }
}

impl ObjectEncode for FsDir {
    fn object_encoding(&self, _cx: &mut Cx) -> Result<ObjectEncoding> {
        Ok(ObjectEncoding::Constructor {
            class: fs_dir_class_symbol(),
            args: vec![
                Expr::Symbol(Symbol::new("v0")),
                Expr::String(self.root.display().to_string()),
            ],
        })
    }
}

impl sim_citizen::Citizen for FsDir {
    fn citizen_symbol() -> Symbol {
        fs_dir_class_symbol()
    }

    fn citizen_version() -> u32 {
        0
    }

    fn citizen_arity() -> usize {
        1
    }

    fn citizen_fields() -> &'static [&'static str] {
        &["root"]
    }
}

impl Table for FsDir {
    fn backend_symbol(&self) -> Symbol {
        Symbol::qualified("table", "fs")
    }

    fn get(&self, cx: &mut Cx, key: Symbol) -> Result<Value> {
        cx.require(&table_fs_read_capability())?;
        match self.leaf_path_for_read(&key)? {
            Some((path, ext)) => {
                let bytes = std::fs::read(&path)
                    .map_err(|err| Error::Eval(format!("table/fs: read {err}")))?;
                let expr = match decode_expr_for_ext(ext, &bytes) {
                    Some(expr) => expr?,
                    None => {
                        let codec = Self::codec_for_ext(ext)?;
                        Self::decode_expr_bytes(cx, &codec, &bytes)?
                    }
                };
                cx.factory().expr(expr)
            }
            None => cx.factory().nil(),
        }
    }

    fn set(&self, cx: &mut Cx, key: Symbol, value: Value) -> Result<()> {
        cx.require(&table_fs_write_capability())?;
        let base = self.segment(&key)?;
        if base.is_dir() {
            return Err(Error::Eval(format!("table/fs: {key} is a directory")));
        }
        let existing_leaf = self.leaf_path_for_read(&key)?;
        for (path, _) in self.leaf_candidates(&key)? {
            if Some(path.clone()) != existing_leaf.as_ref().map(|(path, _)| path.clone())
                && path.extension().and_then(|ext| ext.to_str()) != Some(DEFAULT_EXT)
            {
                std::fs::remove_file(&path)
                    .map_err(|err| Error::Eval(format!("table/fs: write {err}")))?;
            }
        }
        let expr = value.object().as_expr(cx)?;
        let ext = existing_leaf
            .as_ref()
            .map(|(_, ext)| *ext)
            .or_else(|| infer_ext_from_expr(&expr))
            .unwrap_or(DEFAULT_EXT);
        let path = base.with_extension(ext);
        self.ensure_within_root(&path)?;
        let bytes = match encode_expr_for_ext(ext, &expr) {
            Some(bytes) => bytes?,
            None => {
                let codec = Symbol::qualified("codec", "lisp");
                Self::encode_expr_bytes(cx, &codec, &expr)?
            }
        };
        std::fs::write(&path, bytes)
            .map_err(|err| Error::Eval(format!("table/fs: write {err}")))?;
        Ok(())
    }

    fn has(&self, cx: &mut Cx, key: Symbol) -> Result<bool> {
        cx.require(&table_fs_read_capability())?;
        let path = self.segment(&key)?;
        Ok(path.is_dir() || self.leaf_path_for_read(&key)?.is_some())
    }

    fn del(&self, cx: &mut Cx, key: Symbol) -> Result<Value> {
        cx.require(&table_fs_write_capability())?;
        match self.leaf_path_for_read(&key)? {
            Some((path, ext)) => {
                let bytes = std::fs::read(&path).unwrap_or_default();
                std::fs::remove_file(&path)
                    .map_err(|err| Error::Eval(format!("table/fs: del {err}")))?;
                let expr = match decode_expr_for_ext(ext, &bytes) {
                    Some(expr) => expr,
                    None => {
                        let codec = Self::codec_for_ext(ext)?;
                        Self::decode_expr_bytes(cx, &codec, &bytes)
                    }
                };
                match expr {
                    Ok(expr) => cx.factory().expr(expr),
                    Err(_) => cx.factory().nil(),
                }
            }
            None => cx.factory().nil(),
        }
    }

    fn keys(&self, cx: &mut Cx) -> Result<Vec<Symbol>> {
        cx.require(&table_fs_read_capability())?;
        let mut keys = BTreeSet::new();
        let entries = std::fs::read_dir(&self.root)
            .map_err(|err| Error::Eval(format!("table/fs: read_dir {err}")))?;
        for entry in entries {
            let entry = entry.map_err(|err| Error::Eval(format!("table/fs: {err}")))?;
            let path = entry.path();
            self.ensure_within_root(&path)?;
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.') {
                continue;
            }
            if path.is_dir() {
                keys.insert(Symbol::new(name));
                continue;
            }
            let Some(stem) = known_exts().into_iter().find_map(|ext| {
                name.strip_suffix(&format!(".{ext}"))
                    .map(std::borrow::ToOwned::to_owned)
            }) else {
                continue;
            };
            keys.insert(Symbol::new(stem));
        }
        Ok(keys.into_iter().collect())
    }

    fn entries(&self, cx: &mut Cx) -> Result<Vec<(Symbol, Value)>> {
        cx.require(&table_fs_read_capability())?;
        let mut entries = Vec::new();
        for key in self.keys(cx)? {
            if self.is_dir(cx, key.clone())? {
                continue;
            }
            entries.push((key.clone(), self.get(cx, key)?));
        }
        Ok(entries)
    }

    fn len(&self, cx: &mut Cx) -> Result<usize> {
        Ok(self.entries(cx)?.len())
    }

    fn clear(&self, cx: &mut Cx) -> Result<()> {
        cx.require(&table_fs_write_capability())?;
        for key in self.keys(cx)? {
            if !self.is_dir(cx, key.clone())? {
                let _ = self.del(cx, key)?;
            }
        }
        Ok(())
    }
}

impl Dir for FsDir {
    fn mkdir(&self, cx: &mut Cx, name: Symbol) -> Result<Value> {
        cx.require(&table_fs_mkdir_capability())?;
        let path = self.segment(&name)?;
        if self.leaf_path_for_read(&name)?.is_some() {
            return Err(Error::Eval(format!("table/fs: {name} is a file")));
        }
        std::fs::create_dir_all(&path)
            .map_err(|err| Error::Eval(format!("table/fs: mkdir {err}")))?;
        cx.factory().opaque(Arc::new(Self::open(path)?))
    }

    fn opendir(&self, cx: &mut Cx, name: Symbol) -> Result<Option<Value>> {
        cx.require(&table_fs_read_capability())?;
        let path = self.segment(&name)?;
        if path.is_dir() {
            Ok(Some(cx.factory().opaque(Arc::new(Self::open(path)?))?))
        } else if path.exists() || self.leaf_path_for_read(&name)?.is_some() {
            Err(Error::Eval(format!("table/fs: {name} is not a directory")))
        } else {
            Ok(None)
        }
    }

    fn rmdir(&self, cx: &mut Cx, name: Symbol) -> Result<Value> {
        cx.require(&table_fs_rmdir_capability())?;
        let path = self.segment(&name)?;
        if !path.is_dir() {
            return Err(Error::Eval(format!("table/fs: {name} is not a directory")));
        }
        std::fs::remove_dir_all(&path)
            .map_err(|err| Error::Eval(format!("table/fs: rmdir {err}")))?;
        cx.factory().nil()
    }

    fn is_dir(&self, cx: &mut Cx, name: Symbol) -> Result<bool> {
        cx.require(&table_fs_read_capability())?;
        Ok(self.segment(&name)?.is_dir())
    }
}

/// Opens a filesystem table at `root` and returns it as a runtime table value.
///
/// Requires the table-fs capability; the returned value wraps an [`FsDir`].
pub fn install_fs_dir_lib(cx: &mut Cx, root: &str) -> Result<Value> {
    cx.require(&table_fs_capability())?;
    let dir = FsDir::open(PathBuf::from(root))?;
    cx.factory().opaque(Arc::new(dir))
}
