extern crate std;

use crate::{Secp256k1Verification, Secp256k1VerificationClient, VerifyError};
use soroban_sdk::{Bytes, BytesN, Env, Vec, testutils::Address as _, testutils::Ledger as _};
use warpdrive_secp256k1_security::{Secp256k1Security, Secp256k1SecurityClient};
use warpdrive_shared::testutils::{
    CompressedSecpPubKey, SecpSigningKey, make_secp256k1_key, secp256k1_pubkey,
    secp256k1_sign_envelope,
};

fn setup_contracts(env: &Env) -> (Secp256k1VerificationClient<'_>, Secp256k1SecurityClient<'_>) {
    let admin = soroban_sdk::Address::generate(env);

    let key1 = make_secp256k1_key(1);
    let key2 = make_secp256k1_key(2);

    // Set ledger to 100 so checkpoints are recorded at this sequence
    env.ledger().with_mut(|li| li.sequence_number = 100);

    // Deploy security contract with threshold 55/100
    let security_id = env.register(Secp256k1Security, (&admin, 55u64, 100u64));
    let security = Secp256k1SecurityClient::new(env, &security_id);

    // key1 weight 100, key2 weight 200
    security
        .mock_all_auths()
        .add_signer(&secp256k1_pubkey(env, &key1), &100);
    security
        .mock_all_auths()
        .add_signer(&secp256k1_pubkey(env, &key2), &200);

    // Deploy verification contract referencing the security contract
    let verification_id = env.register(Secp256k1Verification, (&admin, &security_id));
    let verification = Secp256k1VerificationClient::new(env, &verification_id);

    // Advance ledger past the checkpoint
    env.ledger().with_mut(|li| li.sequence_number = 200);

    (verification, security)
}

#[test]
fn test_required_weight() {
    let env = Env::default();
    let (verification, _security) = setup_contracts(&env);

    // total_weight = 100 + 200 = 300
    // required = 300 * 55 / 100 = 165
    assert_eq!(verification.required_weight(), 165);
}

#[test]
fn test_signer_weight_existing() {
    let env = Env::default();
    let (verification, _security) = setup_contracts(&env);

    let key1 = make_secp256k1_key(1);
    let key2 = make_secp256k1_key(2);

    assert_eq!(
        verification.signer_weight(&secp256k1_pubkey(&env, &key1)),
        100
    );
    assert_eq!(
        verification.signer_weight(&secp256k1_pubkey(&env, &key2)),
        200
    );
}

#[test]
fn test_signer_weight_missing() {
    let env = Env::default();
    let (verification, _security) = setup_contracts(&env);

    let key3 = make_secp256k1_key(3);
    assert_eq!(
        verification.signer_weight(&secp256k1_pubkey(&env, &key3)),
        0
    );
}

#[test]
fn test_verify_invalid_signature() {
    let env = Env::default();
    let (verification, _security) = setup_contracts(&env);

    let key2 = make_secp256k1_key(2);
    let pubkey2 = secp256k1_pubkey(&env, &key2);

    let envelope = Bytes::from_slice(&env, b"hello world");
    // Garbage signature — not valid for any message
    let bad_sig = BytesN::from_array(&env, &[0xAA; 65]);

    let result = verification.try_check_one(&envelope, &bad_sig, &pubkey2, &None);
    assert_eq!(result, Err(Ok(VerifyError::InvalidSignature)));
}

#[test]
fn test_verify_success_high_weight() {
    let env = Env::default();
    let (verification, _security) = setup_contracts(&env);

    let key2 = make_secp256k1_key(2);
    let pubkey2 = secp256k1_pubkey(&env, &key2);

    let message = b"hello world";
    let sig_bytes = secp256k1_sign_envelope(&key2, message);

    let envelope = Bytes::from_slice(&env, message);
    let signature = BytesN::from_array(&env, &sig_bytes);

    // key2 has weight 200, should succeed and return the weight
    let result = verification.try_check_one(&envelope, &signature, &pubkey2, &None);
    assert_eq!(result, Ok(Ok(200)));
}

#[test]
fn test_check_one_returns_weight() {
    let env = Env::default();
    let (verification, _security) = setup_contracts(&env);

    let key1 = make_secp256k1_key(1);
    let pubkey1 = secp256k1_pubkey(&env, &key1);

    let message = b"hello world";
    let sig_bytes = secp256k1_sign_envelope(&key1, message);

    let envelope = Bytes::from_slice(&env, message);
    let signature = BytesN::from_array(&env, &sig_bytes);

    // key1 has weight 100 — check_one returns it without threshold comparison
    let result = verification.try_check_one(&envelope, &signature, &pubkey1, &None);
    assert_eq!(result, Ok(Ok(100)));
}

#[test]
fn test_verify_signer_not_registered() {
    let env = Env::default();
    let (verification, _security) = setup_contracts(&env);

    let key3 = make_secp256k1_key(3);
    let pubkey3 = secp256k1_pubkey(&env, &key3);

    let message = b"hello world";
    let sig_bytes = secp256k1_sign_envelope(&key3, message);

    let envelope = Bytes::from_slice(&env, message);
    let signature = BytesN::from_array(&env, &sig_bytes);

    // key3 is not registered in the security contract
    let result = verification.try_check_one(&envelope, &signature, &pubkey3, &None);
    assert_eq!(result, Err(Ok(VerifyError::SignerNotRegistered)));
}

#[test]
fn test_check_one_with_reference_block() {
    let env = Env::default();
    let admin = soroban_sdk::Address::generate(&env);

    let key1 = make_secp256k1_key(1);
    let pk1 = secp256k1_pubkey(&env, &key1);

    // Ledger 100: key1 weight 100
    env.ledger().with_mut(|li| li.sequence_number = 100);
    let security_id = env.register(Secp256k1Security, (&admin, 55u64, 100u64));
    let security = Secp256k1SecurityClient::new(&env, &security_id);
    security.mock_all_auths().add_signer(&pk1, &100);

    let verification_id = env.register(Secp256k1Verification, (&admin, &security_id));
    let verification = Secp256k1VerificationClient::new(&env, &verification_id);

    // Ledger 150: update key1 weight to 250
    env.ledger().with_mut(|li| li.sequence_number = 150);
    security.mock_all_auths().add_signer(&pk1, &250);

    // Advance to ledger 200
    env.ledger().with_mut(|li| li.sequence_number = 200);

    let message = b"hello world";
    let sig_bytes = secp256k1_sign_envelope(&key1, message);
    let envelope = Bytes::from_slice(&env, message);
    let signature = BytesN::from_array(&env, &sig_bytes);

    // None => current weight (250)
    let result = verification.try_check_one(&envelope, &signature, &pk1, &None);
    assert_eq!(result, Ok(Ok(250)));

    // Some(100) => historical weight at ledger 100 (100)
    let result = verification.try_check_one(&envelope, &signature, &pk1, &Some(100));
    assert_eq!(result, Ok(Ok(100)));

    // Some(150) => historical weight at ledger 150 (250)
    let result = verification.try_check_one(&envelope, &signature, &pk1, &Some(150));
    assert_eq!(result, Ok(Ok(250)));
}

// ── T-2: All-zero signature rejection ───────────────────────────────

#[test]
fn test_zero_signature_rejected() {
    let env = Env::default();
    let (verification, _security) = setup_contracts(&env);

    let key2 = make_secp256k1_key(2);
    let pubkey2 = secp256k1_pubkey(&env, &key2);

    let envelope = Bytes::from_slice(&env, b"hello world");
    let zero_sig = BytesN::from_array(&env, &[0u8; 65]);

    let result = verification.try_check_one(&envelope, &zero_sig, &pubkey2, &None);
    assert_eq!(result, Err(Ok(VerifyError::InvalidSignature)));
}

// ── T-3: Recovery ID 0/1 normalization ──────────────────────────────

#[test]
fn test_verify_with_raw_recovery_id() {
    let env = Env::default();
    let (verification, _security) = setup_contracts(&env);

    let key2 = make_secp256k1_key(2);
    let pubkey2 = secp256k1_pubkey(&env, &key2);

    let message = b"hello world";
    let mut sig_bytes = secp256k1_sign_envelope(&key2, message);

    // secp256k1_sign_envelope produces v=27 or v=28. Normalize to 0/1 to test that path.
    let v = sig_bytes[64];
    assert!(v == 27 || v == 28, "expected Ethereum-format recovery ID");
    sig_bytes[64] = v - 27; // convert to raw 0 or 1

    let envelope = Bytes::from_slice(&env, message);
    let signature = BytesN::from_array(&env, &sig_bytes);

    let result = verification.try_check_one(&envelope, &signature, &pubkey2, &None);
    assert_eq!(result, Ok(Ok(200)));
}

// ── verify (multi-sig) tests ────────────────────────────────────────

/// Return (lo_key, lo_pubkey, hi_key, hi_pubkey) where lo < hi by byte order.
fn ordered_keys(
    env: &Env,
) -> (
    SecpSigningKey,
    CompressedSecpPubKey,
    SecpSigningKey,
    CompressedSecpPubKey,
) {
    let key1 = make_secp256k1_key(1);
    let key2 = make_secp256k1_key(2);
    let pk1 = secp256k1_pubkey(env, &key1);
    let pk2 = secp256k1_pubkey(env, &key2);

    if pk1.to_array() < pk2.to_array() {
        (key1, pk1, key2, pk2)
    } else {
        (key2, pk2, key1, pk1)
    }
}

#[test]
fn test_verify_multi_empty_signatures() {
    let env = Env::default();
    let (verification, _) = setup_contracts(&env);

    let envelope = Bytes::from_slice(&env, b"hello world");
    let sigs: Vec<BytesN<65>> = Vec::new(&env);
    let pubs: Vec<CompressedSecpPubKey> = Vec::new(&env);

    let result = verification.try_verify(&envelope, &sigs, &pubs, &100u32);
    assert_eq!(result, Err(Ok(VerifyError::EmptySignatures)));
}

#[test]
fn test_verify_multi_length_mismatch() {
    let env = Env::default();
    let (verification, _) = setup_contracts(&env);

    let (lo_key, lo_pub, _, _) = ordered_keys(&env);
    let message = b"hello world";
    let envelope = Bytes::from_slice(&env, message);

    let sig_bytes = secp256k1_sign_envelope(&lo_key, message);
    let mut sigs: Vec<BytesN<65>> = Vec::new(&env);
    sigs.push_back(BytesN::from_array(&env, &sig_bytes));

    // Two pubkeys but only one signature
    let mut pubs: Vec<CompressedSecpPubKey> = Vec::new(&env);
    pubs.push_back(lo_pub.clone());
    pubs.push_back(lo_pub);

    let result = verification.try_verify(&envelope, &sigs, &pubs, &100u32);
    assert_eq!(result, Err(Ok(VerifyError::LengthMismatch)));
}

#[test]
fn test_verify_multi_signers_not_ordered() {
    let env = Env::default();
    let (verification, _) = setup_contracts(&env);

    let (lo_key, lo_pub, hi_key, hi_pub) = ordered_keys(&env);
    let message = b"hello world";
    let envelope = Bytes::from_slice(&env, message);

    let sig_lo = secp256k1_sign_envelope(&lo_key, message);
    let sig_hi = secp256k1_sign_envelope(&hi_key, message);

    // Provide in descending order (hi, lo) — should fail
    let mut sigs: Vec<BytesN<65>> = Vec::new(&env);
    sigs.push_back(BytesN::from_array(&env, &sig_hi));
    sigs.push_back(BytesN::from_array(&env, &sig_lo));

    let mut pubs: Vec<CompressedSecpPubKey> = Vec::new(&env);
    pubs.push_back(hi_pub);
    pubs.push_back(lo_pub);

    let result = verification.try_verify(&envelope, &sigs, &pubs, &100u32);
    assert_eq!(result, Err(Ok(VerifyError::SignersNotOrdered)));
}

#[test]
fn test_verify_multi_duplicate_signer() {
    let env = Env::default();
    let (verification, _) = setup_contracts(&env);

    let (lo_key, lo_pub, _, _) = ordered_keys(&env);
    let message = b"hello world";
    let envelope = Bytes::from_slice(&env, message);

    let sig = secp256k1_sign_envelope(&lo_key, message);

    // Same signer twice — not strictly ascending
    let mut sigs: Vec<BytesN<65>> = Vec::new(&env);
    sigs.push_back(BytesN::from_array(&env, &sig));
    sigs.push_back(BytesN::from_array(&env, &sig));

    let mut pubs: Vec<CompressedSecpPubKey> = Vec::new(&env);
    pubs.push_back(lo_pub.clone());
    pubs.push_back(lo_pub);

    let result = verification.try_verify(&envelope, &sigs, &pubs, &100u32);
    assert_eq!(result, Err(Ok(VerifyError::SignersNotOrdered)));
}

#[test]
fn test_verify_multi_one_invalid_signature() {
    let env = Env::default();
    let (verification, _) = setup_contracts(&env);

    let (_, lo_pub, hi_key, hi_pub) = ordered_keys(&env);
    let message = b"hello world";
    let envelope = Bytes::from_slice(&env, message);

    let valid_sig = secp256k1_sign_envelope(&hi_key, message);
    let bad_sig = [0xAA; 65];

    // First sig is garbage, second is valid
    let mut sigs: Vec<BytesN<65>> = Vec::new(&env);
    sigs.push_back(BytesN::from_array(&env, &bad_sig));
    sigs.push_back(BytesN::from_array(&env, &valid_sig));

    let mut pubs: Vec<CompressedSecpPubKey> = Vec::new(&env);
    pubs.push_back(lo_pub);
    pubs.push_back(hi_pub);

    let result = verification.try_verify(&envelope, &sigs, &pubs, &100u32);
    assert_eq!(result, Err(Ok(VerifyError::InvalidSignature)));
}

#[test]
fn test_verify_multi_signer_not_registered() {
    let env = Env::default();
    let (verification, _) = setup_contracts(&env);

    let key3 = make_secp256k1_key(3);
    let pk3 = secp256k1_pubkey(&env, &key3);

    let (lo_key, lo_pub, _, _) = ordered_keys(&env);
    let message = b"hello world";
    let envelope = Bytes::from_slice(&env, message);

    let sig_lo = secp256k1_sign_envelope(&lo_key, message);
    let sig_3 = secp256k1_sign_envelope(&key3, message);

    // Order them correctly by pubkey bytes
    let mut sigs: Vec<BytesN<65>> = Vec::new(&env);
    let mut pubs: Vec<CompressedSecpPubKey> = Vec::new(&env);
    if lo_pub.to_array() < pk3.to_array() {
        sigs.push_back(BytesN::from_array(&env, &sig_lo));
        sigs.push_back(BytesN::from_array(&env, &sig_3));
        pubs.push_back(lo_pub);
        pubs.push_back(pk3);
    } else {
        sigs.push_back(BytesN::from_array(&env, &sig_3));
        sigs.push_back(BytesN::from_array(&env, &sig_lo));
        pubs.push_back(pk3);
        pubs.push_back(lo_pub);
    }

    let result = verification.try_verify(&envelope, &sigs, &pubs, &100u32);
    // key3 is not registered — should fail (may hit on first or second position)
    assert_eq!(result, Err(Ok(VerifyError::SignerNotRegistered)));
}

#[test]
fn test_verify_multi_insufficient_total_weight() {
    let env = Env::default();
    let admin = soroban_sdk::Address::generate(&env);

    let key1 = make_secp256k1_key(1);
    let key2 = make_secp256k1_key(2);
    let pk1 = secp256k1_pubkey(&env, &key1);
    let pk2 = secp256k1_pubkey(&env, &key2);

    // Set ledger to 100 so checkpoints are recorded at this sequence
    env.ledger().with_mut(|li| li.sequence_number = 100);

    // key1 weight 100, key2 weight 200 — total 300, required = 300 * 55 / 100 = 165
    // key1 alone = 100 < 165
    let security_id = env.register(Secp256k1Security, (&admin, 55u64, 100u64));
    let security = Secp256k1SecurityClient::new(&env, &security_id);
    security.mock_all_auths().add_signer(&pk1, &100);
    security.mock_all_auths().add_signer(&pk2, &200);

    let verification_id = env.register(Secp256k1Verification, (&admin, &security_id));
    let verification = Secp256k1VerificationClient::new(&env, &verification_id);

    // Advance ledger past the checkpoint
    env.ledger().with_mut(|li| li.sequence_number = 200);

    let message = b"hello world";
    let envelope = Bytes::from_slice(&env, message);
    let sig1 = secp256k1_sign_envelope(&key1, message);

    // Only key1 (weight 100) — required is 165
    let mut sigs: Vec<BytesN<65>> = Vec::new(&env);
    sigs.push_back(BytesN::from_array(&env, &sig1));

    let mut pubs: Vec<CompressedSecpPubKey> = Vec::new(&env);
    pubs.push_back(pk1);

    let result = verification.try_verify(&envelope, &sigs, &pubs, &100u32);
    assert_eq!(result, Err(Ok(VerifyError::InsufficientWeight)));
}

#[test]
fn test_verify_multi_success_combined_weight() {
    let env = Env::default();
    let (verification, _) = setup_contracts(&env);

    let (lo_key, lo_pub, hi_key, hi_pub) = ordered_keys(&env);
    let message = b"hello world";
    let envelope = Bytes::from_slice(&env, message);

    let sig_lo = secp256k1_sign_envelope(&lo_key, message);
    let sig_hi = secp256k1_sign_envelope(&hi_key, message);

    // Both signers: weight 100 + 200 = 300 >= required 165
    let mut sigs: Vec<BytesN<65>> = Vec::new(&env);
    sigs.push_back(BytesN::from_array(&env, &sig_lo));
    sigs.push_back(BytesN::from_array(&env, &sig_hi));

    let mut pubs: Vec<CompressedSecpPubKey> = Vec::new(&env);
    pubs.push_back(lo_pub);
    pubs.push_back(hi_pub);

    let result = verification.try_verify(&envelope, &sigs, &pubs, &100u32);
    assert_eq!(result, Ok(Ok(())));
}

#[test]
fn test_verify_historical_passes_current_fails() {
    let env = Env::default();
    let admin = soroban_sdk::Address::generate(&env);

    let key1 = make_secp256k1_key(1);
    let key2 = make_secp256k1_key(2);
    let pk1 = secp256k1_pubkey(&env, &key1);
    let pk2 = secp256k1_pubkey(&env, &key2);

    // Ledger 100: key1=100, key2=200
    // total=300, required=300*55/100=165, key2 alone=200 >= 165 passes
    env.ledger().with_mut(|li| li.sequence_number = 100);

    let security_id = env.register(Secp256k1Security, (&admin, 55u64, 100u64));
    let security = Secp256k1SecurityClient::new(&env, &security_id);
    security.mock_all_auths().add_signer(&pk1, &100);
    security.mock_all_auths().add_signer(&pk2, &200);

    let verification_id = env.register(Secp256k1Verification, (&admin, &security_id));
    let verification = Secp256k1VerificationClient::new(&env, &verification_id);

    // Ledger 150: update key2 weight to 50
    // total=150, required=150*55/100=82, key2 alone=50 < 82 fails
    env.ledger().with_mut(|li| li.sequence_number = 150);
    security.mock_all_auths().add_signer(&pk2, &50);

    // Advance to ledger 200 for verification calls
    env.ledger().with_mut(|li| li.sequence_number = 200);

    let message = b"hello world";
    let envelope = Bytes::from_slice(&env, message);
    let sig2 = secp256k1_sign_envelope(&key2, message);

    let mut sigs: Vec<BytesN<65>> = Vec::new(&env);
    sigs.push_back(BytesN::from_array(&env, &sig2));
    let mut pubs: Vec<CompressedSecpPubKey> = Vec::new(&env);
    pubs.push_back(pk2.clone());

    // reference_block=100: key2 had weight 200, total 300, required 165 -> 200 >= 165
    let result = verification.try_verify(&envelope, &sigs, &pubs, &100u32);
    assert_eq!(result, Ok(Ok(())));

    // reference_block=150: key2 has weight 50, total 150, required 82 -> 50 < 82
    let result = verification.try_verify(&envelope, &sigs, &pubs, &150u32);
    assert_eq!(result, Err(Ok(VerifyError::InsufficientWeight)));
}

#[test]
fn test_verify_historical_fails_current_passes() {
    let env = Env::default();
    let admin = soroban_sdk::Address::generate(&env);

    let key1 = make_secp256k1_key(1);
    let key2 = make_secp256k1_key(2);
    let pk1 = secp256k1_pubkey(&env, &key1);
    let pk2 = secp256k1_pubkey(&env, &key2);

    // Ledger 100: key1=50, key2=200
    // total=250, required=250*55/100=137, key1 alone=50 < 137 fails
    env.ledger().with_mut(|li| li.sequence_number = 100);

    let security_id = env.register(Secp256k1Security, (&admin, 55u64, 100u64));
    let security = Secp256k1SecurityClient::new(&env, &security_id);
    security.mock_all_auths().add_signer(&pk1, &50);
    security.mock_all_auths().add_signer(&pk2, &200);

    let verification_id = env.register(Secp256k1Verification, (&admin, &security_id));
    let verification = Secp256k1VerificationClient::new(&env, &verification_id);

    // Ledger 150: update key1 to 200 and remove key2
    // total=200, required=200*55/100=110, key1 alone=200 >= 110 passes
    env.ledger().with_mut(|li| li.sequence_number = 150);
    security.mock_all_auths().add_signer(&pk1, &200);
    security.mock_all_auths().remove_signer(&pk2);

    // Advance to ledger 200 for verification calls
    env.ledger().with_mut(|li| li.sequence_number = 200);

    let message = b"hello world";
    let envelope = Bytes::from_slice(&env, message);
    let sig1 = secp256k1_sign_envelope(&key1, message);

    let mut sigs: Vec<BytesN<65>> = Vec::new(&env);
    sigs.push_back(BytesN::from_array(&env, &sig1));
    let mut pubs: Vec<CompressedSecpPubKey> = Vec::new(&env);
    pubs.push_back(pk1.clone());

    // reference_block=100: key1 had weight 50, total 250, required 137 -> 50 < 137
    let result = verification.try_verify(&envelope, &sigs, &pubs, &100u32);
    assert_eq!(result, Err(Ok(VerifyError::InsufficientWeight)));

    // reference_block=150: key1 has weight 200, total 200, required 110 -> 200 >= 110
    let result = verification.try_verify(&envelope, &sigs, &pubs, &150u32);
    assert_eq!(result, Ok(Ok(())));
}
