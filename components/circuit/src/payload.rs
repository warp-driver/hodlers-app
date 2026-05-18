use anyhow::{Context, Result};
use stellar_xdr::curr::{
    Int128Parts, Limits, ScMap, ScMapEntry, ScString, ScSymbol, ScVal, StringM, WriteXdr,
};

// Encode a Soroban `contracttype` HodlersPayload { delta: i128, trader: String }
// as an XDR-serialized ScVal::Map. Field entries must be sorted ascending by
// key — alphabetically that's "delta" before "trader". The handler decodes
// these bytes via `HodlersPayload::from_xdr`.
pub fn encode(trader: &str, delta: i128) -> Result<Vec<u8>> {
    let hi = (delta >> 64) as i64;
    let lo = (delta as u128 & u64::MAX as u128) as u64;

    let delta_entry = ScMapEntry {
        key: symbol_val("delta")?,
        val: ScVal::I128(Int128Parts { hi, lo }),
    };

    let trader_string: StringM = trader
        .as_bytes()
        .try_into()
        .context("trader string too long for XDR StringM")?;
    let trader_entry = ScMapEntry {
        key: symbol_val("trader")?,
        val: ScVal::String(ScString(trader_string)),
    };

    let map = ScMap(
        vec![delta_entry, trader_entry]
            .try_into()
            .context("ScMap construction")?,
    );

    ScVal::Map(Some(map))
        .to_xdr(Limits::none())
        .context("xdr-encode HodlersPayload")
}

fn symbol_val(s: &str) -> Result<ScVal> {
    let inner: StringM<32> = s.as_bytes().try_into().context("symbol too long")?;
    Ok(ScVal::Symbol(ScSymbol(inner)))
}
