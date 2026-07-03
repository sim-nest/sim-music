# sim-lib-sound-tuning

In one line: A collection of tuning systems, from modern equal temperament to historical temperaments, that decide the exact pitch of every note.

## What it gives you

This provides tuning systems and temperaments as interchangeable objects. It includes equal temperament, just intonation, Pythagorean, quarter-comma meantone, Werckmeister III, Young, and arbitrary Scala cents tables, each mapping pitches to and from frequencies. A serialisable descriptor can build any of them, and a runtime surface installs the built-in tuning cards as a library, so switching how an instrument is tuned is a matter of choosing a different system.

## Why you will be glad

- Tune an instrument to historical or modern systems at will.
- Load custom tunings from standard Scala cents tables.
- Swap temperaments without touching the notes themselves.

## Where it fits

This is the tuning authority of the SIM sound family. The sound bridge asks it for the exact frequency of each note, so the constellation can play the same music in equal temperament or in a period tuning simply by choosing which system is in force.
