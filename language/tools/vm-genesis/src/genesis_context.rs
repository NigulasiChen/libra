// Copyright (c) The Libra Core Contributors
// SPDX-License-Identifier: Apache-2.0

#![forbid(unsafe_code)]

use anyhow::Result;
use bytecode_verifier::VerifiedModule;
use libra_state_view::StateView;
use libra_types::{
    access_path::AccessPath,
    account_address::AccountAddress,
    account_config,
    language_storage::{ModuleId, TypeTag},
    transaction::{Script, TransactionArgument},
};
use move_core_types::{
    gas_schedule::{CostTable, GasAlgebra, GasUnits},
    identifier::Identifier,
};
use move_vm_runtime::MoveVM;
use move_vm_state::{data_cache::BlockDataCache, execution_context::TransactionExecutionContext};
use move_vm_types::values::Value;
use std::collections::HashMap;
use vm::{gas_schedule::zero_cost_schedule, transaction_metadata::TransactionMetadata};

/// A context that holds state for generating the genesis write set
pub(crate) struct GenesisContext<'a> {
    vm: MoveVM,
    gas_schedule: CostTable,
    interpreter_context: TransactionExecutionContext<'a>,
    txn_data: TransactionMetadata,
}

impl<'a> GenesisContext<'a> {
    pub fn new(data_cache: &'a BlockDataCache<'a>, stdlib_modules: &[VerifiedModule]) -> Self {
        let vm = MoveVM::new();
        let mut interpreter_context =
            TransactionExecutionContext::new(GasUnits::new(100_000_000), data_cache);
        for module in stdlib_modules {
            vm.cache_module(module.clone(), &mut interpreter_context)
                .expect("Failure loading stdlib");
        }

        Self {
            vm,
            gas_schedule: zero_cost_schedule(),
            interpreter_context,
            txn_data: TransactionMetadata::default(),
        }
    }

    fn module(name: &str) -> ModuleId {
        ModuleId::new(
            account_config::CORE_CODE_ADDRESS,
            Identifier::new(name).unwrap(),
        )
    }

    fn name(name: &str) -> Identifier {
        Identifier::new(name).unwrap()
    }

    /// Convert the transaction arguments into move values.
    fn convert_txn_args(args: &[TransactionArgument]) -> Vec<Value> {
        args.iter()
            .map(|arg| match arg {
                TransactionArgument::U64(i) => Value::u64(*i),
                TransactionArgument::Address(a) => Value::address(*a),
                TransactionArgument::Bool(b) => Value::bool(*b),
                TransactionArgument::U8Vector(v) => Value::vector_u8(v.clone()),
            })
            .collect()
    }

    pub fn exec(
        &mut self,
        module_name: &str,
        function_name: &str,
        type_params: Vec<TypeTag>,
        args: Vec<Value>,
    ) {
        self.vm
            .execute_function(
                &Self::module(module_name),
                &Self::name(function_name),
                &self.gas_schedule,
                &mut self.interpreter_context,
                &self.txn_data,
                type_params,
                args,
            )
            .unwrap()
    }

    pub fn exec_script(&mut self, script: &Script) {
        self.vm
            .execute_script(
                script.code().to_vec(),
                &self.gas_schedule,
                &mut self.interpreter_context,
                &self.txn_data,
                script.ty_args().to_vec(),
                Self::convert_txn_args(script.args()),
            )
            .unwrap()
    }

    pub fn set_sender(&mut self, sender: AccountAddress) {
        self.txn_data.sender = sender;
    }

    pub fn into_interpreter_context(self) -> TransactionExecutionContext<'a> {
        self.interpreter_context
    }
}

// `StateView` has no data given we are creating the genesis
pub(crate) struct GenesisStateView {
    data: HashMap<AccessPath, Vec<u8>>,
}

impl GenesisStateView {
    pub(crate) fn new() -> Self {
        Self {
            data: HashMap::new(),
        }
    }

    pub(crate) fn add_module(&mut self, module_id: &ModuleId, module: &VerifiedModule) {
        let access_path = AccessPath::from(module_id);
        let mut blob = vec![];
        module
            .serialize(&mut blob)
            .expect("serializing stdlib must work");
        self.data.insert(access_path, blob);
    }
}

impl StateView for GenesisStateView {
    fn get(&self, access_path: &AccessPath) -> Result<Option<Vec<u8>>> {
        Ok(self.data.get(access_path).cloned())
    }

    fn multi_get(&self, _access_paths: &[AccessPath]) -> Result<Vec<Option<Vec<u8>>>> {
        unimplemented!()
    }

    fn is_genesis(&self) -> bool {
        true
    }
}
