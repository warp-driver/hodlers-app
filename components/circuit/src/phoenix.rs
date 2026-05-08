use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use stellar_xdr::curr::{Limits, ReadXdr, ScSymbol, ScVal};

// XLM Stellar Asset Contract on Stellar mainnet (the "native" SAC).
pub const XLM_SAC_CONTRACT_ID: &str =
    "CAS3J7GYLGXMF6TDJBBYYSE3HQ6BBSMLNUQ34T6TZMYMW2EVH34XOWMA";

#[derive(Default, Serialize, Deserialize)]
pub struct SwapState {
    pub sender: Option<String>,
    pub sell_token: Option<String>,
    pub buy_token: Option<String>,
    pub offer_amount: Option<i128>,
    pub return_amount: Option<i128>,
}

pub enum FieldUpdate {
    Sender(String),
    SellToken(String),
    BuyToken(String),
    OfferAmount(i128),
    ReturnAmount(i128),
    Other,
}

pub fn decode_field(topic_segments: &[String], value: &str) -> Result<FieldUpdate> {
    if topic_segments.len() < 2 {
        return Ok(FieldUpdate::Other);
    }
    let key_scval = ScVal::from_xdr_base64(&topic_segments[1], Limits::none())
        .context("decode topic[1]")?;
    let key = match key_scval {
        ScVal::Symbol(ScSymbol(s)) => s.to_string(),
        ScVal::String(s) => s.to_string(),
        _ => return Ok(FieldUpdate::Other),
    };
    let val = ScVal::from_xdr_base64(value, Limits::none()).context("decode value")?;

    Ok(match key.as_str() {
        "sender" => FieldUpdate::Sender(decode_address_strkey(&val)?),
        "sell_token" => FieldUpdate::SellToken(decode_address_strkey(&val)?),
        "buy_token" => FieldUpdate::BuyToken(decode_address_strkey(&val)?),
        "offer_amount" => FieldUpdate::OfferAmount(decode_i128(&val)?),
        "return_amount" => FieldUpdate::ReturnAmount(decode_i128(&val)?),
        _ => FieldUpdate::Other,
    })
}

pub fn apply(state: &mut SwapState, update: FieldUpdate) {
    match update {
        FieldUpdate::Sender(s) => state.sender = Some(s),
        FieldUpdate::SellToken(s) => state.sell_token = Some(s),
        FieldUpdate::BuyToken(s) => state.buy_token = Some(s),
        FieldUpdate::OfferAmount(n) => state.offer_amount = Some(n),
        FieldUpdate::ReturnAmount(n) => state.return_amount = Some(n),
        FieldUpdate::Other => {}
    }
}

pub fn try_finalize(state: &SwapState) -> Option<(String, i128)> {
    let sender = state.sender.as_ref()?;
    let sell_token = state.sell_token.as_ref()?;
    let buy_token = state.buy_token.as_ref()?;

    if buy_token == XLM_SAC_CONTRACT_ID {
        let amount = state.return_amount?;
        Some((sender.clone(), amount))
    } else if sell_token == XLM_SAC_CONTRACT_ID {
        let amount = state.offer_amount?;
        Some((sender.clone(), -amount))
    } else {
        None
    }
}

fn decode_address_strkey(val: &ScVal) -> Result<String> {
    match val {
        ScVal::Address(addr) => Ok(addr.to_string()),
        _ => Err(anyhow!("expected ScVal::Address, got {val:?}")),
    }
}

fn decode_i128(val: &ScVal) -> Result<i128> {
    match val {
        ScVal::I128(parts) => Ok(((parts.hi as i128) << 64) | (parts.lo as u128 as i128)),
        _ => Err(anyhow!("expected ScVal::I128, got {val:?}")),
    }
}
