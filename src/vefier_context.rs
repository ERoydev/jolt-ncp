//! Context loading for the verifier.
// ! It is modelled after the ckb-standalone-debugger/ckb-debugger/src/main.rs file.

use std::collections::HashSet;
use std::rc::Rc;
use std::sync::Arc;

use ckb_chain_spec::consensus::{Consensus, ConsensusBuilder};
use ckb_mock_tx_types::{MockTransaction, Resource};
use ckb_script::Scheduler;
use ckb_script::types::{Machine, SgData};
use ckb_script::{
    ScriptGroup, ScriptGroupType, ScriptVersion, TransactionScriptsVerifier, TxVerifyEnv,
};
use ckb_types::bytes::Bytes;
use ckb_types::core::cell::ResolvedTransaction;
use ckb_types::core::cell::resolve_transaction;
use ckb_types::core::{HeaderView, ScriptHashType, hardfork};
use ckb_types::packed::{Byte32, OutPoint, Script};
use ckb_types::prelude::Entity;
use ckb_types::prelude::Pack;
use crate::types::error::{HostError, Result};

#[derive(Clone)]
pub struct VerifierGroupContext {
    pub verifier_resolve_transaction: Arc<ResolvedTransaction>,
    pub verifier_resource: Resource,
    pub verifier_consensus: Arc<Consensus>,
    pub verifier_tx_env: Arc<TxVerifyEnv>,
    /// Pre-built lookup: code_hash -> ELF bytes (for Data hash types)
    /// and type_script_hash -> ELF bytes (for Type hash type)
    cell_dep_data_hash_to_elf: std::collections::HashMap<Byte32, Bytes>,
}

type VerifierGroupMetadata = (ScriptGroupType, Byte32, ScriptGroup, ScriptVersion);

impl VerifierGroupContext {
    /// Build shared verifier data and enumerate all script groups (by type and hash),
    /// with the script version for each group from consensus (epoch + script hash_type).
    /// Returns `(base, groups)` where each group is [VerifierGroupMetadata].
    ///
    /// *`epoch_for_version_selection`* is used to build [TxVerifyEnv]; use an epoch where all
    pub fn from_mock_transaction_with_groups(
        mock_transaction: &MockTransaction,
        epoch_for_version_selection: ScriptVersion,
    ) -> Result<(Self, Vec<VerifierGroupMetadata>)> {
        let verifier_resource = Resource::from_mock_tx(mock_transaction)
            .map_err(|e| HostError::VerifierResource(e.to_string()))?;
        let verifier_resolve_transaction = resolve_transaction(
            mock_transaction.core_transaction(),
            &mut HashSet::new(),
            &verifier_resource,
            &verifier_resource,
        )
        .map_err(|e| HostError::VerifierOutpointError(e.to_string()))?;

        // Pre-build ELF lookup map: code_hash/type_hash -> ELF bytes
        // This avoids re-hashing cell data on every get_program_elf call
        let mut elf_by_hash = std::collections::HashMap::new();
        for cell_dep in &mock_transaction.mock_info.cell_deps {
            let elf_data = cell_dep.data.clone();
            // Index by data hash (for Data/Data1/Data2 hash types)
            let data_hash: Byte32 = ckb_hash::blake2b_256(&cell_dep.data).pack();
            elf_by_hash.insert(data_hash, elf_data.clone());
            // Index by type script hash (for Type hash type)
            if let Some(type_script) = cell_dep.output.type_().to_opt() {
                let type_hash = type_script.calc_script_hash();
                elf_by_hash.insert(type_hash, elf_data);
            }
        }

        let verifier_hardforks = hardfork::HardForks {
            ckb2021: hardfork::CKB2021::new_mirana().as_builder().rfc_0032(20).build().map_err(
                |e| HostError::VerifierContextCreation(format!("CKB2021 build failed: {:?}", e)),
            )?,

            ckb2023: hardfork::CKB2023::new_mirana().as_builder().rfc_0049(30).build().map_err(
                |e| HostError::VerifierContextCreation(format!("CKB2021 build failed: {:?}", e)),
            )?,
        };
        let verifier_consensus =
            Arc::new(ConsensusBuilder::default().hardfork_switch(verifier_hardforks).build());
        let verifier_epoch = match epoch_for_version_selection {
            ScriptVersion::V0 => ckb_types::core::EpochNumberWithFraction::new(15, 0, 1),
            ScriptVersion::V1 => ckb_types::core::EpochNumberWithFraction::new(25, 0, 1),
            ScriptVersion::V2 => ckb_types::core::EpochNumberWithFraction::new(35, 0, 1),
        };
        let verifier_header_view =
            HeaderView::new_advanced_builder().epoch(verifier_epoch.pack()).build();
        let verifier_tx_env = Arc::new(TxVerifyEnv::new_commit(&verifier_header_view));

        let verifier_resolve_transaction = Arc::new(verifier_resolve_transaction);
        let verifier = TransactionScriptsVerifier::new(
            verifier_resolve_transaction.clone(),
            verifier_resource.clone(),
            verifier_consensus.clone(),
            verifier_tx_env.clone(),
        );
        let groups: Vec<(ScriptGroupType, Byte32, ScriptGroup, ScriptVersion)> = verifier
            .groups_with_type()
            .map(|(t, h, g)| {
                let version = verifier
                    .select_version(&g.script)
                    .map_err(|e| HostError::ScriptError(e.to_string()))?;
                Ok((t, h.clone(), g.clone(), version))
            })
            .collect::<Result<Vec<_>>>()?;

        Ok((
            Self {
                verifier_resolve_transaction,
                verifier_resource,
                verifier_consensus,
                verifier_tx_env,
                cell_dep_data_hash_to_elf: elf_by_hash,
            },
            groups,
        ))
    }

    /// Get the program ELF for a script using the pre-built lookup map.
    /// O(1) lookup instead of O(n) search with re-hashing.
    pub fn get_program_elf(&self, verifier_script: &Script) -> Result<Bytes> {
        let code_hash = verifier_script.code_hash();
        self.cell_dep_data_hash_to_elf.get(&code_hash).cloned().ok_or(HostError::ProgramElfNotFound)
    }
}

type VerifierScheduler =
    Scheduler<Resource, Arc<dyn Fn(&Byte32, &str) + Send + Sync + 'static>, Machine>;
#[derive(Clone)]
pub struct VerifierContext {
    pub verifier_resolve_transaction: ResolvedTransaction,
    pub verifier_resource: Resource,
    #[allow(dead_code)]
    pub verifier_program_elf: Bytes,
    pub verifier_consensus: Arc<Consensus>,
    pub verifier_tx_env: Arc<TxVerifyEnv>,
    pub verifier_script_group: ScriptGroup,
    #[allow(dead_code)]
    pub verifier_scheduler: Rc<VerifierScheduler>,
    #[allow(dead_code)]
    pub verifier_sg_data: SgData<Resource>,
}

impl VerifierContext {
    #[allow(dead_code)]
    pub fn from_mock_transaction(
        mock_transaction: &MockTransaction,
        verifier_script: &Script,
        script_version: ScriptVersion,
        script_group_type: ScriptGroupType,
        script_hash: &Byte32,
    ) -> Result<Self> {
        let verifier_resource = Resource::from_mock_tx(mock_transaction)
            .map_err(|e| HostError::VerifierResource(e.to_string()))?;
        let mut verifier_resolve_transaction = resolve_transaction(
            mock_transaction.core_transaction(),
            &mut HashSet::new(),
            &verifier_resource,
            &verifier_resource,
        )
        .map_err(|e| HostError::VerifierOutpointError(e.to_string()))?;

        let verifier_script_out_point = || -> Result<OutPoint> {
            match ScriptHashType::try_from(verifier_script.hash_type())
                .map_err(|e| HostError::ScriptError(format!("invalid script hash type: {:?}", e)))?
            {
                ScriptHashType::Data | ScriptHashType::Data1 | ScriptHashType::Data2 => {
                    for e in &mock_transaction.mock_info.cell_deps {
                        if ckb_hash::blake2b_256(&e.data) == verifier_script.code_hash().as_slice()
                        {
                            return Ok(e.cell_dep.out_point());
                        }
                    }
                    unreachable!()
                }
                ScriptHashType::Type => {
                    for e in &mock_transaction.mock_info.cell_deps {
                        if let Some(kype) = e.output.type_().to_opt() {
                            if kype.calc_script_hash() == verifier_script.code_hash() {
                                return Ok(e.cell_dep.out_point());
                            }
                        }
                    }
                    unreachable!()
                }
                _ => unreachable!(),
            }
        }()?;

        let verifier_program_elf = {
            let mut found = None;
            for e in &mut verifier_resolve_transaction.resolved_cell_deps {
                if e.out_point == verifier_script_out_point {
                    found = Some(e.mem_cell_data.clone());
                    break;
                }
            }
            found.flatten().ok_or(HostError::ProgramElfNotFound)?
        };

        let verifier_hardforks = hardfork::HardForks {
            ckb2021: hardfork::CKB2021::new_mirana().as_builder().rfc_0032(20).build().map_err(
                |e| HostError::VerifierContextCreation(format!("CKB2021 build failed: {:?}", e)),
            )?,
            ckb2023: hardfork::CKB2023::new_mirana().as_builder().rfc_0049(30).build().map_err(
                |e| HostError::VerifierContextCreation(format!("CKB2023 build failed: {:?}", e)),
            )?,
        };
        let verifier_consensus =
            Arc::new(ConsensusBuilder::default().hardfork_switch(verifier_hardforks).build());
        let verifier_epoch = match script_version {
            ScriptVersion::V0 => ckb_types::core::EpochNumberWithFraction::new(15, 0, 1),
            ScriptVersion::V1 => ckb_types::core::EpochNumberWithFraction::new(25, 0, 1),
            ScriptVersion::V2 => ckb_types::core::EpochNumberWithFraction::new(35, 0, 1),
        };
        let verifier_header_view =
            HeaderView::new_advanced_builder().epoch(verifier_epoch.pack()).build();
        let verifier_tx_env = Arc::new(TxVerifyEnv::new_commit(&verifier_header_view));
        let verifier = TransactionScriptsVerifier::new(
            Arc::new(verifier_resolve_transaction.clone()),
            verifier_resource.clone(),
            verifier_consensus.clone(),
            verifier_tx_env.clone(),
        );
        let verifier_script_group = verifier
            .find_script_group(script_group_type, script_hash)
            .ok_or(HostError::ScriptError("script group not found".to_string()))?
            .clone();

        let verifier_scheduler = Rc::new(
            verifier
                .create_scheduler(&verifier_script_group)
                .map_err(|e| HostError::ScriptError(e.to_string()))?,
        );
        let verifier_sg_data = verifier_scheduler.sg_data().clone();

        Ok(Self {
            verifier_resolve_transaction,
            verifier_resource,
            verifier_program_elf,
            verifier_consensus,
            verifier_tx_env,
            verifier_script_group,
            verifier_scheduler,
            verifier_sg_data,
        })
    }
}