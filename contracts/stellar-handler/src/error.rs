use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub enum HandlerError {
    EnvelopeDecodeFailed = 1,
    PayloadDecodeFailed = 2,
    EventAlreadySeen = 3,
    VerificationFailed = 4,
    HodlersCallFailed = 5,
}
