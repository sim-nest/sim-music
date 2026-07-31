use crate::{
    ChannelMatrix, DitherPolicy, PcmConversionError, QuantizationPolicy, convert_f32_to_pcm16,
    convert_f32_to_wav_bytes,
};

#[test]
fn explicit_channel_matrices_preserve_frames_and_report_clipping() {
    let mono = [0.25, -0.5, 1.25];
    let policy = QuantizationPolicy {
        max_frames: 3,
        dither: DitherPolicy::None,
    };
    let conversion = convert_f32_to_pcm16(&mono, &ChannelMatrix::mono_to_stereo(), policy).unwrap();

    assert_eq!(conversion.samples.len(), 6);
    assert_eq!(conversion.samples[0], conversion.samples[1]);
    assert_eq!(conversion.samples[2], conversion.samples[3]);
    assert_eq!(conversion.report.frames, 3);
    assert_eq!(conversion.report.output_channels, 2);
    assert_eq!(conversion.report.clipped_samples, 2);
    assert_eq!(conversion.report.peak_before_quantization, 1.25);

    let downmix = convert_f32_to_pcm16(
        &[0.75, 0.25, -0.5, 0.5],
        &ChannelMatrix::stereo_to_mono(),
        policy,
    )
    .unwrap();
    assert_eq!(downmix.samples, vec![16_384, 0]);
}

#[test]
fn seeded_tpdf_and_noise_shaping_are_reproducible_and_distinct() {
    let input = vec![1.0 / 65_536.0; 2_048];
    let matrix = ChannelMatrix::identity(1).unwrap();
    let tpdf = QuantizationPolicy {
        max_frames: input.len(),
        dither: DitherPolicy::Tpdf { seed: 0x5eed },
    };
    let first = convert_f32_to_pcm16(&input, &matrix, tpdf).unwrap();
    let second = convert_f32_to_pcm16(&input, &matrix, tpdf).unwrap();
    assert_eq!(first, second);
    assert!(first.samples.iter().any(|sample| *sample != 1));

    let shaped = convert_f32_to_pcm16(
        &input,
        &matrix,
        QuantizationPolicy {
            max_frames: input.len(),
            dither: DitherPolicy::NoiseShapedTpdf {
                seed: 0x5eed,
                feedback: 0.85,
            },
        },
    )
    .unwrap();
    assert_ne!(first.samples, shaped.samples);
    assert_eq!(shaped.report.clipped_samples, 0);
}

#[test]
fn conversion_bounds_fail_closed_before_output() {
    let matrix = ChannelMatrix::identity(2).unwrap();
    let error = convert_f32_to_pcm16(
        &[0.0; 6],
        &matrix,
        QuantizationPolicy {
            max_frames: 2,
            dither: DitherPolicy::None,
        },
    )
    .unwrap_err();
    assert_eq!(
        error,
        PcmConversionError::FrameLimit {
            supplied: 3,
            maximum: 2,
        }
    );
}

#[test]
fn float_conversion_flows_through_canonical_pcm16_wav_encoder() {
    let (bytes, report) = convert_f32_to_wav_bytes(
        48_000,
        &[0.0, 0.5, -0.5, 1.1],
        &ChannelMatrix::identity(2).unwrap(),
        QuantizationPolicy {
            max_frames: 2,
            dither: DitherPolicy::Tpdf { seed: 7 },
        },
    )
    .unwrap();
    assert_eq!(&bytes[0..4], b"RIFF");
    assert_eq!(&bytes[8..12], b"WAVE");
    assert_eq!(report.frames, 2);
    assert_eq!(report.clipped_samples, 1);
}
