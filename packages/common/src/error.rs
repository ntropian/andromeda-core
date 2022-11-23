use cosmwasm_std::{OverflowError, StdError};
use cw20_base::ContractError as Cw20ContractError;
use cw721_base::ContractError as Cw721ContractError;
use cw_utils::{Expiration, ParseReplyError, PaymentError};
use std::convert::From;

use std::string::FromUtf8Error;
use thiserror::Error;

use hex::FromHexError;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("{0}")]
    Hex(#[from] FromHexError),

    #[error("{0}")]
    ParseReply(#[from] ParseReplyError),

    #[error("{0}")]
    Payment(#[from] PaymentError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("ContractLocked")]
    ContractLocked {},

    #[error("RewardTooLow")]
    RewardTooLow {},

    #[error("IncompleteUnbondingPeriod")]
    IncompleteUnbondingPeriod {},
    #[error("LockedNFT")]
    LockedNFT {},

    #[error("UserNotFound")]
    UserNotFound {},

    #[error("UnsupportedNFT")]
    UnsupportedNFT {},

    #[error("AlreadyUnbonded")]
    AlreadyUnbonded {},
    #[error("NFTNotFound")]
    NFTNotFound {},

    #[error("PriceNotSet")]
    PriceNotSet {},

    #[error("InvalidPrimitive")]
    InvalidPrimitive {},

    #[error("StillBonded")]
    StillBonded {},

    #[error("InsufficientBondedTime")]
    InsufficientBondedTime {},

    #[error("LockTimeTooShort")]
    LockTimeTooShort {},

    #[error("LockTimeTooLong")]
    LockTimeTooLong {},

    #[error("InvalidWeight")]
    InvalidWeight {},

    #[error("OnlyOneSourceAllowed")]
    OnlyOneSourceAllowed {},

    #[error("IllegalTokenName")]
    IllegalTokenName {},

    #[error("IllegalTokenSymbol")]
    IllegalTokenSymbol {},

    #[error("Refilling")]
    Refilling {},

    #[error("WithdrawalLimitExceeded")]
    WithdrawalLimitExceeded {},

    #[error("CoinNotFound")]
    CoinNotFound {},

    #[error("NotInRefillMode")]
    NotInRefillMode {},

    #[error("ReachedRecipientLimit")]
    ReachedRecipientLimit {},

    #[error("MinterBlacklisted")]
    MinterBlacklisted {},

    #[error("EmptyRecipientsList")]
    EmptyRecipientsList {},

    #[error("AmountExceededHundredPrecent")]
    AmountExceededHundredPrecent {},

    #[error("InvalidAddress")]
    InvalidAddress {},

    #[error("ExpirationInPast")]
    ExpirationInPast {},

    #[error("UnspecifiedWithdrawalFrequency")]
    UnspecifiedWithdrawalFrequency {},

    #[error("ExpirationNotSpecified")]
    ExpirationNotSpecified {},

    #[error("CannotOverwriteHeldFunds")]
    CannotOverwriteHeldFunds {},

    #[error("ContractAddressNotInAddressList")]
    ContractAddressNotInAddressList {},

    #[error("ModuleNotUnique")]
    ModuleNotUnique {},

    #[error("InvalidRate")]
    InvalidRate {},

    #[error("InsufficientFunds")]
    InsufficientFunds {},

    #[error("NoPendingPayments")]
    NoPendingPayments {},

    #[error("NoReceivingAddress")]
    NoReceivingAddress {},

    #[error("AccountNotFound")]
    AccountNotFound {},

    #[error("ModuleDiscriptionTooLong: {msg}")]
    ModuleDiscriptionTooLong { msg: String },

    #[error("SymbolInUse")]
    SymbolInUse {},

    #[error("ExceedsMaxAllowedCoins")]
    ExceedsMaxAllowedCoins {},

    #[error("NoLockedFunds")]
    NoLockedFunds {},

    #[error("FundsAreLocked")]
    FundsAreLocked {},

    #[error("InvalidTokenNameLength: {msg}")]
    InvalidTokenNameLength { msg: String },

    #[error("TokenIsArchived")]
    TokenIsArchived {},

    #[error("AuctionDoesNotExist")]
    AuctionDoesNotExist {},

    #[error("SaleDoesNotExist")]
    SaleDoesNotExist {},

    #[error("AuctionNotStarted")]
    AuctionNotStarted {},

    #[error("AuctionEnded")]
    AuctionEnded {},

    #[error("SaleNotOpen")]
    SaleNotOpen {},

    #[error("TokenOwnerCannotBid")]
    TokenOwnerCannotBid {},

    #[error("TokenOwnerCannotBuy")]
    TokenOwnerCannotBuy {},

    #[error("BidSmallerThanHighestBid")]
    BidSmallerThanHighestBid {},

    #[error("Overflow")]
    Overflow {},

    #[error("CannotWithdrawHighestBid")]
    CannotWithdrawHighestBid {},

    #[error("WithdrawalIsEmpty")]
    WithdrawalIsEmpty {},

    #[error("AuctionAlreadyStarted")]
    AuctionAlreadyStarted {},

    #[error("StartTimeAfterEndTime")]
    StartTimeAfterEndTime {},

    #[error("Start time in past. Current time: {current_time}. Current block: {current_block}")]
    StartTimeInThePast {
        current_time: u64,
        current_block: u64,
    },

    #[error("OutOfNFTs")]
    OutOfNFTs {},

    #[error("HighestBidderCannotOutBid")]
    HighestBidderCannotOutBid {},

    #[error("InvalidFunds: {msg}")]
    InvalidFunds { msg: String },

    #[error("AuctionRewardAlreadyClaimed")]
    AuctionAlreadyClaimed {},

    #[error("SaleAlreadyConducted")]
    SaleAlreadyConducted {},

    #[error("AuctionNotEnded")]
    AuctionNotEnded {},

    #[error("AuctionCancelled")]
    AuctionCancelled {},

    #[error("ExpirationMustNotBeNever")]
    ExpirationMustNotBeNever {},

    #[error("ExpirationsMustBeOfSameType")]
    ExpirationsMustBeOfSameType {},

    #[error("MoreThanOneCoin")]
    MoreThanOneCoin {},

    #[error("InvalidReplyId")]
    InvalidReplyId {},

    #[error("ParsingError: {err}")]
    ParsingError { err: String },

    #[error("MissingRequiredMessageData")]
    MissingRequiredMessageData {},

    #[error("Cannot migrate from different contract type: {previous_contract}")]
    CannotMigrate { previous_contract: String },

    #[error("NestedAndromedaMsg")]
    NestedAndromedaMsg {},

    #[error("UnexpectedExternalRate")]
    UnexpectedExternalRate {},

    #[error("DuplicateCoinDenoms")]
    DuplicateCoinDenoms {},

    #[error("DuplicateRecipient")]
    DuplicateRecipient {},

    #[error("DuplicateContract")]
    DuplicateContract {},

    // BEGIN CW20 ERRORS
    #[error("Cannot set to own account")]
    CannotSetOwnAccount {},

    #[error("Invalid zero amount")]
    InvalidZeroAmount {},

    #[error("Allowance is expired")]
    Expired {},

    #[error("No allowance for this account")]
    NoAllowance {},

    #[error("Minting cannot exceed the cap")]
    CannotExceedCap {},

    #[error("Logo binary data exceeds 5KB limit")]
    LogoTooBig {},

    #[error("Invalid xml preamble for SVG")]
    InvalidXmlPreamble {},

    #[error("Invalid png header")]
    InvalidPngHeader {},

    #[error("Duplicate initial balance addresses")]
    DuplicateInitialBalanceAddresses {},

    // END CW20 ERRORS
    #[error("Invalid Module, {msg:?}")]
    InvalidModule { msg: Option<String> },

    #[error("UnsupportedOperation")]
    UnsupportedOperation {},

    #[error("IncompatibleModules: {msg}")]
    IncompatibleModules { msg: String },

    #[error("ModuleDoesNotExist")]
    ModuleDoesNotExist {},

    #[error("token_id already claimed")]
    Claimed {},

    #[error("Approval not found for: {spender}")]
    ApprovalNotFound { spender: String },

    #[error("BidAlreadyPlaced")]
    BidAlreadyPlaced {},

    #[error("BidLowerThanCurrent")]
    BidLowerThanCurrent {},

    #[error("BidNotExpired")]
    BidNotExpired {},

    #[error("TransferAgreementExists")]
    TransferAgreementExists {},

    #[error("CannotDoubleWrapToken")]
    CannotDoubleWrapToken {},

    #[error("UnwrappingDisabled")]
    UnwrappingDisabled {},

    #[error("TokenNotWrappedByThisContract")]
    TokenNotWrappedByThisContract {},

    #[error("InvalidMetadata")]
    InvalidMetadata {},

    #[error("InvalidRecipientType: {msg}")]
    InvalidRecipientType { msg: String },

    #[error("InvalidTokensToWithdraw: {msg}")]
    InvalidTokensToWithdraw { msg: String },

    #[error("ModuleImmutable")]
    ModuleImmutable {},

    #[error("GeneratorNotSpecified")]
    GeneratorNotSpecified {},

    #[error("TooManyAppComponents")]
    TooManyAppComponents {},

    #[error("InvalidLtvRatio: {msg}")]
    InvalidLtvRatio { msg: String },

    #[error("Name already taken")]
    NameAlreadyTaken {},

    #[error("No Ongoing Sale")]
    NoOngoingSale {},

    #[error("Purchase limit reached")]
    PurchaseLimitReached {},

    #[error("Sale not ended")]
    SaleNotEnded {},

    #[error("Min sales exceeded")]
    MinSalesExceeded {},

    #[error("Limit must not be zero")]
    LimitMustNotBeZero {},

    #[error("Sale has already started")]
    SaleStarted {},

    #[error("No purchases")]
    NoPurchases {},

    #[error("Cannot mint after sale conducted")]
    CannotMintAfterSaleConducted {},

    #[error("Not implemented: {msg:?}")]
    NotImplemented { msg: Option<String> },

    #[error("Invalid Strategy: {strategy}")]
    InvalidStrategy { strategy: String },

    #[error("Invalid Query")]
    InvalidQuery {},

    #[error("Invalid Withdrawal: {msg:?}")]
    InvalidWithdrawal { msg: Option<String> },

    #[error("Airdrop stage {stage} expired at {expiration}")]
    StageExpired { stage: u8, expiration: Expiration },

    #[error("Airdrop stage {stage} not expired yet")]
    StageNotExpired { stage: u8, expiration: Expiration },

    #[error("Wrong Length")]
    WrongLength {},

    #[error("Verification Failed")]
    VerificationFailed {},

    #[error("Invalid Asset: {asset}")]
    InvalidAsset { asset: String },

    #[error("Invalid cycle duration")]
    InvalidCycleDuration {},

    #[error("Reward increase must be less than 1")]
    InvalidRewardIncrease {},

    #[error("Max of {max} for reward tokens is exceeded")]
    MaxRewardTokensExceeded { max: u32 },

    #[error("Primitive Does Not Exist: {msg}")]
    PrimitiveDoesNotExist { msg: String },

    #[error("Token already being distributed")]
    TokenAlreadyBeingDistributed {},

    #[error("Deposit window closed")]
    DepositWindowClosed {},

    #[error("No saved auction contract")]
    NoSavedBootstrapContract {},

    #[error("Phase ongoing")]
    PhaseOngoing {},

    #[error("Claims already allowed")]
    ClaimsAlreadyAllowed {},

    #[error("ClaimsNotAllowed")]
    ClaimsNotAllowed {},

    #[error("Lockdrop already claimed")]
    LockdropAlreadyClaimed {},

    #[error("No lockup to claim rewards for")]
    NoLockup {},

    #[error("Invalid deposit/withdraw window")]
    InvalidWindow {},

    #[error("Duplicate tokens")]
    DuplicateTokens {},

    #[error("All tokens purchased")]
    AllTokensPurchased {},

    #[error("Token not available")]
    TokenNotAvailable {},

    #[error("Too many mint messages, limit is {limit}")]
    TooManyMintMessages { limit: u32 },

    #[error("App contract not specified")]
    AppContractNotSpecified {},

    #[error("Invalid component: {name}")]
    InvalidComponent { name: String },

    #[error("Multi-batch not supported")]
    MultiBatchNotSupported {},

    #[error("Unexpected number of bytes. Expected: {expected}, actual: {actual}")]
    UnexpectedNumberOfBytes { expected: u8, actual: usize },

    #[error("Not an assigned operator, {msg:?}")]
    NotAssignedOperator { msg: Option<String> },

    #[error("Invalid Expiration Time")]
    InvalidExpirationTime {},

    #[error("Cannot spend more than limit: {0} {1}")]
    CannotSpendMoreThanLimit(String, String),
}

impl From<Cw20ContractError> for ContractError {
    fn from(err: Cw20ContractError) -> Self {
        match err {
            Cw20ContractError::Std(std) => ContractError::Std(std),
            Cw20ContractError::Expired {} => ContractError::Expired {},
            Cw20ContractError::LogoTooBig {} => ContractError::LogoTooBig {},
            Cw20ContractError::NoAllowance {} => ContractError::NoAllowance {},
            Cw20ContractError::Unauthorized {} => ContractError::Unauthorized {},
            Cw20ContractError::CannotExceedCap {} => ContractError::CannotExceedCap {},
            Cw20ContractError::InvalidPngHeader {} => ContractError::InvalidPngHeader {},
            Cw20ContractError::InvalidZeroAmount {} => ContractError::InvalidZeroAmount {},
            Cw20ContractError::InvalidXmlPreamble {} => ContractError::InvalidXmlPreamble {},
            Cw20ContractError::CannotSetOwnAccount {} => ContractError::CannotSetOwnAccount {},
            Cw20ContractError::DuplicateInitialBalanceAddresses {} => {
                ContractError::DuplicateInitialBalanceAddresses {}
            }
        }
    }
}

impl From<Cw721ContractError> for ContractError {
    fn from(err: Cw721ContractError) -> Self {
        match err {
            Cw721ContractError::Std(std) => ContractError::Std(std),
            Cw721ContractError::Expired {} => ContractError::Expired {},
            Cw721ContractError::Unauthorized {} => ContractError::Unauthorized {},
            Cw721ContractError::Claimed {} => ContractError::Claimed {},
            Cw721ContractError::ApprovalNotFound { spender } => {
                ContractError::ApprovalNotFound { spender }
            }
        }
    }
}

impl From<FromUtf8Error> for ContractError {
    fn from(err: FromUtf8Error) -> Self {
        ContractError::Std(StdError::from(err))
    }
}

impl From<OverflowError> for ContractError {
    fn from(_err: OverflowError) -> Self {
        ContractError::Overflow {}
    }
}
