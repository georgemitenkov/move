// Copyright (c) The Move Contributors
// SPDX-License-Identifier: Apache-2.0

//! Traits for resolving Move resources from persistent storage at runtime.

use crate::{effects::Data, values::Value};
use move_core_types::{
    account_address::AccountAddress, language_storage::StructTag, resolver::ModuleResolver,
};
use std::{fmt::Debug, sync::Arc};

/// Represents any resource stored in persisten storage or cache.
pub enum Resource {
    // Resource is stored as a blob.
    Serialized(Arc<Vec<u8>>),
    // Resource is stored as a Move value and is not serialized yet. This type is
    // useful to cache outputs of VM session and avoid unnecessary deserializations.
    Cached(Arc<Value>),
}

impl From<Vec<u8>> for Resource {
    fn from(blob: Vec<u8>) -> Self {
        Resource::Serialized(Arc::new(blob))
    }
}

impl From<&Vec<u8>> for Resource {
    fn from(blob: &Vec<u8>) -> Self {
        Resource::Serialized(Arc::new(blob.clone()))
    }
}

impl From<Data> for Resource {
    fn from(data: Data) -> Self {
        match data {
            Data::Serialized(blob) => Resource::Serialized(blob),
            Data::Cached(value, _) => Resource::Cached(value),
        }
    }
}

impl From<&Data> for Resource {
    fn from(data: &Data) -> Self {
        match data {
            Data::Serialized(blob) => Resource::Serialized(Arc::clone(blob)),
            Data::Cached(value, _) => Resource::Cached(Arc::clone(value)),
        }
    }
}

/// Any persistent storage backend or cache that can resolve resources by
/// address and type at runtime. Storage backends should return:
///   - Ok(Some(..)) if the data exists
///   - Ok(None)     if the data does not exist
///   - Err(..)      only when something really wrong happens, for example
///                    - invariants are broken and observable from the storage side
///                      (this is not currently possible as ModuleId and StructTag
///                       are always structurally valid)
///                    - storage encounters internal error
pub trait ResourceResolver {
    type Error: Debug;

    fn get_resource(
        &self,
        address: &AccountAddress,
        typ: &StructTag,
    ) -> Result<Option<Resource>, Self::Error>;
}

/// A persistent storage implementation that can resolve both resources and
/// modules at runtime.
pub trait MoveResolver:
    ModuleResolver<Error = Self::Err> + ResourceResolver<Error = Self::Err>
{
    type Err: Debug;
}

impl<E: Debug, T: ModuleResolver<Error = E> + ResourceResolver<Error = E> + ?Sized> MoveResolver
    for T
{
    type Err = E;
}

impl<T: ResourceResolver + ?Sized> ResourceResolver for &T {
    type Error = T::Error;

    fn get_resource(
        &self,
        address: &AccountAddress,
        tag: &StructTag,
    ) -> Result<Option<Resource>, Self::Error> {
        (**self).get_resource(address, tag)
    }
}
