#[path = "../01-end-to-end/row-to-audition/setup.rs"]
mod setup;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let outcome = setup::row_to_audition()?;
    println!(
        "serial-workbench: OK strict_notes={} modal_notes={} midi_bytes={} lilypond_chars={} audition_events={}",
        outcome.strict_note_count,
        outcome.modal_note_count,
        outcome.midi_bytes,
        outcome.lilypond_chars,
        outcome.audition_events,
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::setup;

    #[test]
    fn fixture_manifest_is_complete_and_public_safe() {
        setup::validate_fixture_manifest().expect("fixture manifest");
    }

    #[test]
    fn end_to_end_workbench_recipe_runs() {
        let outcome = setup::row_to_audition().expect("workbench");
        assert!(outcome.strict_note_count > 0);
        assert!(outcome.modal_note_count > 0);
        assert!(outcome.midi_bytes > 4);
        assert!(outcome.lilypond_chars > 0);
    }
}
