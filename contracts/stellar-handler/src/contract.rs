use soroban_sdk::{
    contract, contractimpl, contracttype, Address, Bytes, BytesN, Env, String, Vec,
};
use warpdrive_shared::interfaces::verification::Secp256k1VerificationClient;

use hodlers::HodlersClient;

use crate::envelope::{Envelope, HodlersPayload};
use crate::error::HandlerError;
use crate::storage;

#[contracttype]
pub struct SignatureData {
    pub signatures: Vec<BytesN<65>>,
    pub signers: Vec<BytesN<33>>,
    pub reference_block: u32,
}

#[contract]
pub struct StellarHandler;

#[contractimpl]
impl StellarHandler {
    pub fn __constructor(env: Env, verification_contract: Address, hodlers_contract: Address) {
        storage::set_verification_contract(&env, &verification_contract);
        storage::set_hodlers_contract(&env, &hodlers_contract);
        storage::set_version(&env, &String::from_str(&env, env!("CARGO_PKG_VERSION")));
        storage::extend_instance_ttl(&env);
    }

    pub fn verify(
        env: Env,
        envelope_bytes: Bytes,
        sig_data: SignatureData,
    ) -> Result<(), HandlerError> {
        let envelope = Envelope::abi_decode_from(&envelope_bytes)
            .ok_or(HandlerError::EnvelopeDecodeFailed)?;
        let event_id = BytesN::<20>::from_array(&env, &envelope.eventId.0);

        if storage::is_event_seen(&env, &event_id) {
            return Err(HandlerError::EventAlreadySeen);
        }

        let verification_addr = storage::get_verification_contract(&env);
        Secp256k1VerificationClient::new(&env, &verification_addr)
            .try_verify(
                &envelope_bytes,
                &sig_data.signatures,
                &sig_data.signers,
                &sig_data.reference_block,
            )
            .map_err(|_| HandlerError::VerificationFailed)?
            .map_err(|_| HandlerError::VerificationFailed)?;

        let payload = HodlersPayload::abi_decode_from(&envelope.payload)
            .ok_or(HandlerError::PayloadDecodeFailed)?;
        let trader_strkey = String::from_str(&env, &payload.trader);
        let trader = Address::from_string(&trader_strkey);
        let delta: i128 = payload.delta;

        let hodlers_addr = storage::get_hodlers_contract(&env);
        HodlersClient::new(&env, &hodlers_addr)
            .try_add_points(&trader, &delta)
            .map_err(|_| HandlerError::HodlersCallFailed)?
            .map_err(|_| HandlerError::HodlersCallFailed)?;

        storage::mark_event_seen(&env, &event_id);
        storage::extend_instance_ttl(&env);

        Ok(())
    }

    pub fn verification_contract(env: Env) -> Address {
        storage::get_verification_contract(&env)
    }

    pub fn hodlers_contract(env: Env) -> Address {
        storage::get_hodlers_contract(&env)
    }
}
