// Copyright (c) The Move Contributors
// SPDX-License-Identifier: Apache-2.0

//! Defines data types produced by the VM session.

use crate::values::Value;
use move_core_types::value::MoveTypeLayout;
use std::sync::Arc;

/// Represents the data produced by the VM session.
pub enum Data {
    // Session created/modified serialized bytes.
    Serialized(Arc<Vec<u8>>),
    // Session created/modified Move value.
    Cached(Arc<Value>, MoveTypeLayout),
}
