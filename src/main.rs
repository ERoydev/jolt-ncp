use clap::Parser;
use jolt_testing::{HostArgs, HostExecutor};
use tracing::info;


pub fn main() {
    tracing_subscriber::fmt::init();

    let args = HostArgs::parse();
    let tx_hash = args.tx_hash.as_deref().unwrap_or_else(|| {
        eprintln!("Error: --tx-hash is required");
        std::process::exit(2);
    });
    let config = args.as_config();

    let executor = HostExecutor::new(config).unwrap_or_else(|e| {
        eprintln!("Failed to create executor: {}", e);
        std::process::exit(1);
    });

    let target_dir = "/tmp/jolt-guest-targets";
    let mut program = guest::compile_entrypoint(target_dir);

    if let Err(e) = executor.run(tx_hash, program) {
        eprintln!("Execution failed: {}", e);
        std::process::exit(1);
    }

    
    // let mut program = guest::entrypoint(input_bytes);

    // let target_dir = "/tmp/jolt-guest-targets";
    // let mut program = guest::compile_fib(target_dir);

    // let shared_preprocessing = guest::preprocess_shared_fib(&mut program);

    // let prover_preprocessing = guest::preprocess_prover_fib(shared_preprocessing.clone());
    // let verifier_setup = prover_preprocessing.generators.to_verifier_setup();
    // let verifier_preprocessing =
    //     guest::preprocess_verifier_fib(shared_preprocessing, verifier_setup);

    // let prove_fib = guest::build_prover_fib(program, prover_preprocessing);
    // let verify_fib = guest::build_verifier_fib(verifier_preprocessing);

    // let (output, proof, io_device) = prove_fib(50);
    // let is_valid = verify_fib(50, output, io_device.panic, proof);

    // info!("output: {output}");
    // info!("valid: {is_valid}");
}
