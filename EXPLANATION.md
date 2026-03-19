

COde hase 3 phases: `preprocessing` -> `proving` -> `verifying`

1. preprocess_shared_entrypoint(&mut program)     
    1. Parses the guest ELF — extracts the RISC-V bytecode from your compiled guest                                                                                                             
    2. Builds the instruction lookup tables — precomputed tables for all the instruction decompositions (ADD, SUB, AND, OR, SLT, etc. broken into byte-sized lookups)
    3. Generates the Dory PCS parameters — the dory_38.urs setup you saw taking ~5 min (one-time, cached after)                                                                                 
    4. Computes commitment generators — the elliptic curve points needed for committing to polynomials later  

    It's called "shared" because both prover and verifier need these tables and parameters. 
    The prover needs them to construct the proof, the verifier needs them to check that the lookups are valid.                  
                                                                                                                                                                                                
    Dory PCS (Polynomial Commitment Scheme) is the cryptographic primitive Jolt uses to commit to large polynomials. max_log_n=38 means it's generating parameters for polynomials of size up to
    2^38 (~274 billion entries). This involves computing massive multi-scalar multiplications on elliptic curves — pure math, fully parallelized across all 24 cores. That's your ~5 minutes of
    100% CPU. It saves to dory_38.urs so next run it loads from disk instantly.                                                                                                                
                                                                                    
2. preprocess_prover_entrypoint / preprocess_verifier_entrypoint

### preprocess_prover_entrypoint
Takes the full shared data and keeps everything — all the lookup tables, all the Dory generators, all commitment parameters. The prover needs the complete set because it has to:           
  - Commit to every witness polynomial                            
  - Produce opening proofs for the polynomial commitments                                                                                                                                     
  - Construct the Sumcheck proofs over the lookup arguments       
                                                                                                                                                                                              
  This is the heavy side. The prover preprocessing is large (potentially gigabytes) because it holds the full generator arrays for Dory at max_log_n=38.        

### preprocess_verifier_entrypoint
Takes the shared data but replaces the full generators with to_verifier_setup() — a compressed/minimal version. The verifier doesn't need to compute commitments, it only needs to check    
  them. So it keeps:                                              
  - The lookup table definitions (to verify lookups are correct)                                                                                                                              
  - A small verifier key derived from the generators (enough to verify polynomial openings, not produce them)                                                                                 
                                                                                                             
  This is tiny in comparison.                                                                                                                                                                 
                                                                                                                                                                                              
  Why the split?                                                                                                                                                                              
                                                                                                                                                                                              
  In a real deployment, the prover runs on a beefy machine (like your 24-core VM). The verifier could run on-chain or on a lightweight client. You don't want to ship gigabytes of prover data
   to the verifier. The split ensures the verifier only gets what it needs — making verification fast and cheap while proving stays on the heavy machine.
                                                                                                                                                                                              
  In your code, both happen on the same machine, but the separation is architecturally correct for when you eventually deploy the verifier separately.                                                                              
                                                                                    
1. build_prover_entrypoint / build_verifier_entrypoint    
 Returns closures (callable functions) that capture the preprocessed data:                                                                                                                   
  - prove(input) → runs the guest, generates a proof              
  - verify(input, output, proof) → checks the proof is valid                                                                                                                                  
                                                                  
  No heavy computation here — it's just wiring things together.                                                                                                                               
                                                                                                                                                                                              
  Why so heavy before proving?                                                                                                                                                                
                                                                                                                                                                                              
  The Dory setup dominates. It's a one-time cost — structured reference string (SRS) generation for the polynomial commitment scheme. Think of it like generating RSA keys but for ZK proofs. 
  2^38 is massive because your max_trace_length = 1_073_741_824 (2^30) requires commitments over very large polynomials.
                                                                                       
START Loading the Setup: 15:33
END Loading the Setup: 15:36

Start Proof Generation: 15:36
Proof Stage 1 baseline - > 16:02, memory usage: 99.24 GB
End Proof Generation: 

# Running Notes:

It setups my Jolt first time i run it
```
2026-03-19T12:36:36.709538Z DEBUG JoltProverPreprocessing::gen: dory_pcs: Setup file not found, will generate new one                                                                         
2026-03-19T12:36:36.709563Z  INFO JoltProverPreprocessing::gen: dory_pcs: Setup not found on disk, generating new setup for max_log_n=38                                                      
2026-03-19T12:41:21.435212Z  INFO JoltProverPreprocessing::gen: dory_pcs::setup: Saving setup to /home/emil_roydev/.cache/dory/dory_38.urs                                                    
2026-03-19T12:41:25.203062Z  INFO JoltProverPreprocessing::gen: dory_pcs::setup: Successfully saved setup to disk  
```

Second time i run Jolt i get this, so i think it loads the setup
```
2026-03-19T13:30:29.280083Z  INFO JoltProverPreprocessing::gen: dory_pcs::setup: Looking for saved setup at /home/emil_roydev/.cache/dory/dory_38.urs
2026-03-19T13:34:33.791350Z  INFO JoltProverPreprocessing::gen: dory_pcs::setup: Loaded setup for max_log_n=38
```