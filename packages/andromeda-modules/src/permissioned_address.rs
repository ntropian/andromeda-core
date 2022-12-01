use std::convert::TryInto;

use chrono::{Datelike, NaiveDate, NaiveDateTime};
use cosmwasm_std::{Coin, Deps, StdError, StdResult, Timestamp, Uint128};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::sourced_coin::SourcedCoins;
use crate::sources::Sources;

use crate::permissioned_address_error::ContractError;

pub const JUNO_MAINNET_AXLUSDC_IBC: &str =
    "ibc/EAC38D55372F38F1AFD68DF7FE9EF762DCF69F26520643CF3F9D292A738D8034";

/// The `PeriodType` type is used for recurring components, including spend limits.
/// Multiples of `DAYS` and `MONTHS` allow for weekly and yearly recurrence.
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, JsonSchema, Debug)]
pub enum PeriodType {
    DAYS,
    MONTHS,
}

#[allow(dead_code)]
enum CheckType {
    TotalLimit,
    RemainingLimit,
}

/// The `CoinLimit` type is a practically extended `Coin` type
/// that includes a remaining limit.
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, JsonSchema, Debug)]
pub struct CoinLimit {
    pub denom: String,
    pub amount: u64,
    pub limit_remaining: u64,
}

/// The `PermissionedAddress` type allows addresses to trigger actions by this contract
/// under certain conditions. The addresses may or may not be signers: some
/// possible other use cases include dependents, employees or contractors,
/// wealth managers, single-purpose addresses used by a service somewhere,
/// subscriptions or recurring payments, etc.
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, JsonSchema, Debug)]
pub struct PermissionedAddress {
    params: Option<PermissionedAddressParams>,
    /// `dormancy_hours` must have passed since main account took *no* admin-level
    /// action. Permissioned actions currently do not reset dormancy time since
    /// they could be other authorized individuals – potentially we could allow resets
    /// if the actions are not by `owner_signers`.
    beneficiary_params: Option<PermissionedAddressParams>,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, JsonSchema, Debug)]
pub struct PermissionedAddressParams {
    pub address: String,
    /// `cooldown` holds the current reset time for spend limits if a `PermissionedAddres`.
    /// It holds the main account dormancy threshold if `Beneficiary`.
    pub cooldown: u64,
    pub period_type: PeriodType,
    pub period_multiple: u16,
    /// Only one spend limit is expected, dollar-denominated. However, if Beneficiary,
    /// this is taken as a percentage for ANY asset balance, and asset is ignored.
    /// This will be generalized later, but remains this way now to ease contract migration.
    pub spend_limits: Vec<CoinLimit>,
    /// `usdc_denom` is legacy: all spend limits for `PermissionedAddress` entries are
    /// currently processed as USDC-denominated.
    pub usdc_denom: Option<String>,
    /// `default` is not really used currently.
    pub default: Option<bool>,
}

/// Had some trouble implementing Typestates with CosmWasm serializer,
/// so we have a few `if beneficiary` checks throughout instead at this time
impl PermissionedAddress {
    pub fn new(params: PermissionedAddressParams, beneficiary: bool) -> Self {
        if beneficiary {
            Self {
                params: None,
                beneficiary_params: Some(params),
            }
        } else {
            Self {
                params: Some(params),
                beneficiary_params: None,
            }
        }
    }
}

// simple getters
impl PermissionedAddress {
    pub fn matches_address(&self, address: String) -> bool {
        if self.is_beneficiary() {
            self.beneficiary_params.clone().unwrap().address == address
        } else {
            self.params.clone().unwrap().address == address
        }
    }

    pub fn address(&self) -> Option<String> {
        match &self.params {
            Some(params) => Some(params.address.clone()),
            None => self
                .beneficiary_params
                .as_ref()
                .map(|beneficiary_params| beneficiary_params.address.clone()),
        }
    }

    pub fn get_params_clone(&self) -> Option<PermissionedAddressParams> {
        self.params.as_ref().cloned()
    }

    pub fn get_beneficiary_params_clone(&self) -> Option<PermissionedAddressParams> {
        self.beneficiary_params.as_ref().cloned()
    }
}

impl PermissionedAddressParams {
    /// Checks whether the `current_time` is past the `current_period_reset` for
    /// this `PermissionedAddress`, which means that the remaining limit CAN be reset to full.
    /// This function does not actually process the reset; use reset_period()
    ///
    /// # Arguments
    ///
    /// * `current_time` - a Timestamp of the current time (or simulated reset time).
    /// Usually `env.block.time`
    pub fn should_reset(&self, current_time: Timestamp) -> bool {
        current_time.seconds() >= self.cooldown
    }

    /// Sets a new reset time for spending limit for this wallet. This also
    /// resets the limit directly by calling self.reset_limits().
    pub fn reset_period(
        &mut self,
        current_time: Timestamp,
    ) -> Result<PermissionedAddressParams, ContractError> {
        let new_dt = NaiveDateTime::from_timestamp(current_time.seconds() as i64, 0u32);
        // how far ahead we set new current_period_reset to
        // depends on the spend limit period (type and multiple)
        let new_dt: Result<NaiveDateTime, ContractError> = match self.period_type {
            PeriodType::DAYS => {
                let working_dt =
                    new_dt.checked_add_signed(chrono::Duration::days(self.period_multiple as i64));
                match working_dt {
                    Some(dt) => Ok(dt),
                    None => {
                        return Err(ContractError::DayUpdateError("unknown error".to_string()));
                    }
                }
            }
            PeriodType::MONTHS => {
                let working_month = new_dt.month() as u16 + self.period_multiple;
                match working_month {
                    2..=12 => Ok(NaiveDate::from_ymd(new_dt.year(), working_month as u32, 1)
                        .and_hms(0, 0, 0)),
                    13..=268 => {
                        let year_increment: i32 = (working_month / 12u16) as i32;
                        Ok(NaiveDate::from_ymd(
                            new_dt.year() + year_increment,
                            working_month as u32 % 12,
                            1,
                        )
                        .and_hms(0, 0, 0))
                    }
                    _ => Err(ContractError::MonthUpdateError {}),
                }
            }
        };
        self.reset_limits();
        let dt = match new_dt {
            Ok(dt) => dt,
            Err(e) => return Err(ContractError::DayUpdateError(e.to_string())),
        };

        self.cooldown = dt.timestamp() as u64;
        Ok(self.clone())
    }
}

// spending limit time period reset handlers
impl PermissionedAddress {
    pub fn is_active_permissioned_address(&self) -> bool {
        !matches!(self.params, None)
    }

    pub fn is_beneficiary(&self) -> bool {
        !matches!(self.beneficiary_params, None)
    }

    /// Returns true if a spend limit reset is needed
    pub fn should_reset_beneficiary(&self, current_time: Timestamp) -> bool {
        if self.is_beneficiary() {
            self.beneficiary_params
                .clone()
                .unwrap()
                .should_reset(current_time)
        } else {
            false
        }
    }

    pub fn should_reset_active(&self, current_time: Timestamp) -> bool {
        if self.is_active_permissioned_address() {
            self.params.clone().unwrap().should_reset(current_time)
        } else {
            false
        }
    }

    pub fn reset_period(
        &mut self,
        current_time: Timestamp,
        as_beneficiary: bool,
    ) -> Result<(), ContractError> {
        if as_beneficiary && self.is_beneficiary() {
            self.beneficiary_params = Some(
                self.beneficiary_params
                    .clone()
                    .unwrap()
                    .reset_period(current_time)?,
            );
            Ok(())
        } else if self.is_active_permissioned_address() {
            self.params = Some(self.params.clone().unwrap().reset_period(current_time)?);
            Ok(())
        } else {
            Err(ContractError::PermissionedAddressDoesNotExist {})
        }
    }

    pub fn reduce_limit_direct(
        &mut self,
        coin: Coin,
        as_beneficiary: String,
    ) -> Result<(), ContractError> {
        if as_beneficiary == *"true" && self.is_beneficiary() {
            self.beneficiary_params = Some(
                self.beneficiary_params
                    .clone()
                    .unwrap()
                    .reduce_limit_direct(coin)?,
            );
            Ok(())
        } else if self.is_active_permissioned_address() {
            self.params = Some(self.params.clone().unwrap().reduce_limit_direct(coin)?);
            Ok(())
        } else {
            Err(ContractError::PermissionedAddressDoesNotExist {})
        }
    }

    pub fn process_spend_vec(
        &mut self,
        deps: Deps,
        asset_unifier_contract_address: String,
        spend_vec: Vec<Coin>,
        as_beneficiary: bool,
    ) -> Result<SourcedCoins, ContractError> {
        let sourced_coin: SourcedCoins;
        let new_params: PermissionedAddressParams;
        if as_beneficiary && self.is_beneficiary() {
            (sourced_coin, new_params) = self
                .beneficiary_params
                .clone()
                .unwrap()
                .process_spend_vec(deps, asset_unifier_contract_address, spend_vec)?;
            self.beneficiary_params = Some(new_params);
            Ok(sourced_coin)
        } else if self.is_active_permissioned_address() {
            (sourced_coin, new_params) = self.params.clone().unwrap().process_spend_vec(
                deps,
                asset_unifier_contract_address,
                spend_vec,
            )?;
            self.params = Some(new_params);
            Ok(sourced_coin)
        } else {
            Err(ContractError::PermissionedAddressDoesNotExist {})
        }
    }
    /// Pass-through to function in params
    pub fn update_spend_limit(
        &mut self,
        new_limit: CoinLimit,
        beneficiary: String,
    ) -> Result<(), StdError> {
        if beneficiary == *"true" {
            match self.is_beneficiary() {
                false => Err(StdError::GenericErr {
                    msg: "This address is permissioned, but not a beneficiary".to_string(),
                }),
                true => {
                    let updated_params = self
                        .beneficiary_params
                        .clone()
                        .unwrap()
                        .update_spend_limit(new_limit)?;
                    self.beneficiary_params = Some(updated_params);
                    Ok(())
                }
            }
        } else {
            match self.is_active_permissioned_address() {
                false => Err(StdError::GenericErr {
                    msg: "This address has beneficiary permissions only".to_string(),
                }),
                true => {
                    let updated_params =
                        self.params.clone().unwrap().update_spend_limit(new_limit)?;
                    self.params = Some(updated_params);
                    Ok(())
                }
            }
        }
    }

    pub fn should_reset_permissioned_address_limit(
        &self,
        current_time: Timestamp,
    ) -> Result<bool, ContractError> {
        match self.params.clone() {
            Some(params) => Ok(params.should_reset(current_time)),
            None => Err(ContractError::PermissionedAddressDoesNotExist {}),
        }
    }

    pub fn should_reset_beneficiary_limit(
        &self,
        current_time: Timestamp,
    ) -> Result<bool, ContractError> {
        match self.beneficiary_params.clone() {
            Some(beneficiary_params) => Ok(beneficiary_params.should_reset(current_time)),
            None => Err(ContractError::BeneficiaryDoesNotExist {}),
        }
    }

    pub fn check_spend_vec(
        &self,
        deps: Deps,
        asset_unifier_contract_address: String,
        spend_vec: Vec<Coin>,
        should_reset: bool,
        as_beneficiary: bool,
    ) -> Result<SourcedCoins, ContractError> {
        if as_beneficiary && self.is_beneficiary() {
            self.beneficiary_params
                .clone()
                .unwrap()
                .simulate_reduce_limit(
                    deps,
                    spend_vec,
                    asset_unifier_contract_address,
                    should_reset,
                )
                .map(|tuple| tuple.1)
        } else if self.is_active_permissioned_address() {
            self.params
                .clone()
                .unwrap()
                .simulate_reduce_limit(
                    deps,
                    spend_vec,
                    asset_unifier_contract_address,
                    should_reset,
                )
                .map(|tuple| tuple.1)
        } else {
            Err(ContractError::PermissionedAddressDoesNotExist {})
        }
    }
}

// handlers for modifying spend limits (not reset times)
impl PermissionedAddressParams {
    /// Replaces this wallet's current spending limit. Since only single USDC
    /// limit is currently supported, all limits are replaced.
    ///
    pub fn update_spend_limit(&mut self, new_limit: CoinLimit) -> StdResult<Self> {
        self.spend_limits = vec![new_limit];
        Ok(self.clone())
    }

    pub fn reset_limits(&mut self) {
        self.spend_limits[0].limit_remaining = self.spend_limits[0].amount;
    }

    pub fn simulate_reduce_limit(
        &self,
        deps: Deps,
        spend: Vec<Coin>,
        asset_unifier_contract_address: String,
        reset: bool,
    ) -> Result<(u64, SourcedCoins), ContractError> {
        let unconverted_coin = SourcedCoins {
            coins: spend,
            wrapped_sources: Sources { sources: vec![] },
        };
        println!("Converting {} to USDC", unconverted_coin.coins[0]);
        let converted_spend_amt = unconverted_coin
            .convert_to_usdc(deps, asset_unifier_contract_address, false)
            .unwrap();
        // spend can't be bigger than total spend limit
        let limit_to_check = match reset {
            false => self.spend_limits[0].limit_remaining,
            true => self.spend_limits[0].amount,
        };
        println!("Reducing limit of {} by {}", limit_to_check, converted_spend_amt.unified_asset);
        let limit_remaining = limit_to_check
            .checked_sub(converted_spend_amt.unified_asset.amount.u128() as u64)
            .ok_or_else(|| {
                ContractError::CannotSpendMoreThanLimit(
                    converted_spend_amt.unified_asset.amount.to_string(),
                    converted_spend_amt.unified_asset.denom.clone(),
                )
            })?;
        Ok((
            limit_remaining,
            SourcedCoins {
                coins: vec![converted_spend_amt.unified_asset],
                wrapped_sources: converted_spend_amt.sources,
            },
        ))
    }

    pub fn make_usdc_sourced_coin(
        &self,
        amount: Uint128,
        wrapped_sources: Sources,
    ) -> SourcedCoins {
        SourcedCoins {
            coins: vec![Coin {
                amount,
                denom: JUNO_MAINNET_AXLUSDC_IBC.to_string(),
            }],
            wrapped_sources,
        }
    }

    pub fn process_spend_vec(
        &mut self,
        deps: Deps,
        asset_unifier_contract_address: String,
        spend_vec: Vec<Coin>,
    ) -> Result<(SourcedCoins, PermissionedAddressParams), ContractError> {
        let _spend_tally = Uint128::from(0u128);
        let _spend_tally_sources: Sources = Sources { sources: vec![] };

        let all_assets = SourcedCoins {
            coins: spend_vec,
            wrapped_sources: Sources { sources: vec![] },
        };
        let res = all_assets
            .convert_to_usdc(deps, asset_unifier_contract_address.clone(), false)
            .unwrap();
        self.reduce_limit(
            deps,
            asset_unifier_contract_address,
            res.unified_asset.clone(),
        )?;
        Ok((
            SourcedCoins {
                coins: vec![res.unified_asset],
                wrapped_sources: res.sources,
            },
            self.clone(),
        ))
    }

    pub fn reduce_limit(
        &mut self,
        deps: Deps,
        asset_unifier_contract_address: String,
        spend: Coin,
    ) -> Result<SourcedCoins, ContractError> {
        println!("limit starting at {}", self.spend_limits[0].limit_remaining);
        let spend_limit_reduction: (u64, SourcedCoins) =
            self.simulate_reduce_limit(deps, vec![spend], asset_unifier_contract_address, false)?;
        self.spend_limits[0].limit_remaining = spend_limit_reduction.0;
        println!("limit reduced by {:?}", spend_limit_reduction.1);
        println!("limit is now {:?}", self.spend_limits[0].limit_remaining);
        Ok(spend_limit_reduction.1)
    }

    pub fn reduce_limit_direct(
        &mut self,
        limit_reduction: Coin,
    ) -> Result<PermissionedAddressParams, ContractError> {
        // error handling todo, currently panics if overflow
        match self.spend_limits[0]
            .limit_remaining
            .checked_sub(limit_reduction.amount.u128().try_into().unwrap())
        {
            Some(val) => {
                self.spend_limits[0].limit_remaining = val;
                Ok(self.clone())
            }
            None => Err(ContractError::CannotSpendMoreThanLimit(
                limit_reduction.denom,
                limit_reduction.amount.to_string(),
            )),
        }
    }
}

// functions for tests only
#[cfg(test)]
impl PermissionedAddressParams {
    /// Deprecated, will be axed when better spend limit asset/multiasset
    /// handling is implemented.
    pub fn usdc_denom(&self) -> Option<String> {
        self.usdc_denom.clone()
    }

    pub fn set_usdc_denom(&mut self, new_setting: Option<String>) -> StdResult<()> {
        self.usdc_denom = new_setting;
        Ok(())
    }

    pub fn spend_limits(&self) -> Vec<CoinLimit> {
        self.spend_limits.clone()
    }

    pub fn current_period_reset(&self) -> u64 {
        self.cooldown
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, JsonSchema, Debug)]
pub struct PermissionedAddresssResponse {
    pub permissioned_addresses: Vec<PermissionedAddressParams>,
}
