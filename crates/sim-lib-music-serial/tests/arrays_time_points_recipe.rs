// SPDX-License-Identifier: MPL-2.0
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// conformance: serial arrays and time points keep the immutable plan's temporal evidence.
include!("../recipes/01-basics/arrays-time-points/setup.rs");

#[test]
fn public_arrays_time_points_recipe_runs() {
    arrays_time_points().expect("arrays and time points scenario");
}
