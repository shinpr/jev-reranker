use serde_json::{Map, Value};

use crate::error::AppError;

pub fn serialize_output(objects: &[Map<String, Value>]) -> Result<Vec<u8>, AppError> {
    let mut output =
        serde_json::to_vec(objects).map_err(|source| AppError::Serialization { source })?;
    output.push(b'\n');
    Ok(output)
}
