# sim-lib-midi-live

In one line: Fast in-and-out buffers that let live MIDI flow through your setup without stalling when things get busy.

## What it gives you

This gives you fixed-size ring buffers that sit between real-time MIDI coming in and the tools that consume it. One buffer acts as both a place to write events and a place to read them back in order; another tags each event with the track it belongs to. If a consumer falls behind and a buffer fills, the oldest event is dropped and counted rather than blocking the live input, so a slow reader never freezes a live player.

## Why you will be glad

- Keep a live keyboard responsive even when downstream tools lag.
- See a count of dropped events instead of a silent stall.
- Route events by track with per-lane tagging built in.

## Where it fits

This is the live plumbing of the SIM MIDI stack. It bridges real-time input into the shared source and sink traits and publishes the buffers as runtime plugin rows, so performance tools can pull from a keyboard or transport without worrying about timing pressure.
