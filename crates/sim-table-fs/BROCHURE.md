# sim-table-fs

In one line: Turns a folder on disk into a lookup table where each entry is a file and each subfolder is a nested table.

## What it gives you

This exposes a host directory as a SIM table: every table key maps to a file, and nested tables map to subdirectories, so a folder tree becomes a structured store you can read and write by key. Access is gated by the kernel's table capabilities and passes through the configured codec. With its format options enabled, recognised extensions -- for example MIDI, music, tone, and tuning files -- round-trip automatically through their domain shapes.

## Why you will be glad

- Read and write a directory as a keyed table.
- Map nested folders to nested tables without extra work.
- Have known file types decode into their domain objects automatically.

## Where it fits

This is the filesystem-backed store of the SIM stack, the place where music and sound material can live as ordinary files while still being addressable as a table. It lets the constellation persist and browse projects on disk under the kernel's capability rules.
