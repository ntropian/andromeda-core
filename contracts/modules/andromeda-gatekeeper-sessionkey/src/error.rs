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

    #[error("Transaction delay still in progress for tx {0}")]
    TransactionDelayInProgress(u64),

    #[error("Custom Error val: {val:?}")]
    CustomError { val: String },
    // Add any other custom errors you like here.
    // Look at https://docs.rs/thiserror/1.0.21/thiserror/ for details.
}
