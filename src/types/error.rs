use derive_more::{Display, From};

// Use this Result type throughout the SP1 project instead of the standard library's Result.
// This ensures all errors are handled as Sp1Error.
pub type Result<T> = core::result::Result<T, HostError>;

#[derive(Debug, Display, From)]
#[display("{self:?}")]
pub enum HostError {
    // -- RPC errors
    #[display("RPC client creation failed: {_0}")]
    RpcClientCreation(String),

    #[display("Failed to fetch transaction: {_0}")]
    TransactionFetch(String),

    #[display("Failed to convert transaction to mock: {_0}")]
    MockTransactionConversion(String),

    // -- Verifier errors
    #[display("Failed to create verifier context: {_0}")]
    VerifierContextCreation(String),

    #[display("Failed to create verifier resource: {_0}")]
    VerifierResource(String),

    #[display("OutPoint error: {_0}")]
    VerifierOutpointError(String),

    #[display("Failed to get program ELF for script")]
    ProgramElfNotFound,

    #[display("Script error: {_0}")]
    ScriptError(String),

    // -- Types conversion errors
    #[display("Failed to convert to TransactionProofContext: {_0}")]
    TransactionProofContextConversion(String),

    // -- Tx cell resolution errors
    #[display("Failed to resolve cells: {_0}")]
    CellResolutionError(String),

    #[display("VerificationError: {_0}")]
    VerificationError(String),

    #[display("HeaderResolutionError: {_0}")]
    HeaderResolutionError(String),

    // -- SP1 execution errors
    #[display("SP1 execution failed: {_0}")]
    Sp1Execution(String),

    #[display("Failed to deserialize guest output")]
    GuestOutputDeserialization,

    // -- Report errors
    #[display("Failed to write execution report: {_0}")]
    ReportWrite(String),

    // -- Externals
    #[from]
    Io(std::io::Error),
}

impl std::error::Error for HostError {}