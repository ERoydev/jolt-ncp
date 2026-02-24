//! CKB JSON-RPC client for fetching transactions and cells.
//!
//! Return types:
//! - [get_transaction][]: [ckb_types::packed::Transaction]
//! - [get_header][]: [ckb_types::core::HeaderView]
//! - [get_live_cell][]: [ckb_jsonrpc_types::CellWithStatus]
//! - [get_cell_by_out_point][]: (CellOutput, Bytes) from live cell or creating transaction
//!
//! **Design:** Functions take `rpc_url` so call sites can switch nodes or use config. An
//! alternative is a struct `RpcClient { base_url, client }` with instance methods: that would
//! allow reusing one [reqwest::blocking::Client] (connection pooling), one timeout/config,
//! and easier testing/mocking. For a small number of calls in one binary, free functions
//! are fine; consider a struct if you add more RPC methods or need to share the client.

#[allow(dead_code)]
pub const DEFAULT_RPC_TIMEOUT_SECONDS: u64 = 30;
#[allow(dead_code)]
pub const DEFAULT_RPC_URL_MAINNET: &str = "https://mainnet.ckb.dev";
#[allow(dead_code)]
pub const DEFAULT_RPC_URL_TESTNET: &str = "https://testnet.ckb.dev";

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use ckb_jsonrpc_types::{
    BlockResponse, CellWithStatus, Either, HeaderView as JsonHeaderView, OutPoint, ResponseFormat,
    TransactionWithStatusResponse,
};
use ckb_types::bytes::Bytes;
use ckb_types::core::{BlockView, Cycle};
use ckb_types::{
    core::HeaderView,
    packed::{Byte32, CellOutput, OutPoint as PackedOutPoint, Transaction},
    prelude::Pack,
};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize, de::DeserializeOwned};

#[derive(Serialize)]
struct JsonRpcRequest {
    jsonrpc: &'static str,
    method: &'static str,
    params: Vec<serde_json::Value>,
    pub id: u64,
    #[serde(skip_serializing)]
    pub output_index: Option<u32>,
}

#[derive(Deserialize)]
#[serde(bound(deserialize = "T: DeserializeOwned"))]
struct JsonRpcResponse<T> {
    #[serde(default)]
    pub id: u64,
    result: Option<T>,
    error: Option<JsonRpcError>,
}

#[derive(Deserialize)]
struct JsonRpcError {
    message: String,
}

type RpcCallResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;
pub struct RpcCall<T> {
    request: JsonRpcRequest,
    decode: fn(JsonRpcResponse<serde_json::Value>, Option<u32>) -> RpcCallResult<T>,
}

fn decode_transaction(
    body: JsonRpcResponse<serde_json::Value>,
    _output_index: Option<u32>,
) -> Result<Transaction, Box<dyn std::error::Error + Send + Sync>> {
    if let Some(e) = body.error {
        return Err(e.message.into());
    }
    let tws: TransactionWithStatusResponse =
        serde_json::from_value(body.result.ok_or("RPC returned null (transaction not found)")?)?;
    let tx_view =
        tws.transaction.ok_or("transaction field is null (not found or verbosity too low)")?;
    match &tx_view.inner {
        Either::Left(view) => Ok(view.inner.clone().into()),
        Either::Right(_) => Err("hex format not supported here; use verbosity 2".into()),
    }
}

fn decode_live_cell(
    body: JsonRpcResponse<serde_json::Value>,
    _output_index: Option<u32>,
) -> Result<CellWithStatus, Box<dyn std::error::Error + Send + Sync>> {
    if let Some(e) = body.error {
        return Err(e.message.into());
    }
    let v = body.result.ok_or("RPC returned null (cell not found)")?;
    let cws: CellWithStatus = serde_json::from_value(v)?;
    Ok(cws)
}

fn decode_header(
    body: JsonRpcResponse<serde_json::Value>,
    _output_index: Option<u32>,
) -> Result<HeaderView, Box<dyn std::error::Error + Send + Sync>> {
    if let Some(e) = body.error {
        return Err(e.message.into());
    }
    let v = body.result.ok_or("header not found")?;
    let response_format: Option<ResponseFormat<JsonHeaderView>> = serde_json::from_value(v)?;
    let response_format = response_format.ok_or("header not found")?;
    match &response_format.inner {
        Either::Left(json_view) => Ok(json_view.clone().into()),
        Either::Right(_) => Err("hex format not supported here; use verbosity 2".into()),
    }
}

fn decode_block(
    body: JsonRpcResponse<serde_json::Value>,
    _output_index: Option<u32>,
) -> Result<(BlockView, Vec<Cycle>), Box<dyn std::error::Error + Send + Sync>> {
    if let Some(e) = body.error {
        return Err(e.message.into());
    }
    let v = body.result.ok_or("header not found")?;
    let block_response: BlockResponse = serde_json::from_value(v)?;
    match block_response {
        BlockResponse::Regular(block_response_format) => match block_response_format.inner {
            Either::Left(json_view) => Ok((json_view.clone().into(), vec![])),
            Either::Right(_) => Err("hex format not supported here; use verbosity 2".into()),
        },
        BlockResponse::WithCycles(block_with_cycles_response) => {
            let block_response_format = block_with_cycles_response.block;
            let cycles_opt = block_with_cycles_response.cycles;
            let cycles = cycles_opt.unwrap_or_default().into_iter().map(|c| c.into()).collect();
            match block_response_format.inner {
                Either::Left(json_view) => Ok((json_view.clone().into(), cycles)),
                Either::Right(_) => Err("hex format not supported here; use verbosity 2".into()),
            }
        }
    }
}

/// Decode cell output, data, and the block hash where the creating transaction was included.
/// The block hash is needed for DAO calculations and other scripts that use header information.
fn decode_cell_by_out_point(
    body: JsonRpcResponse<serde_json::Value>,
    output_index: Option<u32>,
) -> Result<(CellOutput, Bytes, Option<Byte32>), Box<dyn std::error::Error + Send + Sync>> {
    let index = output_index.ok_or("get_cell_by_out_point requires output_index")? as usize;

    if let Some(e) = body.error {
        return Err(e.message.into());
    }
    let tws: TransactionWithStatusResponse =
        serde_json::from_value(body.result.ok_or("RPC returned null (transaction not found)")?)?;

    let block_hash: Option<Byte32> = tws.tx_status.block_hash.map(|h| h.pack());

    let tx_view =
        tws.transaction.ok_or("transaction field is null (not found or verbosity too low)")?;
    let tx: Transaction = match &tx_view.inner {
        Either::Left(view) => view.inner.clone().into(),
        Either::Right(_) => return Err("hex format not supported here; use verbosity 2".into()),
    };

    let raw = tx.raw();
    let output = raw
        .outputs()
        .into_iter()
        .nth(index)
        .ok_or_else(|| format!("output index {} out of bounds", index))?;
    let data = raw
        .outputs_data()
        .get(index)
        .map(|b| Bytes::from(b.raw_data().to_vec()))
        .ok_or_else(|| format!("outputs_data index {} out of bounds", index))?;
    Ok((output, data, block_hash))
}

impl<T> RpcCall<T> {
    pub fn exec(self, client: &RpcClient) -> Result<T, Box<dyn std::error::Error + Send + Sync>> {
        let res = client.client.post(&client.rpc_url).json(&self.request).send()?;
        let raw: JsonRpcResponse<serde_json::Value> = res.json()?;
        (self.decode)(raw, self.request.output_index)
    }
}

/// Extract (CellOutput, Bytes) for the given out_point from a transaction (the creating tx).
#[allow(dead_code)]
pub fn cell_from_tx(
    tx: &Transaction,
    out_point: &PackedOutPoint,
) -> Result<(CellOutput, Bytes), Box<dyn std::error::Error + Send + Sync>> {
    let tx_hash_hex = format!("0x{:x}", out_point.tx_hash());
    let raw = tx.raw();
    let index = ckb_types::prelude::Unpack::<u32>::unpack(&out_point.index()) as usize;
    let output =
        raw.outputs().into_iter().nth(index).ok_or_else(|| {
            format!("output index {} out of bounds for tx {}", index, tx_hash_hex)
        })?;
    let data =
        raw.outputs_data().get(index).map(|b| Bytes::from(b.raw_data().to_vec())).ok_or_else(
            || format!("outputs_data index {} out of bounds for tx {}", index, tx_hash_hex),
        )?;
    Ok((output, data))
}

pub struct RpcClient {
    client: Client,
    rpc_url: String,
    next_id: AtomicU64,
}

impl RpcClient {
    fn next_request_id(&self) -> u64 {
        self.next_id.fetch_add(1, Ordering::SeqCst)
    }

    #[allow(dead_code)]
    pub fn default() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        Self::new(DEFAULT_RPC_URL_MAINNET, DEFAULT_RPC_TIMEOUT_SECONDS)
    }

    #[allow(dead_code)]
    pub fn new(
        rpc_url: &str,
        timeout: u64,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let rpc_url = format!("{}/", rpc_url.trim_end_matches('/')).to_string();
        let client = Client::builder().timeout(Duration::from_secs(timeout)).build()?;
        Ok(Self { client, rpc_url, next_id: AtomicU64::new(0) })
    }

    pub fn exec_batch<T>(
        &self,
        calls: Vec<RpcCall<T>>,
    ) -> Result<Vec<T>, Box<dyn std::error::Error + Send + Sync>> {
        if calls.is_empty() {
            return Ok(vec![]);
        }

        let mut requests = Vec::with_capacity(calls.len());
        let mut decodes = Vec::with_capacity(calls.len());
        let mut output_indices = Vec::with_capacity(calls.len());
        let mut assigned_ids = Vec::with_capacity(calls.len());
        for call in calls {
            assigned_ids.push(call.request.id);
            output_indices.push(call.request.output_index);
            requests.push(call.request);
            decodes.push(call.decode);
        }

        let res = self.client.post(&self.rpc_url).json(&requests).send()?;
        let responses: Vec<JsonRpcResponse<serde_json::Value>> = res.json()?;

        let mut by_id = std::collections::HashMap::new();
        for r in responses {
            by_id.insert(r.id, r);
        }

        let mut out = Vec::with_capacity(decodes.len());
        for (i, decode) in decodes.into_iter().enumerate() {
            let id = assigned_ids[i];
            let resp = by_id.remove(&id).ok_or("missing RPC response in batch")?;
            out.push(decode(resp, output_indices[i])?);
        }

        Ok(out)
    }

    /// Fetch a transaction from the chain by hash.
    ///
    /// RPC returns [TransactionWithStatusResponse]; we extract the transaction and convert to
    /// [Transaction] (packed). Requires verbosity 2 (JSON); hex format is not supported.
    ///
    /// * `tx_hash` - Transaction hash as 0x-prefixed hex string
    pub fn get_transaction(&self, tx_hash: &str) -> RpcCall<Transaction> {
        RpcCall {
            request: JsonRpcRequest {
                jsonrpc: "2.0",
                method: "get_transaction",
                params: vec![
                    serde_json::Value::String(tx_hash.to_string()),
                    serde_json::Value::String("0x2".to_string()),
                    serde_json::Value::Null,
                ],
                id: self.next_request_id(),
                output_index: None,
            },
            decode: decode_transaction,
        }
    }

    /// Fetch a live cell by out point.
    ///
    /// RPC returns [CellWithStatus]. We return it as-is; callers use `.cell` and `.status` as needed.
    #[allow(dead_code)]
    pub fn get_live_cell(
        &self,
        out_point: &OutPoint,
        with_data: bool,
        include_tx_pool: bool,
    ) -> Result<RpcCall<CellWithStatus>, Box<dyn std::error::Error + Send + Sync>> {
        let params = vec![
            serde_json::to_value(out_point).map_err(|e| e.to_string())?,
            serde_json::Value::Bool(with_data),
            serde_json::Value::Bool(include_tx_pool),
        ];
        Ok(RpcCall {
            request: JsonRpcRequest {
                jsonrpc: "2.0",
                method: "get_live_cell",
                params,
                id: self.next_request_id(),
                output_index: None,
            },
            decode: decode_live_cell,
        })
    }

    /// Resolve a cell by out_point: fetches the creating transaction and returns (output, data, block_hash).
    /// The block_hash is the block where the creating transaction was included (needed for DAO).
    pub fn get_cell_by_out_point(
        &self,
        out_point: &PackedOutPoint,
    ) -> RpcCall<(CellOutput, Bytes, Option<Byte32>)> {
        let output_index = ckb_types::prelude::Unpack::<u32>::unpack(&out_point.index());
        RpcCall {
            request: JsonRpcRequest {
                jsonrpc: "2.0",
                method: "get_transaction",
                params: vec![
                    serde_json::Value::String(format!("0x{:x}", out_point.tx_hash())),
                    serde_json::Value::String("0x2".to_string()),
                    serde_json::Value::Null,
                ],
                id: self.next_request_id(),
                output_index: Some(output_index),
            },
            decode: decode_cell_by_out_point,
        }
    }

    /// Fetch a block header by block hash.
    ///
    /// RPC returns [Option<ResponseFormat<HeaderView>>] (null if not found; format is JSON or hex).
    /// We require JSON format (verbosity 0x1) and convert to [HeaderView] (core).
    /// Verbosity 2 is not available for this RPC method.
    ///
    /// * `block_hash` - Block hash as 0x-prefixed hex string
    pub fn get_header(&self, block_hash: &str) -> RpcCall<HeaderView> {
        RpcCall {
            request: JsonRpcRequest {
                jsonrpc: "2.0",
                method: "get_header",
                params: vec![
                    serde_json::Value::String(block_hash.to_string()),
                    serde_json::Value::String("0x1".to_string()),
                ],
                id: self.next_request_id(),
                output_index: None,
            },
            decode: decode_header,
        }
    }

    /// Fetch a block by block hash.
    ///
    /// RPC returns [Option<BlockResponse>] (null if not found; format is JSON or hex).
    /// We require JSON format (verbosity 2) and convert to [(BlockView, Vec<Cycle>)] .
    ///
    /// * `block_hash` - Block hash as 0x-prefixed hex string
    /// * `with_cycles` - Whether to include cycles in the response
    #[allow(dead_code)]
    pub fn get_block(
        &self,
        block_hash: &str,
        with_cycles: bool,
    ) -> RpcCall<(BlockView, Vec<Cycle>)> {
        RpcCall {
            request: JsonRpcRequest {
                jsonrpc: "2.0",
                method: "get_block",
                params: vec![
                    serde_json::Value::String(block_hash.to_string()),
                    serde_json::Value::String("0x2".to_string()),
                    serde_json::Value::Bool(with_cycles),
                ],
                id: self.next_request_id(),
                output_index: None,
            },
            decode: decode_block,
        }
    }

    /// Fetch a block by block number.
    ///
    /// RPC returns [Option<BlockResponse>] (null if not found; format is JSON or hex).
    /// We require JSON format (verbosity 2) and convert to [(BlockView, Vec<Cycle>)] .
    ///
    /// * `block_number` - Block number (sent as 0x-prefixed hex to the RPC)
    /// * `with_cycles` - Whether to request cycles (actual cycles only if the node returns them)
    #[allow(dead_code)]
    pub fn get_block_by_number(
        &self,
        block_number: u64,
        with_cycles: bool,
    ) -> RpcCall<(BlockView, Vec<Cycle>)> {
        RpcCall {
            request: JsonRpcRequest {
                jsonrpc: "2.0",
                method: "get_block_by_number",
                params: vec![
                    serde_json::Value::String(format!("0x{:x}", block_number)),
                    serde_json::Value::String("0x2".to_string()),
                    serde_json::Value::Bool(with_cycles),
                ],
                id: self.next_request_id(),
                output_index: None,
            },
            decode: decode_block,
        }
    }
}