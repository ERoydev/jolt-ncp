
use crate::config::HostConfig;
use crate::rpc_client::{DEFAULT_RPC_URL_MAINNET, DEFAULT_RPC_URL_TESTNET};
use clap::Parser;

/// arguments for the host binary
#[derive(Parser, Debug)]
#[command(name = "ckb-host")]
#[command(about = "Fetch CKB transaction by its hash and replay it", long_about = None)]
pub struct HostArgs {
    #[arg(long, value_name = "TX_HASH")]
    pub tx_hash: Option<String>,
    #[arg(long, default_value = "mainnet", value_name = "NETWORK")]
    pub network: String,
    #[arg(long, value_name = "RPC_URL")]
    pub rpc_url: Option<String>,
}

impl HostArgs {
    pub fn as_config(&self) -> HostConfig {
        HostConfig {
            network: self.network.clone(),
            rpc_url: self.effective_rpc_url(),
        }
    }

    pub fn effective_rpc_url(&self) -> String {
        self.rpc_url.clone().unwrap_or_else(|| Self::default_rpc_url_for_network(&self.network))
    }

    fn default_rpc_url_for_network(network: &str) -> String {
        match network.to_lowercase().as_str() {
            "mainnet" => DEFAULT_RPC_URL_MAINNET.to_string(),
            "testnet" => DEFAULT_RPC_URL_TESTNET.to_string(),
            _ => DEFAULT_RPC_URL_MAINNET.to_string(),
        }
    }
}