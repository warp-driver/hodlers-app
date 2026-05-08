use alloc::vec;
use alloy_sol_types::{sol, SolValue};
use soroban_sdk::Bytes;

sol! {
    struct Envelope {
        bytes20 eventId;
        bytes12 ordering;
        bytes payload;
    }

    struct HodlersPayload {
        string trader;
        int128 delta;
    }
}

impl Envelope {
    pub fn abi_decode_from(data: &Bytes) -> Option<Self> {
        let mut buf = vec![0u8; data.len() as usize];
        data.copy_into_slice(&mut buf);
        <Envelope as SolValue>::abi_decode(&buf).ok()
    }
}

impl HodlersPayload {
    pub fn abi_decode_from(payload: &alloy_primitives::Bytes) -> Option<Self> {
        <HodlersPayload as SolValue>::abi_decode(payload.as_ref()).ok()
    }
}
