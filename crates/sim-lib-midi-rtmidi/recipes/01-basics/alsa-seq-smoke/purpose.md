# ALSA sequencer smoke check

This recipe records the operator-gated ALSA sequencer loopback check for the
RtMidi adapter. Set `SIM_RTMIDI_HARDWARE_SMOKE=1` plus `SIM_RTMIDI_INPUT` and
`SIM_RTMIDI_OUTPUT` to matching local ALSA sequencer ports, then run the gated
hardware test.
