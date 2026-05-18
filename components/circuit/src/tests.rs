use crate::payload;
use crate::phoenix::{
    apply, decode_field, try_finalize, FieldUpdate, SwapState, XLM_SAC_CONTRACT_ID,
};
use std::str::FromStr;
use stellar_xdr::curr::{
    Int128Parts, Limits, ReadXdr, ScAddress, ScString, ScSymbol, ScVal, StringM, WriteXdr,
};

#[test]
fn payload_encodes_as_scmap_with_alphabetical_entries() {
    let trader = "GAFRT3TQNZH2DVKMSAERPA5QDDXBLMSF6VRDOOEX4CYVXSXNW6YGBLGM";
    let delta: i128 = -1_234_567;
    let bytes = payload::encode(trader, delta).unwrap();

    let decoded = ScVal::from_xdr(&bytes, Limits::none()).unwrap();
    let entries = match decoded {
        ScVal::Map(Some(m)) => m.0.to_vec(),
        other => panic!("expected ScMap, got {other:?}"),
    };
    assert_eq!(entries.len(), 2);

    // Soroban contracttype serializes map entries sorted by symbol key —
    // alphabetically: "delta" before "trader".
    let delta_sym = ScSymbol("delta".try_into().unwrap());
    let trader_sym = ScSymbol("trader".try_into().unwrap());
    assert_eq!(entries[0].key, ScVal::Symbol(delta_sym));
    assert_eq!(entries[1].key, ScVal::Symbol(trader_sym));

    match &entries[0].val {
        ScVal::I128(Int128Parts { hi, lo }) => {
            let recombined = ((*hi as i128) << 64) | (*lo as u128 as i128);
            assert_eq!(recombined, delta);
        }
        other => panic!("expected I128, got {other:?}"),
    }

    match &entries[1].val {
        ScVal::String(ScString(s)) => {
            assert_eq!(std::str::from_utf8(s.as_slice()).unwrap(), trader);
        }
        other => panic!("expected String, got {other:?}"),
    }
}

#[test]
fn finalize_buys_xlm_with_positive_delta() {
    let mut s = SwapState::default();
    apply(&mut s, FieldUpdate::Sender("GA...".into()));
    apply(&mut s, FieldUpdate::SellToken("CUSDC".into()));
    apply(&mut s, FieldUpdate::BuyToken(XLM_SAC_CONTRACT_ID.into()));
    apply(&mut s, FieldUpdate::OfferAmount(100));
    assert!(try_finalize(&s).is_none());
    apply(&mut s, FieldUpdate::ReturnAmount(500));
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
    apply(&mut s, FieldUpdate::ReturnAmount(99));
    assert_eq!(try_finalize(&s), None);
}

// Recorded mainnet swap: tx cfda8d134eda95d30e7059c1277af9bd4a809496b22ab2ae7857a0e8c1e811e7
// Trader GB3JC... sold 820_000_000 XLM stroops for 131_802_593 USDC units.
#[test]
fn decode_real_phoenix_event() {
    let trader = "GB3JCHJUP6HHZJLN5LKQDRFP2HWSLNXYE2TGWDGBTNXIW6MLVRQXNDBC";
    let xlm = XLM_SAC_CONTRACT_ID;
    let usdc = "CCW67TSZV3SSS2HXMBQ5JFGCKJNXKZM7UQUWUZPUTHXSTZLEO7SJMI75";

    let events = [
        (string_topic("swap"), string_topic("sender"), addr_value(trader)),
        (string_topic("swap"), string_topic("sell_token"), addr_value(xlm)),
        (string_topic("swap"), string_topic("offer_amount"), i128_value(820_000_000)),
        (string_topic("swap"), string_topic("buy_token"), addr_value(usdc)),
        (string_topic("swap"), string_topic("return_amount"), i128_value(131_802_593)),
    ];

    let mut state = SwapState::default();
    for (t0, t1, v) in &events {
        let update = decode_field(&[t0.clone(), t1.clone()], v).unwrap();
        apply(&mut state, update);
    }

    assert_eq!(
        try_finalize(&state),
        Some((trader.to_string(), -820_000_000))
    );
}

fn string_topic(s: &str) -> String {
    let inner: StringM = s.try_into().unwrap();
    ScVal::String(ScString(inner))
        .to_xdr_base64(Limits::none())
        .unwrap()
}

fn addr_value(strkey: &str) -> String {
    let addr = ScAddress::from_str(strkey).unwrap();
    ScVal::Address(addr).to_xdr_base64(Limits::none()).unwrap()
}

fn i128_value(n: i128) -> String {
    let hi = (n >> 64) as i64;
    let lo = n as u64;
    ScVal::I128(Int128Parts { hi, lo })
        .to_xdr_base64(Limits::none())
        .unwrap()
}
