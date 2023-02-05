// Copyright (c) The Move Contributors
// SPDX-License-Identifier: Apache-2.0

//! Defines data types produced by the VM session.

use crate::values::Value;
use anyhow::{bail, Result};
use move_binary_format::errors::{Location, PartialVMError, PartialVMResult, VMError, VMResult};
use move_core_types::{
    account_address::AccountAddress,
    effects::{AccountChangeSet as BlobAccountChangeSet, ChangeSet as BlobChangeSet, Op},
    identifier::Identifier,
    language_storage::{ModuleId, StructTag},
    value::MoveTypeLayout,
    vm_status::StatusCode,
};
use std::{
    collections::{btree_map::Entry, BTreeMap},
    sync::Arc,
};

/// Represents the data produced by the VM session.
#[derive(Debug, Clone)]
pub enum Data {
    // Session created/modified serialized bytes.
    Serialized(Arc<Vec<u8>>),
    // Session created/modified Move value.
    Cached(Arc<Value>, MoveTypeLayout),
}

impl Data {
    pub fn from_bytes(blob: Vec<u8>) -> Data {
        Data::Serialized(Arc::new(blob))
    }

    pub fn from_value(value: Value, layout: MoveTypeLayout) -> Data {
        Data::Cached(Arc::new(value), layout)
    }

    pub fn simple_serialize(&self) -> Option<Vec<u8>> {
        match self {
            Data::Serialized(blob) => Some(blob.as_ref().clone()),
            Data::Cached(value, layout) => value.simple_serialize(&layout),
        }
    }
}

/// A collection of changes to resources and modules for an account.
#[derive(Debug, Clone)]
pub struct AccountChangeSet {
    modules: BTreeMap<Identifier, Op<Vec<u8>>>,
    resources: BTreeMap<StructTag, Op<Data>>,
}

impl AccountChangeSet {
    pub fn from_modules_resources(
        modules: BTreeMap<Identifier, Op<Vec<u8>>>,
        resources: BTreeMap<StructTag, Op<Data>>,
    ) -> Self {
        Self { modules, resources }
    }

    pub fn new() -> Self {
        Self {
            modules: BTreeMap::new(),
            resources: BTreeMap::new(),
        }
    }

    pub fn add_module_op(&mut self, name: Identifier, op: Op<Vec<u8>>) -> Result<()> {
        match self.modules.entry(name) {
            Entry::Occupied(entry) => bail!("Module {} already exists", entry.key()),
            Entry::Vacant(entry) => {
                entry.insert(op);
            }
        }
        Ok(())
    }

    pub fn add_resource_op(&mut self, struct_tag: StructTag, op: Op<Data>) -> Result<()> {
        match self.resources.entry(struct_tag) {
            Entry::Occupied(entry) => bail!("Resource {} already exists", entry.key()),
            Entry::Vacant(entry) => {
                entry.insert(op);
            }
        }
        Ok(())
    }

    pub fn into_inner(
        self,
    ) -> (
        BTreeMap<Identifier, Op<Vec<u8>>>,
        BTreeMap<StructTag, Op<Data>>,
    ) {
        (self.modules, self.resources)
    }

    pub fn into_modules(self) -> BTreeMap<Identifier, Op<Vec<u8>>> {
        self.modules
    }

    pub fn into_resources(self) -> BTreeMap<StructTag, Op<Data>> {
        self.resources
    }

    pub fn modules(&self) -> &BTreeMap<Identifier, Op<Vec<u8>>> {
        &self.modules
    }

    pub fn resources(&self) -> &BTreeMap<StructTag, Op<Data>> {
        &self.resources
    }

    pub fn is_empty(&self) -> bool {
        self.modules.is_empty() && self.resources.is_empty()
    }
}

impl TryFrom<AccountChangeSet> for BlobAccountChangeSet {
    type Error = PartialVMError;

    fn try_from(account_change_set: AccountChangeSet) -> PartialVMResult<Self> {
        let (modules, resources) = account_change_set.into_inner();
        let mut resources_as_blobs = BTreeMap::new();
        for (struct_tag, op) in resources {
            let new_op = op.and_then(|data| {
                data.simple_serialize()
                    .ok_or_else(|| PartialVMError::new(StatusCode::INTERNAL_TYPE_ERROR))
            })?;
            resources_as_blobs.insert(struct_tag, new_op);
        }
        Ok(BlobAccountChangeSet::from_modules_resources(
            modules,
            resources_as_blobs,
        ))
    }
}

/// A collection of changes to the blockchain state.
#[derive(Debug, Clone)]
pub struct ChangeSet {
    accounts: BTreeMap<AccountAddress, AccountChangeSet>,
}

impl ChangeSet {
    pub fn new() -> Self {
        Self {
            accounts: BTreeMap::new(),
        }
    }

    pub fn add_account_changeset(
        &mut self,
        addr: AccountAddress,
        account_changeset: AccountChangeSet,
    ) -> Result<()> {
        match self.accounts.entry(addr) {
            Entry::Occupied(_) => bail!(
                "Failed to add account change set. Account {} already exists.",
                addr
            ),
            Entry::Vacant(entry) => {
                entry.insert(account_changeset);
            }
        }
        Ok(())
    }

    pub fn accounts(&self) -> &BTreeMap<AccountAddress, AccountChangeSet> {
        &self.accounts
    }

    pub fn into_inner(self) -> BTreeMap<AccountAddress, AccountChangeSet> {
        self.accounts
    }

    fn get_or_insert_account_changeset(&mut self, addr: AccountAddress) -> &mut AccountChangeSet {
        match self.accounts.entry(addr) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => entry.insert(AccountChangeSet::new()),
        }
    }

    pub fn add_module_op(&mut self, module_id: ModuleId, op: Op<Vec<u8>>) -> Result<()> {
        let account = self.get_or_insert_account_changeset(*module_id.address());
        account.add_module_op(module_id.name().to_owned(), op)
    }

    pub fn add_resource_op(
        &mut self,
        addr: AccountAddress,
        struct_tag: StructTag,
        op: Op<Data>,
    ) -> Result<()> {
        let account = self.get_or_insert_account_changeset(addr);
        account.add_resource_op(struct_tag, op)
    }
}

impl TryFrom<ChangeSet> for BlobChangeSet {
    type Error = VMError;

    fn try_from(change_set: ChangeSet) -> VMResult<Self> {
        let accounts = change_set.into_inner();
        let mut new_change_set = BlobChangeSet::new();
        for (addr, account_change_set) in accounts {
            new_change_set
                .add_account_changeset(
                    addr,
                    account_change_set
                        .try_into()
                        .map_err(|e: PartialVMError| e.finish(Location::Undefined))?,
                )
                .expect("accounts should be unique");
        }
        Ok(new_change_set)
    }
}
