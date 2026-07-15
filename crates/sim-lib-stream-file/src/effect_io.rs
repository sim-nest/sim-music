use std::{fs, path::Path};

use sim_kernel::{Cx, Datum, DatumStore, Effect, Error, Ref, Result, Symbol, core_any_ref, effect};

use crate::cap::{stream_file_read_effect_capability, stream_file_write_effect_capability};

pub(crate) fn read_file_with_effect(cx: &mut Cx, path: impl AsRef<Path>) -> Result<Vec<u8>> {
    let path = path.as_ref().to_path_buf();
    let input = operation_input_ref(cx, "read", &path, None)?;
    let effect = Effect::new(
        effect::effect_filesystem_kind(),
        Ref::Symbol(Symbol::qualified("stream/file", "read")),
        input,
        core_any_ref(),
        effect::effect_resume_op_key(),
        effect::effect_abort_op_key(),
    )
    .requiring(stream_file_read_effect_capability(cx))
    .with_replay_key(Some(Ref::Symbol(Symbol::qualified(
        "stream/file",
        "read-v1",
    ))))?;
    let result = effect::resolve_effect(cx, effect, move |cx, _effect| {
        let bytes = fs::read(&path).map_err(|err| io_error("read", &path, err))?;
        bytes_ref(cx, bytes)
    })?;
    bytes_from_ref(cx, &result)
}

pub(crate) fn write_file_with_effect(
    cx: &mut Cx,
    path: impl AsRef<Path>,
    bytes: Vec<u8>,
) -> Result<()> {
    let path = path.as_ref().to_path_buf();
    let input = operation_input_ref(cx, "write", &path, Some(&bytes))?;
    let effect = Effect::new(
        effect::effect_filesystem_kind(),
        Ref::Symbol(Symbol::qualified("stream/file", "write")),
        input,
        core_any_ref(),
        effect::effect_resume_op_key(),
        effect::effect_abort_op_key(),
    )
    .requiring(stream_file_write_effect_capability(cx))
    .with_replay_key(Some(Ref::Symbol(Symbol::qualified(
        "stream/file",
        "write-v1",
    ))))?;
    effect::resolve_effect(cx, effect, move |cx, _effect| {
        fs::write(&path, &bytes).map_err(|err| io_error("write", &path, err))?;
        ok_ref(cx)
    })?;
    Ok(())
}

fn operation_input_ref(
    cx: &mut Cx,
    action: &str,
    path: &Path,
    bytes: Option<&[u8]>,
) -> Result<Ref> {
    let mut fields = vec![
        (Symbol::new("action"), Datum::String(action.to_owned())),
        (
            Symbol::new("path"),
            Datum::String(path.to_string_lossy().into_owned()),
        ),
    ];
    if let Some(bytes) = bytes {
        fields.push((Symbol::new("bytes"), Datum::Bytes(bytes.to_vec())));
    }
    let id = cx.datum_store_mut().intern(Datum::Node {
        tag: Symbol::qualified("stream/file", "Operation"),
        fields,
    })?;
    Ok(Ref::Content(id))
}

fn bytes_ref(cx: &mut Cx, bytes: Vec<u8>) -> Result<Ref> {
    Ok(Ref::Content(
        cx.datum_store_mut().intern(Datum::Bytes(bytes))?,
    ))
}

fn bytes_from_ref(cx: &mut Cx, reference: &Ref) -> Result<Vec<u8>> {
    let Ref::Content(id) = reference else {
        return Err(Error::Eval(
            "filesystem read effect returned a non-content ref".to_owned(),
        ));
    };
    match cx.datum_store().get(id)? {
        Some(Datum::Bytes(bytes)) => Ok(bytes.clone()),
        Some(_) => Err(Error::Eval(
            "filesystem read effect returned non-byte content".to_owned(),
        )),
        None => Err(Error::Eval(
            "filesystem read effect returned missing content".to_owned(),
        )),
    }
}

fn ok_ref(cx: &mut Cx) -> Result<Ref> {
    Ok(Ref::Content(
        cx.datum_store_mut().intern(Datum::Bool(true))?,
    ))
}

fn io_error(action: &str, path: &Path, err: std::io::Error) -> Error {
    Error::Eval(format!(
        "failed to {action} stream file {}: {err}",
        path.display()
    ))
}
