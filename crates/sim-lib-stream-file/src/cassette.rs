use sim_kernel::{Expr, Result};
use sim_lib_stream_core::{
    StreamCassette, StreamGoldenFixtureReport, StreamValue, TransportProfile,
};

/// Records a stream into a cassette using the given transport profile.
pub fn stream_to_cassette(
    stream: &StreamValue,
    profile: TransportProfile,
) -> Result<StreamCassette> {
    StreamCassette::from_stream_value(stream, profile)
}

/// Records a stream into a cassette and encodes it as an expression.
pub fn stream_to_cassette_expr(stream: &StreamValue, profile: TransportProfile) -> Result<Expr> {
    Ok(stream_to_cassette(stream, profile)?.to_expr())
}

/// Decodes a cassette expression and replays it as a stream.
pub fn cassette_expr_to_stream(expr: &Expr) -> Result<StreamValue> {
    StreamCassette::from_expr(expr)?.replay_stream_value()
}

/// Replays a recorded cassette back into a stream.
pub fn cassette_to_stream(cassette: &StreamCassette) -> Result<StreamValue> {
    cassette.replay_stream_value()
}

/// Validates a cassette against a golden fixture file at `path`.
pub fn validate_cassette_fixture_path(
    cassette: &StreamCassette,
    path: &str,
) -> Result<StreamGoldenFixtureReport> {
    cassette.validate_golden_fixture(path)
}
