use ckb_mock_tx_types::{MockCellDep, MockInfo, MockInput, MockTransaction};
use ckb_types::bytes::Bytes;
use ckb_types::core::{DepType, HeaderView};
use ckb_types::packed::{OutPoint, OutPointVec, Transaction};
use ckb_types::prelude::*;
use crate::types::error::{HostError, Result};

use crate::rpc_client::{RpcCall, RpcClient};

/// Convert a CKB transaction to a mock transaction.
///
/// Resolves every cell referenced by the transaction (inputs and cell_deps) via
/// [get_cell_by_out_point](rpc_client::get_cell_by_out_point)
/// Header deps are fetched via [get_header](rpc_client::get_header).
///
/// **Note:** Cellbase transactions (first tx in a block) have a null input OutPoint
/// that doesn't refer to a real cell. We skip resolving inputs for cellbase transactions,
/// consistent with how CKB's `resolve_transaction` handles them.
///
/// * `rpc_url` – CKB node RPC URL
/// * `tx` – CKB transaction
///    
pub fn tx_to_mock_tx(rpc_client: &RpcClient, tx: &Transaction) -> Result<MockTransaction> {
    let mut mock_info = MockInfo::default();
    let view = tx.clone().into_view();

    // Check if this is a cellbase transaction (first tx in block with null input)
    // Cellbase inputs have a null OutPoint (tx_hash = 0x00...00, index = 0xFFFFFFFF)
    // and should not be resolved since they don't refer to real cells.
    let is_cellbase = tx.is_cellbase();
    if is_cellbase {
        println!("Transaction is a cellbase (miner reward) - skipping input resolution");
    }

    // get  all input cells (unless cellbase) + all main cell_dep cells
    let n_inputs = if is_cellbase { 0 } else { view.inputs().len() };
    let n_cell_deps = view.cell_deps().len();
    let mut cell_calls: Vec<
        RpcCall<(ckb_types::packed::CellOutput, Bytes, Option<ckb_types::packed::Byte32>)>,
    > = Vec::with_capacity(n_inputs + n_cell_deps);
    // no need to resolve inputs for cellbase transactions
    if !is_cellbase {
        for input in view.inputs().into_iter() {
            cell_calls.push(rpc_client.get_cell_by_out_point(&input.previous_output()));
        }
    }
    for cell_dep in view.cell_deps_iter() {
        cell_calls.push(rpc_client.get_cell_by_out_point(&cell_dep.out_point()));
    }
    println!("Resolving {} cells (inputs + cell deps)...", cell_calls.len());
    let cell_results = rpc_client
        .exec_batch(cell_calls)
        .map_err(|e| HostError::CellResolutionError(format!("Failed to resolve cells: {}", e)))?;

    // build inputs from first n_inputs results (empty for cellbase)
    // DAO transactions need the block_hash where the input cell was created
    if !is_cellbase {
        for (i, input) in view.inputs().into_iter().enumerate() {
            let (output, data, block_hash) = &cell_results[i];
            mock_info.inputs.push(MockInput {
                input: input.clone(),
                output: output.clone(),
                data: data.clone(),
                header: block_hash.clone(),
            });
        }
    }

    // Build cell_deps: main results start at index n_inputs; handle DepGroup by batching sub out_points
    let main_results = &cell_results[n_inputs..];
    let mut sub_calls: Vec<
        RpcCall<(ckb_types::packed::CellOutput, Bytes, Option<ckb_types::packed::Byte32>)>,
    > = Vec::new();
    let mut sub_out_points_per_dep: Vec<Vec<ckb_types::packed::OutPoint>> =
        Vec::with_capacity(n_cell_deps);
    for (cell_dep, (_output, data, _block_hash)) in view.cell_deps_iter().zip(main_results.iter()) {
        if cell_dep.dep_type() == DepType::DepGroup.into() {
            let sub_out_points = OutPointVec::from_slice(data.as_ref())
                .map_err(|e| HostError::VerificationError(e.to_string()))?;
            let subs: Vec<OutPoint> = (0..sub_out_points.len())
                .map(|i| {
                    sub_out_points.get(i).ok_or_else(|| {
                        HostError::VerificationError(format!(
                            "Invalid DepGroup data: missing OutPoint at index {}",
                            i
                        ))
                    })
                })
                .collect::<Result<Vec<OutPoint>>>()?;
            for sub_op in &subs {
                sub_calls.push(rpc_client.get_cell_by_out_point(sub_op));
            }
            sub_out_points_per_dep.push(subs);
        } else {
            sub_out_points_per_dep.push(vec![]);
        }
    }

    if !sub_calls.is_empty() {
        println!("Resolving dep group sub-cells... {}", sub_calls.len());
        let sub_results = rpc_client
            .exec_batch(sub_calls)
            .map_err(|e| HostError::CellResolutionError(e.to_string()))?;
        let mut sub_result_idx = 0;
        for (dep_idx, (cell_dep, (output, data, block_hash))) in
            view.cell_deps_iter().zip(main_results.iter()).enumerate()
        {
            let subs = &sub_out_points_per_dep[dep_idx];
            // First add the original cell_dep (DepGroup container or Code)
            mock_info.cell_deps.push(MockCellDep {
                cell_dep: cell_dep.clone(),
                output: output.clone(),
                data: data.clone(),
                header: block_hash.clone(),
            });
            // Then add sub-cells AFTER the DepGroup container
            // If we flip this, we get a bug
            if cell_dep.dep_type() == DepType::DepGroup.into() {
                for sub_op in subs {
                    let (out, dat, sub_block_hash) = &sub_results[sub_result_idx];
                    mock_info.cell_deps.push(MockCellDep {
                        cell_dep: ckb_types::packed::CellDep::new_builder()
                            .out_point(sub_op.clone())
                            .dep_type(ckb_types::core::DepType::Code)
                            .build(),
                        output: out.clone(),
                        data: dat.clone(),
                        header: sub_block_hash.clone(),
                    });
                    sub_result_idx += 1;
                }
            }
        }
    } else {
        for (cell_dep, (output, data, block_hash)) in view.cell_deps_iter().zip(main_results.iter())
        {
            mock_info.cell_deps.push(MockCellDep {
                cell_dep: cell_dep.clone(),
                output: output.clone(),
                data: data.clone(),
                header: block_hash.clone(),
            });
        }
    }

    // get header dependencies
    let header_hashes: Vec<String> =
        view.header_deps_iter().map(|h| format!("0x{:x}", h)).collect();
    if !header_hashes.is_empty() {
        let header_calls: Vec<RpcCall<HeaderView>> =
            header_hashes.iter().map(|h| rpc_client.get_header(h)).collect();
        println!("Resolving header deps... {}", header_calls.len());
        let headers: Vec<HeaderView> = rpc_client
            .exec_batch(header_calls)
            .map_err(|e| HostError::HeaderResolutionError(e.to_string()))?;
        mock_info.header_deps = headers;
    }

    Ok(MockTransaction { mock_info, tx: tx.clone() })
}