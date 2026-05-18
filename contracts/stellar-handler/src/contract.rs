use soroban_sdk::{
    contract, contractimpl, contracttype, xdr::FromXdr, Address, Bytes, BytesN, Env, String,
};
use warpdrive_shared::interfaces::{
    handler::{Ed25519SignatureData, HandlerError, Verified, XlmEnvelope},
    verification::Ed25519VerificationClient,
};

use hodlers::HodlersClient;

use crate::storage;

#[contracttype]
pub struct HodlersPayload {
    pub delta: i128,
    pub trader: String,
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

    pub fn verify_xlm(
        env: Env,
        envelope_bytes: Bytes,
        sig_data: Ed25519SignatureData,
    ) -> Result<(), HandlerError> {
        let envelope = XlmEnvelope::from_xdr(&env, &envelope_bytes)
            .map_err(|_| HandlerError::InvalidEnvelope)?;
        let event_id = envelope.event_id.clone();

        if storage::is_event_seen(&env, &event_id) {
            return Err(HandlerError::EventAlreadySeen);
        }

        let verification_addr = storage::get_verification_contract(&env);
        match Ed25519VerificationClient::new(&env, &verification_addr).try_verify(
            &envelope_bytes,
            &sig_data.signatures,
            &sig_data.signers,
            &sig_data.reference_block,
        ) {
            Ok(Ok(())) => {}
            Ok(Err(_)) => return Err(HandlerError::UnknownVerificationError),
            Err(Ok(e)) => return Err(HandlerError::from(e)),
            Err(Err(_)) => return Err(HandlerError::OtherInvocationError),
        }

        let payload = HodlersPayload::from_xdr(&env, &envelope.payload)
            .map_err(|_| HandlerError::InvalidEnvelope)?;
        let trader = Address::from_string(&payload.trader);

        let hodlers_addr = storage::get_hodlers_contract(&env);
        HodlersClient::new(&env, &hodlers_addr)
            .try_add_points(&trader, &payload.delta)
            .map_err(|_| HandlerError::OtherInvocationError)?
            .map_err(|_| HandlerError::OtherInvocationError)?;

        storage::mark_event_seen(&env, &event_id);
        storage::extend_instance_ttl(&env);
        Verified::new(event_id).publish(&env);
        Ok(())
    }

    pub fn verification_contract(env: Env) -> Address {
        storage::get_verification_contract(&env)
    }

    pub fn hodlers_contract(env: Env) -> Address {
        storage::get_hodlers_contract(&env)
    }

    pub fn payload(_env: Env, _event_id: BytesN<20>) -> Option<Bytes> {
        None
    }
}
