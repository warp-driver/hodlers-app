use alloc::format;
use alloc::vec::Vec as StdVec;
use soroban_sdk::testutils::{Address as _, Ledger};
use soroban_sdk::xdr::ToXdr;
use soroban_sdk::{Address, Bytes, BytesN, Env, String, Vec};
use warpdrive_shared::interfaces::handler::XlmEnvelope;
use warpdrive_shared::testutils::{
    ed25519_pubkey, ed25519_sign_envelope, make_ed25519_key, Ed25519SigningKey,
};

use ed25519_security::Ed25519Security;
use ed25519_verification::Ed25519Verification;
use hodlers::Hodlers;

use crate::{Ed25519SignatureData, HandlerError, HodlersPayload, StellarHandler, StellarHandlerClient};

const REGISTRATION_BLOCK: u32 = 10;
const CURRENT_BLOCK: u32 = 100;

struct TestSetup<'a> {
    env: Env,
    handler: StellarHandlerClient<'a>,
    hodlers: hodlers::HodlersClient<'a>,
    keys: StdVec<(Ed25519SigningKey, BytesN<32>)>,
}

fn build_envelope_bytes(env: &Env, event_id: u8, trader: &Address, delta: i128) -> Bytes {
    let payload = HodlersPayload {
        delta,
        trader: String::from_str(env, &format!("{}", trader.to_string())),
    };
    let payload_bytes = payload.to_xdr(env);

    let mut event_id_bytes = [0u8; 20];
    event_id_bytes[19] = event_id;

    let envelope = XlmEnvelope {
        event_id: BytesN::from_array(env, &event_id_bytes),
        ordering: BytesN::from_array(env, &[0u8; 12]),
        payload: payload_bytes,
    };
    envelope.to_xdr(env)
}

fn setup(num_signers: usize, threshold_num: u64, threshold_denom: u64) -> TestSetup<'static> {
    let env = Env::default();
    let admin = Address::generate(&env);

    env.ledger().set_sequence_number(REGISTRATION_BLOCK);

    let security_id = env.register(Ed25519Security, (&admin, threshold_num, threshold_denom));
    let security = ed25519_security::Ed25519SecurityClient::new(&env, &security_id);

    let mut keys: StdVec<(Ed25519SigningKey, BytesN<32>)> = StdVec::new();
    for i in 0..num_signers {
        let sk = make_ed25519_key((i as u8) + 1);
        let pk = ed25519_pubkey(&env, &sk);
        env.mock_all_auths();
        security.add_signer(&pk, &100);
        keys.push((sk, pk));
    }

    let verification_id = env.register(Ed25519Verification, (&admin, &security_id));
    let hodlers_id = env.register(Hodlers, ());
    let handler_id = env.register(StellarHandler, (&verification_id, &hodlers_id));

    env.ledger().set_sequence_number(CURRENT_BLOCK);

    TestSetup {
        env: env.clone(),
        handler: StellarHandlerClient::new(&env, &handler_id),
        hodlers: hodlers::HodlersClient::new(&env, &hodlers_id),
        keys,
    }
}

fn sign(env: &Env, envelope: &Bytes, keys: &[(Ed25519SigningKey, BytesN<32>)]) -> Ed25519SignatureData {
    let envelope_vec = envelope.to_alloc_vec();
    let mut signatures: Vec<BytesN<64>> = Vec::new(env);
    let mut signers: Vec<BytesN<32>> = Vec::new(env);
    for (sk, pk) in keys {
        let raw = ed25519_sign_envelope(sk, &envelope_vec);
        signatures.push_back(BytesN::from_array(env, &raw));
        signers.push_back(pk.clone());
    }
    Ed25519SignatureData {
        signatures,
        signers,
        reference_block: REGISTRATION_BLOCK,
    }
}

#[test]
fn happy_path_verifies_and_credits_hodlers() {
    let s = setup(2, 55, 100);
    let trader = Address::generate(&s.env);
    let envelope = build_envelope_bytes(&s.env, 1, &trader, 42);
    let sig = sign(&s.env, &envelope, &s.keys);

    s.handler.verify_xlm(&envelope, &sig);

    assert_eq!(s.hodlers.points_of(&trader), 42);
}

#[test]
fn replay_is_rejected() {
    let s = setup(2, 55, 100);
    let trader = Address::generate(&s.env);
    let envelope = build_envelope_bytes(&s.env, 1, &trader, 10);
    let sig = sign(&s.env, &envelope, &s.keys);

    s.handler.verify_xlm(&envelope, &sig);
    let result = s.handler.try_verify_xlm(&envelope, &sig);
    assert_eq!(result, Err(Ok(HandlerError::EventAlreadySeen)));
}

#[test]
fn insufficient_quorum_rejected() {
    let s = setup(2, 55, 100);
    let trader = Address::generate(&s.env);
    let envelope = build_envelope_bytes(&s.env, 2, &trader, 10);
    let sig = sign(&s.env, &envelope, &s.keys[..1]);

    let result = s.handler.try_verify_xlm(&envelope, &sig);
    assert_eq!(result, Err(Ok(HandlerError::InsufficientWeight)));
}

#[test]
fn malformed_envelope_rejected() {
    let s = setup(2, 55, 100);
    // Valid XDR but the wrong shape — `from_xdr::<XlmEnvelope>` returns
    // ConversionError, which we surface as InvalidEnvelope. (Bytes that are
    // not valid XDR at all panic in the host before our error mapping
    // runs, so we deliberately test only the in-shape failure here.)
    let envelope = 7u32.to_xdr(&s.env);
    let sig = Ed25519SignatureData {
        signatures: Vec::new(&s.env),
        signers: Vec::new(&s.env),
        reference_block: REGISTRATION_BLOCK,
    };

    let result = s.handler.try_verify_xlm(&envelope, &sig);
    assert_eq!(result, Err(Ok(HandlerError::InvalidEnvelope)));
}
