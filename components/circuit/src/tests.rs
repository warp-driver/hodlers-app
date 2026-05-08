use crate::phoenix::{apply, try_finalize, FieldUpdate, SwapState, XLM_SAC_CONTRACT_ID};
use crate::HodlersPayload;
use alloy_sol_types::SolValue;

#[test]
fn payload_round_trips() {
    let original = HodlersPayload {
        trader: "GAFRT3TQNZH2DVKMSAERPA5QDDXBLMSF6VRDOOEX4CYVXSXNW6YGBLGM".to_string(),
        delta: -1_234_567,
    };
    let encoded = original.abi_encode();
    let decoded = HodlersPayload::abi_decode(&encoded).unwrap();
    assert_eq!(decoded.trader, original.trader);
    assert_eq!(decoded.delta, original.delta);
}

#[test]
fn finalize_returns_none_until_all_fields_present() {
    let mut s = SwapState::default();
    apply(&mut s, FieldUpdate::Sender("GA...".into()));
    assert!(try_finalize(&s).is_none());
    apply(&mut s, FieldUpdate::SellToken("CSOMETHING".into()));
    assert!(try_finalize(&s).is_none());
    apply(&mut s, FieldUpdate::BuyToken(XLM_SAC_CONTRACT_ID.into()));
    assert!(try_finalize(&s).is_none());
    apply(&mut s, FieldUpdate::ActualReceivedAmount(500));
    assert_eq!(try_finalize(&s), Some(("GA...".into(), 500)));
}

#[test]
fn finalize_sells_xlm_with_negative_delta() {
    let mut s = SwapState::default();
    apply(&mut s, FieldUpdate::Sender("GA...".into()));
    apply(&mut s, FieldUpdate::SellToken(XLM_SAC_CONTRACT_ID.into()));
    apply(&mut s, FieldUpdate::BuyToken("CUSDC".into()));
    apply(&mut s, FieldUpdate::OfferAmount(700));
    assert_eq!(try_finalize(&s), Some(("GA...".into(), -700)));
}

#[test]
fn finalize_returns_none_for_non_xlm_swap() {
    let mut s = SwapState::default();
    apply(&mut s, FieldUpdate::Sender("GA...".into()));
    apply(&mut s, FieldUpdate::SellToken("CUSDC".into()));
    apply(&mut s, FieldUpdate::BuyToken("CSOMETHING".into()));
    apply(&mut s, FieldUpdate::OfferAmount(100));
    apply(&mut s, FieldUpdate::ActualReceivedAmount(99));
    assert_eq!(try_finalize(&s), None);
}

#[test]
#[ignore = "fill in once we have a recorded mainnet event base64"]
fn decode_real_phoenix_event() {
    // let topic_segments = vec!["AAAAD...".into(), "AAAADwAAAAZzZW5kZXI=".into()];
    // let value = "AAAAEgAAAAAAAAAA...".into();
    // let update = crate::phoenix::decode_field(&topic_segments, &value).unwrap();
    // matches!(update, FieldUpdate::Sender(_));
}
