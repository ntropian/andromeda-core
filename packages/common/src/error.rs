use cosmwasm_std::{OverflowError, StdError};
use cw20_base::ContractError as Cw20ContractError;
use cw721_base::ContractError as Cw721ContractError;
use cw_controllers::AdminError;
use cw_utils::{Expiration, ParseReplyError, PaymentError};
use std::convert::From;
// use std::num::TryFromIntError;
use std::string::FromUtf8Error;
use thiserror::Error;

use hex::FromHexError;

/// Never is a placeholder to ensure we don't return any errors
#[derive(Error, Debug)]
pub enum Never {}

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

    #[error("{0}")]
    Admin(#[from] AdminError),

    #[error("Channel doesn't exist: {id}")]
    NoSuchChannel { id: String },

    #[error("Didn't send any funds")]
    NoFunds {},

    #[error("Amount larger than 2**64, not supported by ics20 packets")]
    AmountOverflow {},

    #[error("Only supports channel with ibc version ics20-1, got {version}")]
    InvalidIbcVersion { version: String },

    #[error("Only supports unordered channel")]
    OnlyOrderedChannel {},

    #[error("Only accepts tokens that originate on this chain, not native tokens of remote chain")]
    NoForeignTokens {},

    #[error("Parsed port from denom ({port}) doesn't match packet")]
    FromOtherPort { port: String },

    #[error("Parsed channel from denom ({channel}) doesn't match packet")]
    FromOtherChannel { channel: String },

    #[error("Cannot migrate from unsupported version: {previous_version}")]
    CannotMigrateVersion { previous_version: String },

    #[error("Got a submessage reply with unknown id: {id}")]
    UnknownReplyId { id: u64 },

    #[error("You cannot lower the gas limit for a contract on the allow list")]
    CannotLowerGas,

    #[error("You can only send cw20 tokens that have been explicitly allowed by governance")]
    NotOnAllowList,

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("ContractLocked")]
    ContractLocked {},

    #[error("LockedNFT")]
    LockedNFT {},

    #[error("UserNotFound")]
    UserNotFound {},

    #[error("NFTNotFound")]
    NFTNotFound {},

    #[error("PriceNotSet")]
    PriceNotSet {},

    #[error("InvalidPrimitive")]
    InvalidPrimitive {},

    #[error("LockTimeTooShort")]
    LockTimeTooShort {},

    #[error("LockTimeTooLong")]
    LockTimeTooLong {},

    #[error("InvalidWeight")]
    InvalidWeight {},

    #[error("IllegalTokenName")]
    IllegalTokenName {},

    #[error("IllegalTokenSymbol")]
    IllegalTokenSymbol {},

    #[error("Refilling")]
    Refilling {},

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

    #[error("FunctionDeclinesFunds")]
    FunctionDeclinesFunds {},

    #[error("ExpirationInPast")]
    ExpirationInPast {},

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

    #[error("AuctionNotStarted")]
    AuctionNotStarted {},

    #[error("AuctionEnded")]
    AuctionEnded {},

    #[error("TokenOwnerCannotBid")]
    TokenOwnerCannotBid {},

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

    #[error(
        "Start time in past. Current seconds: {current_seconds}. Current block: {current_block}"
    )]
    StartTimeInThePast {
        current_seconds: u64,
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

    #[error("OfferAlreadyPlaced")]
    OfferAlreadyPlaced {},

    #[error("OfferLowerThanCurrent")]
    OfferLowerThanCurrent {},

    #[error("OfferNotExpired")]
    OfferNotExpired {},

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

// impl From<TryFromIntError> for ContractError {
//     fn from(_: TryFromIntError) -> Self {
//         ContractError::AmountOverflow {}
//     }
// }
// impl FromResidual<Result<Infallible, PaymentError>> for ContractError {
//     fn from(_: FromResidualErr) -> Self {
//         ContractError
//     }
// }
