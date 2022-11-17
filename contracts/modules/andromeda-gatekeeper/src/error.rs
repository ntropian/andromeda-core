use andromeda_modules::gatekeeper::Authorization;
use cosmwasm_std::StdError;
use std::str::Utf8Error;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("{0}")]
    CommonError(#[from] common::error::ContractError),

    #[error(transparent)]
    JsonError(#[from] serde_json_wasm::de::Error),
    #[error(transparent)]
    JsonSerError(#[from] serde_json_wasm::ser::Error),

    #[error("{0}")]
    Utf8(#[from] Utf8Error),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("Not operator")]
    NotOperator {},

    #[error("No authorization for target contract")]
    NoSuchAuthorization,

    #[error("Field mismatch: field {key:?} must contain parameter {value:?}")]
    FieldMismatch { key: String, value: String },

    #[error("Missing required field: field {key:?} must contain parameter {value:?}")]
    MissingRequiredField { key: String, value: String },

    #[error("Multiple matching authorizations. Please be more specific or use rm_all_matching_authorizations. Found: {vector:?}")]
    MultipleMatchingAuthorizations {
        vector: Vec<(Vec<u8>, Authorization)>,
    },

    #[error("Custom Error val: {val:?}")]
    CustomError { val: String },
    // Add any other custom errors you like here.
    // Look at https://docs.rs/thiserror/1.0.21/thiserror/ for details.
}
