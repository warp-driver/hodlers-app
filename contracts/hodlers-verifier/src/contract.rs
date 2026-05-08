use soroban_sdk::{contract, contractimpl};

#[contract]
pub struct HodlersVerifier;

#[contractimpl]
impl HodlersVerifier {
    // init(operator_set, hodlers_address, secp256k1_helper),
    // verify_eth(envelope, sig_data) -> calls Hodlers::add_points as admin.
}
