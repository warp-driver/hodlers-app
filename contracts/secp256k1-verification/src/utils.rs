use soroban_sdk::{Bytes, BytesN, Env, crypto::Hash};
use warpdrive_shared::interfaces::{CompressedSecpPubKey, SecpSignature};

/// EIP-191 hash: `keccak256("\x19Ethereum Signed Message:\n32" || digest)`
///
/// Matches Solidity's `ECDSA.toEthSignedMessageHash(bytes32)`.
fn eip191_hash_message(env: &Env, digest: &BytesN<32>) -> Hash<32> {
    let mut prefixed = Bytes::new(env);
    // "\x19Ethereum Signed Message:\n32"
    prefixed.extend_from_array(&[
        0x19, b'E', b't', b'h', b'e', b'r', b'e', b'u', b'm', b' ', b'S', b'i', b'g', b'n', b'e',
        b'd', b' ', b'M', b'e', b's', b's', b'a', b'g', b'e', b':', b'\n', b'3', b'2',
    ]);
    prefixed.extend_from_slice(&digest.to_array());
    env.crypto().keccak256(&prefixed)
}

/// Compress an uncompressed secp256k1 public key (65 bytes: `04 || x || y`)
/// into compressed form (33 bytes: `prefix || x`) where prefix is `0x02` (even y)
/// or `0x03` (odd y).
fn compress_pubkey(env: &Env, uncompressed: &BytesN<65>) -> CompressedSecpPubKey {
    let raw = uncompressed.to_array();
    let mut compressed = [0u8; 33];
    // Prefix: 0x02 if y is even, 0x03 if y is odd
    compressed[0] = 0x02 | (raw[64] & 1);
    // Copy x-coordinate (bytes 1..33 of uncompressed)
    compressed[1..33].copy_from_slice(&raw[1..33]);
    BytesN::from_array(env, &compressed)
}

/// Verifies an EIP-191 secp256k1 signature against a compressed public key.
///
/// Mimics Solidity's `signer.isValidSignatureNow(digest, signature)`.
///
/// - `envelope`: raw payload that was signed (keccak256-hashed, then EIP-191 wrapped)
/// - `signature`: 65-byte ECDSA signature (`r[32] || s[32] || v[1]`)
/// - `signer_pubkey`: expected signer's compressed secp256k1 public key (33 bytes)
pub fn is_valid_signature(
    env: &Env,
    envelope: &Bytes,
    signature: &SecpSignature,
    signer_pubkey: &CompressedSecpPubKey,
) -> bool {
    let sig_bytes = signature.to_array();

    // Reject all-zero signatures explicitly. A zero signature is a known attack vector
    // ("phantom signatures") that can trick some ECDSA recovery implementations into
    // returning a valid-looking public key. Do not remove this check.
    if sig_bytes.iter().all(|&b| b == 0) {
        return false;
    }

    // Split into r||s (64 bytes) and recovery byte v
    let mut rs = [0u8; 64];
    rs.copy_from_slice(&sig_bytes[0..64]);
    let recovery_id = sig_bytes[64];

    // Normalize recovery ID from Ethereum format (27/28) to 0/1
    let normalized = match recovery_id {
        27 => 0u32,
        28 => 1u32,
        0 | 1 => recovery_id as u32,
        _ => return false,
    };

    // keccak256(envelope), then EIP-191 wrap
    let inner_hash: BytesN<32> = env.crypto().keccak256(envelope).into();
    let digest = eip191_hash_message(env, &inner_hash);

    // Recover the uncompressed public key from the message digest + signature
    let rs_bytes = BytesN::from_array(env, &rs);
    let recovered: BytesN<65> = env
        .crypto()
        .secp256k1_recover(&digest, &rs_bytes, normalized);

    // Compress recovered key and compare with expected signer
    compress_pubkey(env, &recovered) == *signer_pubkey
}
