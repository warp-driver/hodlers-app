use crate::{Secp256k1Verification, Secp256k1VerificationClient};
use soroban_sdk::{Address, BytesN, Env, testutils::Address as _};
use warpdrive_secp256k1_security::Secp256k1Security;

mod contract_wasm {
    soroban_sdk::contractimport!(
        file = "../../target/wasm32v1-none/release/warpdrive_secp256k1_verification.wasm"
    );
}

pub fn deploy_contract<'a>(env: &Env) -> (Secp256k1VerificationClient<'a>, Address) {
    let admin = Address::generate(env);

    // Deploy security contract first
    let security_id = env.register(Secp256k1Security, (&admin, 2u64, 3u64));

    let contract_id = env.register(Secp256k1Verification, (&admin, &security_id));
    let client = Secp256k1VerificationClient::new(env, &contract_id);
    (client, admin)
}

pub fn install_contract_wasm(env: &Env) -> BytesN<32> {
    env.deployer().upload_contract_wasm(contract_wasm::WASM)
}
