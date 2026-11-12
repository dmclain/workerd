// Copyright (c) 2026 Cloudflare, Inc.
// Licensed under the Apache 2.0 license found in the LICENSE file or at:
//     https://opensource.org/licenses/Apache-2.0

// `#[jsg_resource(snapshot_stateless)]` copies a resource into a startup snapshot as a new one,
// which would drop the state of a resource with fields.
use jsg_macros::jsg_resource;

#[jsg_resource(snapshot_stateless)]
struct Holder {
    //~^ ERROR `snapshot_stateless` is for a #[jsg_resource] struct without fields
    name: String,
}

#[jsg_resource]
impl Holder {}
