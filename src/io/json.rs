use std::{fs, path::Path};

use serde::{Serialize, de::DeserializeOwned};

use crate::{
    error::SenateSimError,
    model::{
        legislative::LegislativeObject, legislative_context::LegislativeContext, senator::Senator,
    },
};

pub fn load_senator_from_path(path: impl AsRef<Path>) -> Result<Senator, SenateSimError> {
    let senator: Senator = load_json_from_path(path)?;
    senator.validate()?;
    Ok(senator)
}

pub fn load_legislative_object_from_path(
    path: impl AsRef<Path>,
) -> Result<LegislativeObject, SenateSimError> {
    let legislative_object: LegislativeObject = load_json_from_path(path)?;
    legislative_object.validate()?;
    Ok(legislative_object)
}

pub fn load_legislative_context_from_path(
    path: impl AsRef<Path>,
) -> Result<LegislativeContext, SenateSimError> {
    let context: LegislativeContext = load_json_from_path(path)?;
    context.validate()?;
    Ok(context)
}

pub fn to_pretty_json<T: Serialize>(value: &T) -> Result<String, SenateSimError> {
    serde_json::to_string_pretty(value).map_err(SenateSimError::Serialize)
}

pub fn senator_to_pretty_json(senator: &Senator) -> Result<String, SenateSimError> {
    to_pretty_json(senator)
}

fn load_json_from_path<T: DeserializeOwned>(path: impl AsRef<Path>) -> Result<T, SenateSimError> {
    let path = path.as_ref();
    let contents = fs::read_to_string(path).map_err(|source| SenateSimError::Io {
        path: path.to_path_buf(),
        source,
    })?;

    serde_json::from_str(&contents).map_err(|source| SenateSimError::Json {
        path: path.to_path_buf(),
        source,
    })
}
