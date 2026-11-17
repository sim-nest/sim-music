// SPDX-License-Identifier: MPL-2.0
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// conformance: row partitions cover the twelve positions and report mosaic structure.
include!("../recipes/02-partitions/partition-mosaic/setup.rs");

#[test]
fn public_partition_mosaic_recipe_runs() {
    partition_mosaic().expect("partition mosaic scenario");
}
