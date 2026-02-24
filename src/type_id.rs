//! Type ID script support.
//!
//! CKB's Type ID is a built-in consensus mechanism with no on-chain binary.
//! The CKB node special-cases scripts with code_hash == TYPE_ID_CODE_HASH
//! and runs `TypeIdSystemScript::verify()` instead of loading a binary.
//!
//! For transaction validation, we have two options:
//! 1. Skip Type ID script groups and do a simple constraint check
//! 2. Simulate actual traces for Type ID scripts by injecting the type-id ELF binary and overriding the code_hash

use ckb_chain_spec::consensus::TYPE_ID_CODE_HASH;
use ckb_script::ScriptGroup;
use ckb_types::core::ScriptHashType;
use ckb_types::packed::Script;
use ckb_types::prelude::*;

/// Check if a script is a Type ID script (built-in, no binary).
///
/// A Type ID script has:
/// - code_hash == TYPE_ID_CODE_HASH (the magic constant 0x00...545950455f4944)
/// - hash_type == Type
pub fn is_type_id_script(script: &Script) -> bool {
    script.code_hash() == TYPE_ID_CODE_HASH.pack()
        && script.hash_type() == ScriptHashType::Type.into()
}

/// Check if a script group is for Type ID (should be skipped for trace collection).
pub fn is_type_id_group(group: &ScriptGroup) -> bool {
    is_type_id_script(&group.script)
}

/// Placeholder for Type ID injection - currently a no-op.
#[allow(dead_code)]
pub fn inject_type_id_if_needed(_mock_tx: &mut ckb_mock_tx_types::MockTransaction) {
    // No-op: Type ID groups are filtered out in accumulate_traces
}