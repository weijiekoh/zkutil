extern crate bellman_ce;
extern crate clap;
extern crate zkutil;

use bellman_ce::pairing::bn256::Bn256;
use clap::Clap;
use std::fs;
use std::fs::File;
use std::path::Path;
use std::str;
use zkutil::circom_circuit::{
    create_rng, create_verifier_sol_file, generate_random_parameters, groth16_verify, load_inputs_json_file, load_params_file,
    load_proof_json_file, plonk_verify, proof_to_json_file, prove as prove2, proving_key_json_file, r1cs_from_bin_file,
    r1cs_from_json_file, verification_key_json_file, witness_from_json_file, CircomCircuit, R1CS,
};
use zkutil::io;
use zkutil::proofsys_type::ProofSystem;

/// A tool to work with SNARK circuits generated by circom
#[derive(Clap)]
struct Opts {
    #[clap(subcommand)]
    command: SubCommand,
}

#[derive(Clap)]
enum SubCommand {
    /// Generate a SNARK proof
    Prove(ProveOpts),
    /// Verify a SNARK proof
    Verify(VerifyOpts),
    /// Generate trusted setup parameters
    Setup(SetupOpts),
    /// Generate verifier smart contract
    GenerateVerifier(GenerateVerifierOpts),
    /// Export proving and verifying keys compatible with snarkjs/websnark
    ExportKeys(ExportKeysOpts),
}

/// A subcommand for generating a SNARK proof
#[derive(Clap)]
struct ProveOpts {
    /// Plonk universal setup key file
    #[clap(short = "u", long = "key_setup", default_value = "setup.key")]
    setup: String,
    /// Circuit R1CS or JSON file [default: circuit.r1cs|circuit.json]
    #[clap(short = "c", long = "circuit")]
    circuit: Option<String>,
    /// Witness JSON file
    #[clap(short = "w", long = "witness", default_value = "witness.json")]
    witness: String,
    /// Output file for proof BIN
    #[clap(short = "p", long = "proof", default_value = "proof.bin")]
    proof: String,
    // TODO:
    // /// Output file for public inputs JSON
    // #[clap(short = "o", long = "public", default_value = "public.json")]
    // public: String,
    /// Proof system
    #[clap(short = "s", long = "proof_system", default_value = "groth16")]
    proof_system: ProofSystem,
}

/// A subcommand for verifying a SNARK proof
#[derive(Clap)]
struct VerifyOpts {
    /// Proof BIN file
    #[clap(short = "p", long = "proof", default_value = "proof.bin")]
    proof: String,
    /// Verification key file
    #[clap(short = "v", long = "verification_key", default_value = "vk.bin")]
    vk: String,
    /// Proof system
    #[clap(short = "s", long = "proof_system", default_value = "plonk")]
    proof_system: ProofSystem,
}

/// A subcommand for generating a trusted setup parameters
#[derive(Clap)]
struct SetupOpts {
    /// Snark trusted setup parameters file
    #[clap(short = "p", long = "params", default_value = "params.bin")]
    params: String,
    /// Circuit R1CS or JSON file [default: circuit.r1cs|circuit.json]
    #[clap(short = "c", long = "circuit")]
    circuit: Option<String>,
    /// Proof system
    #[clap(short = "s", long = "proof_system", default_value = "groth16")]
    proof_system: ProofSystem,
}

/// A subcommand for generating a Solidity verifier smart contract
#[derive(Clap)]
struct GenerateVerifierOpts {
    /// Snark trusted setup parameters file
    #[clap(short = "p", long = "params", default_value = "params.bin")]
    params: String,
    /// Output smart contract name
    #[clap(short = "v", long = "verifier", default_value = "Verifier.sol")]
    verifier: String,
    /// Proof system
    #[clap(short = "s", long = "proof_system", default_value = "groth16")]
    proof_system: ProofSystem,
}

/// A subcommand for exporting proving and verifying keys compatible with snarkjs/websnark
#[derive(Clap)]
struct ExportKeysOpts {
    /// Snark trusted setup parameters file
    #[clap(short = "p", long = "params", default_value = "params.bin")]
    params: String,
    /// Circuit R1CS or JSON file [default: circuit.r1cs|circuit.json]
    #[clap(short = "c", long = "circuit")]
    circuit: Option<String>,
    /// Output proving key file
    #[clap(short = "r", long = "pk", default_value = "proving_key.json")]
    pk: String,
    /// Output verifying key file
    #[clap(short = "v", long = "vk", default_value = "verification_key.json")]
    vk: String,
    /// Proof system
    #[clap(short = "s", long = "proof_system", default_value = "groth16")]
    proof_system: ProofSystem,
}

fn main() {
    let opts: Opts = Opts::parse();
    match opts.command {
        SubCommand::Prove(o) => {
            println!("Running with proof system: {:?}", o.proof_system);
            prove(o);
        }
        SubCommand::Verify(o) => {
            println!("Running with proof system: {:?}", o.proof_system);
            verify(o);
        }
        SubCommand::Setup(o) => {
            println!("Running with proof system: {:?}", o.proof_system);
            setup(o);
        }
        SubCommand::GenerateVerifier(o) => {
            println!("Running with proof system: {:?}", o.proof_system);
            generate_verifier(o);
        }
        SubCommand::ExportKeys(o) => {
            println!("Running with proof system: {:?}", o.proof_system);
            export_keys(o);
        }
    }
}

fn load_r1cs(filename: &str) -> R1CS<Bn256> {
    if filename.ends_with("json") {
        r1cs_from_json_file(filename)
    } else {
        let (r1cs, _wire_mapping) = r1cs_from_bin_file(filename).unwrap();
        r1cs
    }
}

fn resolve_circuit_file(filename: Option<String>) -> String {
    match filename {
        Some(s) => s,
        None => {
            if Path::new("circuit.r1cs").exists() || !Path::new("circuit.json").exists() {
                "circuit.r1cs".to_string()
            } else {
                "circuit.json".to_string()
            }
        }
    }
}

fn prove(opts: ProveOpts) {
    if opts.proof_system == ProofSystem::Plonk {
        unimplemented!();
    }

    let rng = create_rng();
    let params = load_params_file(&opts.params);
    let circuit_file = resolve_circuit_file(opts.circuit);
    println!("Loading circuit from {}...", circuit_file);
    let circuit = CircomCircuit {
        r1cs: load_r1cs(&circuit_file),
        witness: Some(witness_from_json_file::<Bn256>(&opts.witness)),
        wire_mapping: None,
        aux_offset: opts.proof_system.aux_offset(),
    };
    println!("Proving...");
    let proof = prove2(circuit.clone(), &params, rng).unwrap();
    proof_to_json_file(&proof, &opts.proof).unwrap();
    fs::write(&opts.public, circuit.get_public_inputs_json().as_bytes()).unwrap();
    println!("Saved {} and {}", opts.proof, opts.public);
}

fn verify(opts: VerifyOpts) {
    let correct: bool;
    match opts.proof_system {
        ProofSystem::Plonk => {
            let vk = io::load_verification_key::<Bn256>(&opts.vk);
            let proof = io::load_proof::<Bn256>(&opts.proof);
            correct = plonk_verify(&vk, &proof).unwrap();
        }
        _ => {
            panic!("Deprecated");
        }
    }

    if correct {
        println!("Proof is correct");
    } else {
        println!("Proof is invalid!");
        std::process::exit(400);
    }
}

fn setup(opts: SetupOpts) {
    if opts.proof_system == ProofSystem::Plonk {
        unimplemented!();
    }

    let circuit_file = resolve_circuit_file(opts.circuit);
    println!("Loading circuit from {}...", circuit_file);
    let rng = create_rng();
    let circuit = CircomCircuit {
        r1cs: load_r1cs(&circuit_file),
        witness: None,
        wire_mapping: None,
        aux_offset: opts.proof_system.aux_offset(),
    };
    println!("Generating trusted setup parameters...");
    let params = generate_random_parameters(circuit, rng).unwrap();
    println!("Writing to file...");
    let writer = File::create(&opts.params).unwrap();
    params.write(writer).unwrap();
    println!("Saved parameters to {}", opts.params);
}

fn generate_verifier(opts: GenerateVerifierOpts) {
    if opts.proof_system == ProofSystem::Plonk {
        unimplemented!();
    }

    let params = load_params_file(&opts.params);
    create_verifier_sol_file(&params, &opts.verifier).unwrap();
    println!("Created {}", opts.verifier);
}

fn export_keys(opts: ExportKeysOpts) {
    if opts.proof_system == ProofSystem::Plonk {
        unimplemented!();
    }

    println!("Exporting {}...", opts.params);
    let params = load_params_file(&opts.params);
    let circuit_file = resolve_circuit_file(opts.circuit);
    let circuit = CircomCircuit {
        r1cs: load_r1cs(&circuit_file),
        witness: None,
        wire_mapping: None,
        aux_offset: opts.proof_system.aux_offset(),
    };
    proving_key_json_file(&params, circuit, &opts.pk).unwrap();
    verification_key_json_file(&params, &opts.vk).unwrap();
    println!("Created {} and {}.", opts.pk, opts.vk);
}
