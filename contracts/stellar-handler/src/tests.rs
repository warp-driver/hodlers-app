use alloc::format;
use alloc::vec::Vec as StdVec;
use alloy_sol_types::SolValue;
use soroban_sdk::testutils::{Address as _, Ledger};
use soroban_sdk::{Address, Bytes, BytesN, Env, Vec};
use warpdrive_shared::testutils::{
    make_secp256k1_key, secp256k1_pubkey, secp256k1_sign_envelope, SecpSigningKey,
};

use hodlers::Hodlers;
use secp256k1_security::Secp256k1Security;
use secp256k1_verification::Secp256k1Verification;

use crate::envelope::{Envelope, HodlersPayload};
use crate::{HandlerError, SignatureData, StellarHandler, StellarHandlerClient};

const REGISTRATION_BLOCK: u32 = 10;
const CURRENT_BLOCK: u32 = 100;

struct TestSetup<'a> {
    env: Env,
    handler: StellarHandlerClient<'a>,
    hodlers: hodlers::HodlersClient<'a>,
    keys: StdVec<(SecpSigningKey, BytesN<33>)>,
}

fn build_envelope_bytes(env: &Env, event_id: u8, trader: &Address, delta: i128) -> Bytes {
    let payload = HodlersPayload {
        trader: format!("{}", trader.to_string()),
        delta,
    };
    let payload_bytes = payload.abi_encode();

    let mut event_id_bytes = [0u8; 20];
    event_id_bytes[19] = event_id;
    let envelope = Envelope {
        eventId: alloy_primitives::FixedBytes(event_id_bytes),
        ordering: alloy_primitives::FixedBytes([0u8; 12]),
        payload: payload_bytes.into(),
    };
    Bytes::from_slice(env, &envelope.abi_encode())
}

fn setup(num_signers: usize, threshold_num: u64, threshold_denom: u64) -> TestSetup<'static> {
    let env = Env::default();
    let admin = Address::generate(&env);

    env.ledger().set_sequence_number(REGISTRATION_BLOCK);

    let security_id = env.register(Secp256k1Security, (&admin, threshold_num, threshold_denom));
    let security =
        secp256k1_security::Secp256k1SecurityClient::new(&env, &security_id);

    let mut keys: StdVec<(SecpSigningKey, BytesN<33>)> = StdVec::new();
    for i in 0..num_signers {
        let sk = make_secp256k1_key((i as u8) + 1);
        let pk = secp256k1_pubkey(&env, &sk);
        env.mock_all_auths();
        security.add_signer(&pk, &100);
        keys.push((sk, pk));
    }

    let verification_id = env.register(Secp256k1Verification, (&admin, &security_id));
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

fn sign(env: &Env, envelope: &Bytes, keys: &[(SecpSigningKey, BytesN<33>)]) -> SignatureData {
    let envelope_vec = envelope.to_alloc_vec();
    let mut signatures: Vec<BytesN<65>> = Vec::new(env);
    let mut signers: Vec<BytesN<33>> = Vec::new(env);
    for (sk, pk) in keys {
        let raw = secp256k1_sign_envelope(sk, &envelope_vec);
        signatures.push_back(BytesN::from_array(env, &raw));
        signers.push_back(pk.clone());
    }
    SignatureData {
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

    s.handler.verify(&envelope, &sig);

    assert_eq!(s.hodlers.points_of(&trader), 42);
}

#[test]
fn replay_is_rejected() {
    let s = setup(2, 55, 100);
    let trader = Address::generate(&s.env);
    let envelope = build_envelope_bytes(&s.env, 1, &trader, 10);
    let sig = sign(&s.env, &envelope, &s.keys);

    s.handler.verify(&envelope, &sig);
    let result = s.handler.try_verify(&envelope, &sig);
    assert_eq!(result, Err(Ok(HandlerError::EventAlreadySeen)));
}

#[test]
fn insufficient_quorum_rejected() {
    let s = setup(2, 55, 100);
    let trader = Address::generate(&s.env);
    let envelope = build_envelope_bytes(&s.env, 2, &trader, 10);
    // Only one signer when threshold needs both.
    let sig = sign(&s.env, &envelope, &s.keys[..1]);

    let result = s.handler.try_verify(&envelope, &sig);
    assert_eq!(result, Err(Ok(HandlerError::VerificationFailed)));
}

#[test]
fn malformed_envelope_rejected() {
    let s = setup(2, 55, 100);
    let envelope = Bytes::from_slice(&s.env, &[0xde, 0xad, 0xbe, 0xef]);
    let sig = SignatureData {
        signatures: Vec::new(&s.env),
        signers: Vec::new(&s.env),
        reference_block: REGISTRATION_BLOCK,
    };

    let result = s.handler.try_verify(&envelope, &sig);
    assert_eq!(result, Err(Ok(HandlerError::EnvelopeDecodeFailed)));
}
