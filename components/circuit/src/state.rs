use anyhow::{Context, Result};

use crate::phoenix::SwapState;

const BUCKET: &str = "hodlers-circuit-swap-state";

pub fn load(key: &str) -> Result<SwapState> {
    let bucket = open_bucket()?;
    match bucket.get(&key.to_string()).context("kv get")? {
        Some(bytes) => serde_json::from_slice(&bytes).context("deserialize SwapState"),
        None => Ok(SwapState::default()),
    }
}

pub fn save(key: &str, state: &SwapState) -> Result<()> {
    let bucket = open_bucket()?;
    let bytes = serde_json::to_vec(state).context("serialize SwapState")?;
    bucket.set(&key.to_string(), &bytes).context("kv set")?;
    Ok(())
}

pub fn delete(key: &str) -> Result<()> {
    let bucket = open_bucket()?;
    bucket.delete(&key.to_string()).context("kv delete")?;
    Ok(())
}

fn open_bucket() -> Result<crate::wasi::keyvalue::store::Bucket> {
    crate::wasi::keyvalue::store::open(&BUCKET.to_string()).context("open kv bucket")
}
