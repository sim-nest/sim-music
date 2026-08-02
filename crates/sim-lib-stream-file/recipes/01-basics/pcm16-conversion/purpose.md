# Bounded PCM16 conversion at the interchange owner

Build a finite `ChannelMatrix`, choose a `QuantizationPolicy`, and call
`convert_f32_to_pcm16` or `convert_f32_to_wav_bytes`. The latter routes the
result through the existing canonical PCM16 WAV encoder.

The conversion report keeps frame and channel shape, the mapped float peak,
every out-of-range sample, quantization-error RMS, and the exact seeded TPDF or
first-order noise-shaped dither policy. Analysis and device providers do not
own or duplicate this conversion.
