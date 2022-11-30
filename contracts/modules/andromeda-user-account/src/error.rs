use std::str::Utf8Error;

use cosmwasm_std::OverflowError;
use cosmwasm_std::StdError;
use std::num::TryFromIntError;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("{0}")]
    Overflow(#[from] OverflowError),

    #[error("{0}")]
    TryFromInt(#[from] TryFromIntError),

    #[error("Caller is not owner.")]
    Unauthorized {},

    #[error("Uninitialized message.")]
    UninitializedMessage {},

    #[error("Unable to get current asset price to check spend limit for asset. If this transaction is urgent, use your multisig to sign. SUBMSG: {0} CONTRACT: {1} ERROR: {2}")]
    PriceCheckFailed(String, String, String),

    #[error("Spend limit and fee repay unsupported: Unknown home network")]
    UnknownHomeNetwork(String),

    #[error("Mismatched pair contract.")]
    MismatchedPairContract {},

    #[error("Semver parsing error: {0}")]
    SemVer(String),

    #[error("{0}")]
    BadSwapDenoms(String),

    #[error("Cannot send 0 funds")]
    CannotSpendZero {},

    #[error("{0}")]
    Utf8(#[from] Utf8Error),

    #[error("Custom Error val: {val:?}")]
    CustomError { val: String },
}

impl From<semver::Error> for ContractError {
    fn from(err: semver::Error) -> Self {
        Self::SemVer(err.to_string())
    }
}
