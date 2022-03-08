#[cfg(test)]
mod tests {

    use cli::{
        errors::{SolKeriError, SolKeriResult},
        incp_event::{generate_inception_event, SolDidEvent},
        utils::{
            gen_pda_pk, get_did_pda_account, get_inception_datasize, instruction_from_transaction,
        },
    };

    use solana_client::rpc_client::RpcClient;
    use solana_did_method::{
        id,
        instruction::{InceptionDID, InitializeDidAccount, SDMInstruction},
    };
    use solana_rpc::rpc::JsonRpcConfig;
    use solana_sdk::{
        ed25519_instruction,
        instruction::{AccountMeta, Instruction},
        message::Message,
        pubkey::Pubkey,
        signature::{Keypair, Signature},
        signer::Signer,
        transaction::Transaction,
    };
    use solana_test_validator::{TestValidator, TestValidatorGenesis};
    use std::{path::PathBuf, str::FromStr, thread::sleep, time::Duration};

    /// Location/Name of ProgramTestGenesis ledger
    const LEDGER_PATH: &str = "./.ledger";
    /// Path to BPF program (*.so)
    const PROG_PATH: &str = "../target/deploy/";
    /// Program name from program/Cargo.toml
    const PROG_NAME: &str = "solana_did_method";

    /// Setup the test validator with predefined properties
    pub fn setup_validator() -> SolKeriResult<(TestValidator, Keypair, Pubkey)> {
        // Extend environment variable to include our program location
        std::env::set_var("BPF_OUT_DIR", PROG_PATH);
        // Instantiate the test validator
        let mut test_validator = TestValidatorGenesis::default();
        // Once instantiated, TestValidatorGenesis configuration functions follow
        // a builder pattern enabling chaining of settings function calls
        let (test_validator, kp) = test_validator
            // Set the ledger path and name
            // maps to `solana-test-validator --ledger <DIR>`
            .ledger_path(LEDGER_PATH)
            // Load our program. Ignored if reusing ledger
            // maps to `solana-test-validator --bpf-program <ADDRESS_OR_PATH BPF_PROGRAM.SO>`
            .add_program(PROG_NAME, id())
            // Start the test validator
            .rpc_config(JsonRpcConfig {
                enable_rpc_transaction_history: true,
                enable_cpi_and_log_storage: true,
                ..JsonRpcConfig::default_for_test()
            })
            .start();
        Ok((test_validator, kp, id()))
    }

    /// Convenience function to remove existing ledger before TestValidatorGenesis setup
    /// maps to `solana-test-validator ... --reset`
    pub fn clean_ledger_setup_validator() -> SolKeriResult<(TestValidator, Keypair, Pubkey)> {
        if PathBuf::from_str(LEDGER_PATH).unwrap().exists() {
            std::fs::remove_dir_all(LEDGER_PATH).unwrap();
        }
        setup_validator()
    }

    /// Submits a transaction with programs instruction
    fn submit_transaction(
        rpc_client: &RpcClient,
        wallet_signer: &dyn Signer,
        wallet_payer: &dyn Signer,
        instructions: Vec<Instruction>,
    ) -> SolKeriResult<Signature> {
        let mut transaction =
            Transaction::new_unsigned(Message::new(&instructions, Some(&wallet_payer.pubkey())));
        let recent_blockhash = rpc_client.get_latest_blockhash().unwrap();
        transaction
            .try_sign(&vec![wallet_signer], recent_blockhash)
            .unwrap();
        Ok(rpc_client
            .send_and_confirm_transaction(&transaction)
            .unwrap())
    }

    #[test]
    fn test_inception_two_controllers_pass() -> SolKeriResult<()> {
        // Setup faux keypair for management
        let mut keys = Vec::<Keypair>::new();
        keys.push(Keypair::new());
        keys.push(Keypair::new());
        let threshold = keys.len() as u64 - 1u64;
        let sol_did_incp = generate_inception_event(keys, threshold)?;

        // Now we want to create two (2) instructions:
        // 1. The ed25519 signature verification on the serialized message
        // 2. The inception of the DID for our program to the active keys (inception)

        // Spawn test validator node
        // The 'payer' will be our wallet for now

        println!("Starting local validator node");
        let (test_validator, payer, program_pk) = clean_ledger_setup_validator()?;

        // Get the RpcClient
        let connection = test_validator.get_rpc_client();
        // Create a PDA for our DID
        let digest_bytes = sol_did_incp.prefix_digest();
        let (pda_key, bump) = gen_pda_pk(digest_bytes, &program_pk);
        // If the account exists, we have a problem
        let check_pda_res = get_did_pda_account(&connection, &pda_key);
        if check_pda_res.is_ok() {
            return Err(SolKeriError::DIDExists(sol_did_incp.did_string()));
        }
        // Account does not exist
        println!(
            "Created PDA (pubkey) {:?} bump {} for `did:solana:{}`",
            pda_key,
            bump,
            sol_did_incp.prefix_as_string()
        );

        // Capture our programs log statements
        // ***************** UNCOMMENT NEXT LINE TO SEE LOGS
        // solana_logger::setup_with_default("solana_runtime::message=debug");

        // Instruction 1 - Add ledger signature verification on our inception data
        let serialized_incp = sol_did_incp.serialize()?;
        let privkey = ed25519_dalek::Keypair::from_bytes(&payer.to_bytes()).unwrap();
        let verify_instruction =
            ed25519_instruction::new_ed25519_instruction(&privkey, &serialized_incp);

        // Setup instruction payload to get size needed for account
        let mut prefix_bytes = [0u8; 32];
        prefix_bytes.copy_from_slice(sol_did_incp.prefix_digest());
        let did_account = InceptionDID {
            prefix: prefix_bytes,
            bump,
            keys: sol_did_incp.active_pubkeys(),
        };
        let space = get_inception_datasize(&did_account);
        let rent_exemption_amount = connection
            .get_minimum_balance_for_rent_exemption(space)
            .unwrap();

        let init = InitializeDidAccount {
            rent: 5 * rent_exemption_amount,
            storage: space as u64,
        };

        // Instruction 2 - Send the DID creation instruction

        println!("Submitting Solana-Keri Inception Instruction");

        let accounts = &[
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new(pda_key, false),
            AccountMeta::new(solana_sdk::system_program::id(), false),
        ];
        // Build instruction array and submit transaction
        let txn = submit_transaction(
            &connection,
            &payer, //payer,
            &payer,
            [
                verify_instruction,
                Instruction::new_with_borsh(
                    program_pk,
                    &SDMInstruction::SDMInception(init, did_account),
                    accounts.to_vec(),
                ),
            ]
            .to_vec(),
        );
        assert!(txn.is_ok());
        let signature = txn.unwrap();

        println!("Delay 20s for block completion. Should use websocket monitoring");
        sleep(Duration::from_secs(20));
        println!(
            "{:?}",
            instruction_from_transaction(&connection, &signature)
        );
        Ok(())
    }
}
