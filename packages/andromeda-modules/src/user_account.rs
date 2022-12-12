use ado_base::ADOContract;
use common::{
    ado_base::{hooks::AndromedaHook, AndromedaMsg, AndromedaQuery},
    error::ContractError,
};
use cosmwasm_std::{
    ensure, to_binary, BankMsg, Coin, CosmosMsg, Deps, QueryRequest, StakingMsg, StdError, WasmMsg,
    WasmQuery, StdResult, Binary,
};
use cw_storage_plus::Item;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    gatekeeper_common::UniversalMsg,
    gatekeeper_message::AuthorizationsResponse,
    gatekeeper_sessionkey::CanExecuteResponse,
    gatekeeper_spendlimit::CanSpendResponse,
    submsgs::{PendingSubmsg, SubmsgType, WasmmsgType},
};

use crate::gatekeeper_spendlimit::QueryMsg as SpendlimitQueryMsg;
use SpendlimitQueryMsg::CanSpend;

use crate::gatekeeper_message::QueryMsg as MessageQueryMsg;
use MessageQueryMsg::CheckTransaction;

use crate::gatekeeper_sessionkey::QueryMsg as SessionkeyQueryMsg;
use SessionkeyQueryMsg::CanExecute;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct InstantiateMsg {
    pub account: UserAccount,
    pub starting_usd_debt: Option<u64>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    AndrReceive(AndromedaMsg),
    /// Update the owner of the contract, possibly with a delay
    ProposeUpdateOwner {
        /// The new owner
        new_owner: String,
    },
    /// Change the delay for owner updates, cannot be done if update is pending
    ChangeOwnerUpdatesDelay {
        /// The new delay in seconds
        new_delay: u64,
    },
    /// Execute a message, if it passes the checks
    Execute {
        /// The message to execute
        universal_msg: UniversalMsg,
    },
    /// note this doesn't let the legacy owner be set to None
    UpdateLegacyOwner {
        new_owner: String,
    },
}

impl ExecuteMsg {
    /// serializes the message
    pub fn into_binary(self) -> StdResult<Binary> {
        let msg = self;
        to_binary(&msg)
    }
    /// creates a cosmos_msg sending this struct to the named contract
    pub fn into_cosmos_msg<T: Into<String>, C>(self, contract_addr: T) -> StdResult<CosmosMsg<C>>
    where
        C: Clone + std::fmt::Debug + PartialEq + JsonSchema,
    {
        let msg = self.into_binary()?;
        let execute = WasmMsg::Execute {
            contract_addr: contract_addr.into(),
            msg,
            funds: vec![],
        };
        Ok(execute.into())
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct MigrateMsg {}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    /// Query if the given address can execute the given message
    CanExecute {
        /// The address to check
        address: String,
        /// The message to check
        msg: UniversalMsg,
        funds: Vec<Coin>,
    },
    /// Query the current update delay
    UpdateDelay {},
    /// Query the current legacy owner, if it exists
    LegacyOwner {},
    /// Return all the user's attached gatekeeper contracts
    GatekeeperContracts {},
    AndrHook(AndromedaHook),
    AndrQuery(AndromedaQuery),
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct GatekeeperResponse {
    pub spendlimit_gatekeeper_contract_addr: Option<String>,
    pub delay_gatekeeper_contract_addr: Option<String>,
    pub message_gatekeeper_contract_addr: Option<String>,
    pub sessionkey_gatekeeper_contract_addr: Option<String>,
    pub debt_gatekeeper_contract_addr: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct UserAccount {
    pub legacy_owner: Option<String>,
    pub owner_updates_delay_secs: Option<u64>,
    /// Contract that manages spend limits for permissioned addresses
    pub spendlimit_gatekeeper_contract_addr: Option<String>,
    /// Contract that manages actions which incur a delay
    pub delay_gatekeeper_contract_addr: Option<String>,
    /// Contract that manages message types/fields for permissioned addresses
    pub message_gatekeeper_contract_addr: Option<String>,
    /// Contract that manages session keys
    pub sessionkey_gatekeeper_contract_addr: Option<String>,
    /// Contract that manages this account's debt (for fees)
    pub debt_gatekeeper_contract_addr: Option<String>,
}

pub const ACCOUNT: Item<UserAccount> = Item::new("account");

impl UserAccount {
    pub fn can_execute(
        &self,
        deps: Deps,
        address: String,
        msgs: Vec<UniversalMsg>,
    ) -> Result<CanSpendResponse, ContractError> {
        // vec for future, but right now just first msg in it checked+attached
        if msgs.len() > 1 {
            return Ok(CanSpendResponse {
                can_spend: false,
                reason: "Multi-message txes with permissioned addresss not supported yet"
                    .to_string(),
            });
        }

        // if user is owner, check debt and delay
        if let Some(addy) = self.legacy_owner.clone() {
            if addy == address {
                println!("\x1b[3m\tCalling address is user account legacy owner.\x1b[0m");
                return self.can_owner_execute(deps, msgs[0].clone());
            }
        } else if ADOContract::default().is_owner_or_operator(deps.storage, address.as_str())?
        // probably todo: operators can have restrictions here
        {
            println!("\x1b[3m\tCalling address is user account owner.\x1b[0m");
            return self.can_owner_execute(deps, msgs[0].clone());
        }
        println!("\x1b[3m\tCalling address is not an owner.\x1b[0m");
        self.can_nonowner_execute(deps, address, msgs[0].clone())
    }

    pub fn can_owner_execute(
        &self,
        _deps: Deps,
        _msg: UniversalMsg,
    ) -> Result<CanSpendResponse, ContractError> {
        // check delay
        // How delay transactions are handled is a bit TBD; the Delay Gatekeeper doesn't
        // have transaction analysis capabilities at the moment, and those would seem
        // redundant with the submsg+funds analysis User Account can do and the deep
        // message analysis Spendlimit Gatekeeper can do.
        //
        // For now, we will force admin updates to be delayed, but that specific case
        // is implemented in the actual `propose_update_owner` handling, not here.
        Ok(CanSpendResponse {
            can_spend: true,
            reason: "caller is owner with no debt".to_string(),
        })
        // check debt (once done)
    }

    pub fn dispatch_with_delay(&self, msg: CosmosMsg) -> Result<CosmosMsg, ContractError> {
        let unwrapped_contract_addr =
            self.delay_gatekeeper_contract_addr
                .clone()
                .ok_or(ContractError::Std(StdError::GenericErr {
                    msg: "No known delay gatekeeper address".to_string(),
                }))?;
        let delay_msg = crate::gatekeeper_delay::ExecuteMsg::BeginTransaction {
            message: msg,
            delay_seconds: self.owner_updates_delay_secs.unwrap_or(0),
        };
        Ok(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: unwrapped_contract_addr,
            msg: to_binary(&delay_msg)?,
            funds: vec![],
        }))
    }

    // hardcode for now
    pub fn is_authorized_permissioned_address_contract(&self, addr: String) -> bool {
        match addr {
            val if val == *"juno18c5uecrztn4rqakm23fskusasud7s8afujnl8yu54ule2kak5q4sdnvcz4" => {
                true //DRINK
            }
            val if val == *"juno1x5xz6wu8qlau8znmc60tmazzj3ta98quhk7qkamul3am2x8fsaqqcwy7n9" => {
                true //BOTTLE
            }
            _ => false,
        }
    }

    pub fn can_nonowner_execute(
        &self,
        deps: Deps,
        address: String,
        msg: UniversalMsg,
    ) -> Result<CanSpendResponse, ContractError> {
        // Is this an admin-strength session key?
        println!(
            "\x1b[3m\tChecking if address `{}` is admin session key...\x1b[0m",
            address
        );
        if self.is_admin_session_key(deps, address.clone(), msg.clone())? {
            println!("\x1b[3m\tYes, address `{}` is session key.\x1b[0m", address);
            return Ok(CanSpendResponse {
                can_spend: true,
                reason: "Sender is active admin session key".to_string(),
            });
        } else {
            println!("\x1b[3m\tNo, not a session key.\x1b[0m");
        }

        // check for blanket authorizations ("any permissioned address can spend this")
        // usefulness TBD, but good for ensuring some low-value utility or event token
        // is easily and relatively cheaply used.
        //
        // Note that funds cannot be attached, or this might be a way to circumvent
        // restrictions.
        if let UniversalMsg::Legacy(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr,
            msg: _,
            funds,
        })) = msg.clone()
        {
            let empty_funds: Vec<Coin> = vec![];
            if funds == empty_funds
                && self.is_authorized_permissioned_address_contract(contract_addr)
            {
                return Ok(CanSpendResponse {
                    can_spend: true,
                    reason: "Active permissioned address spending blanket-authorized token"
                        .to_string(),
                });
            }
        }
        println!("\x1b[3m\tNo blanket authorizations apply. Checking if tx uses funds...\x1b[0m");

        // check if TX is using funds at all. (This way we know whether
        // to run funds and debt checks)

        // `spend_limit_authorization_rider` allows certain message types
        // (specifically BankMsg::Send and WasmMsg::Execute(Cw20Transfer)
        // to pass message gatekeeper, if applicable, if the permissioned address
        // has an active spend limit
        let mut spend_limit_authorization_rider = false;
        println!("\x1b[3m\tAnalyzing message: \x1b[90m{:#?}\x1b[0m", msg);
        let funds: Vec<Coin> = match msg.clone() {
            //strictly speaking cw20 spend limits not supported yet, unless blanket authorized.
            //As kludge, send/transfer is blocked if debt exists. Otherwise, depends on
            //authorization.
            UniversalMsg::Legacy(cosmos_msg) => {
                match cosmos_msg.clone() {
                    CosmosMsg::Wasm(WasmMsg::Execute {
                        contract_addr: _,
                        msg: _,
                        funds,
                    }) => {
                        let mut processed_msg = PendingSubmsg {
                            msg: cosmos_msg,
                            contract_addr: None,
                            binarymsg: None,
                            funds: vec![],
                            ty: SubmsgType::Unknown,
                        };
                        processed_msg.add_funds(funds.to_vec());
                        let msg_type = processed_msg.process_and_get_msg_type();
                        if let SubmsgType::ExecuteWasm(WasmmsgType::Cw20Transfer) = msg_type {
                            spend_limit_authorization_rider = true;
                        }
                        // can't immediately pass but can proceed to fund checking
                        funds
                    }
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: _,
                        amount,
                    }) => {
                        spend_limit_authorization_rider = true;
                        amount
                    }
                    CosmosMsg::Staking(StakingMsg::Delegate {
                        validator: _,
                        amount,
                    }) => {
                        vec![amount]
                    }
                    CosmosMsg::Custom(_) => {
                        return Ok(CanSpendResponse {
                            can_spend: false,
                            reason: "Custom CosmosMsg not yet supported".to_string(),
                        });
                    }
                    CosmosMsg::Distribution(_) => {
                        return Ok(CanSpendResponse {
                            can_spend: false,
                            reason: "Distribution CosmosMsg not yet supported".to_string(),
                        });
                    }
                    _ => {
                        return Ok(CanSpendResponse {
                            can_spend: false,
                            reason: "This CosmosMsg type not yet supported".to_string(),
                        });
                    }
                }
            }
            UniversalMsg::Andromeda(_) => {
                vec![]
            } // not at all supported yet
        };

        let empty_funds: Vec<Coin> = vec![];
        if funds != empty_funds {
            println!("\x1b[3m\tYes, this TX uses funds.\x1b[0m");

            // if so...
            // we must have a spend controller
            // and must be within spend limit
            ensure!(
                self.spend_is_ok(deps, address.clone(), funds.clone())?,
                ContractError::CannotSpendMoreThanLimit(
                    funds[0].amount.to_string(),
                    funds[0].denom.clone()
                )
            );

            // also...
            // check that debt is repaid: otherwise, attach a debt repay msg
        } else {
            println!("\x1b[3m\tNo funds used or attached in this transaction.\x1b[0m");
        }

        println!("\x1b[3m\tCheck that message is authorized...\x1b[0m");
        println!(
            "\x1b[3m\tSpend limit authorization rider is: {}\x1b[0m",
            spend_limit_authorization_rider
        );

        // We need to have an authorization by message type, except
        // that "spend" authorization comes with implicit inclusion of
        // BankMsg and cw20 Transfer (but not implicit inclusion of Send,
        // which can trigger contracts)
        ensure!(
            spend_limit_authorization_rider || self.message_is_ok(deps, address, msg)?,
            ContractError::Unauthorized {}
        );

        Ok(CanSpendResponse {
            can_spend: true,
            reason: "all checks passed".to_string(),
        })
    }

    pub fn is_admin_session_key(
        &self,
        deps: Deps,
        sender: String,
        msg: UniversalMsg,
    ) -> Result<bool, StdError> {
        if let Some(contract_addr) = self.sessionkey_gatekeeper_contract_addr.clone() {
            let query_msg: SessionkeyQueryMsg = CanExecute {
                sender,
                message: msg,
            };
            println!("Inter-contract query: \x1b[1;34mUser Account\x1b[0m querying \x1b[1;34mSessionkey Gatekeeper\x1b[0m");
            let query_response: Result<CanExecuteResponse, StdError> =
                deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
                    contract_addr,
                    msg: to_binary(&query_msg)?,
                }));
            match query_response {
                Err(_e) => Ok(false),
                Ok(response) => Ok(response.can_execute),
            }
        } else {
            Ok(false)
        }
    }

    pub fn spend_is_ok(
        &self,
        deps: Deps,
        sender: String,
        funds: Vec<Coin>,
    ) -> Result<bool, ContractError> {
        if let Some(contract_addr) = self.spendlimit_gatekeeper_contract_addr.clone() {
            let query_msg: SpendlimitQueryMsg = CanSpend { sender, funds };
            println!("Inter-contract query: \x1b[1;34mUser Account\x1b[0m querying \x1b[1;34mSpendlimit Gatekeeper\x1b[0m");
            let query_response: CanSpendResponse =
                deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
                    contract_addr,
                    msg: to_binary(&query_msg)?,
                }))?;
            Ok(query_response.can_spend)
        } else {
            Ok(false)
        }
    }

    pub fn message_is_ok(
        &self,
        deps: Deps,
        sender: String,
        msg: UniversalMsg,
    ) -> Result<bool, ContractError> {
        if let Some(contract_addr) = self.message_gatekeeper_contract_addr.clone() {
            let query_msg: MessageQueryMsg = CheckTransaction { msg, sender };
            println!("Inter-contract query: \x1b[1;34mAsset Unifer\x1b[0m querying \x1b[1;34mMessage Gatekeeper\x1b[0m");
            let query_response: AuthorizationsResponse =
                deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
                    contract_addr,
                    msg: to_binary(&query_msg)?,
                }))?;
            match query_response.authorizations.len() {
                0 => Ok(false),
                _ => Ok(true),
            }
        } else {
            Ok(false)
        }
    }
}
