use ado_base::ADOContract;
use andromeda_modules::{
    gatekeeper_common::is_legacy_owner,
    permissioned_address::{PermissionedAddress, PermissionedAddressParams},
    sourced_coin::{get_admin_sourced_coin, SourcedCoins},
};
//use cw_multi_test::Contract;
use cosmwasm_std::{Addr, Coin, Deps, StdResult, Timestamp};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cw_storage_plus::Item;

use crate::{contract::check_owner, error::ContractError as CustomError};

pub const STATE: Item<State> = Item::new("state");

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, JsonSchema, Debug)]
pub struct State {
    pub permissioned_addresses: Vec<PermissionedAddress>,
    pub asset_unifier_contract: String,
}

impl State {
    pub fn is_active_permissioned_address(&self, addr: Addr) -> StdResult<bool> {
        let this_wallet_opt: Option<&PermissionedAddress> = self
            .permissioned_addresses
            .iter()
            .find(|a| a.address() == Some(addr.to_string()));
        match this_wallet_opt {
            None => Ok(false),
            Some(_) => Ok(true),
        }
    }

    pub fn upsert_permissioned_address(
        &mut self,
        permissioned_address: PermissionedAddressParams,
        beneficiary: bool,
    ) {
        self.permissioned_addresses
            .iter()
            .position(|addy| match addy.address() {
                None => false,
                Some(stored_addy) => stored_addy == permissioned_address.address,
            });
        self.permissioned_addresses
            .push(PermissionedAddress::new(permissioned_address, beneficiary));
    }

    pub fn rm_permissioned_address(&mut self, doomed_permissioned_address: String) {
        self.permissioned_addresses
            .retain(|wallet| wallet.address() != Some(doomed_permissioned_address.clone()));
    }

    pub fn maybe_get_permissioned_address(
        &self,
        addr: String,
    ) -> Result<&PermissionedAddress, CustomError> {
        let this_wallet_opt: Option<&PermissionedAddress> = self
            .permissioned_addresses
            .iter()
            .find(|a| a.matches_address(addr.clone()));
        match this_wallet_opt {
            None => Err(CustomError::PermissionedAddressDoesNotExist {}),
            Some(wal) => Ok(wal),
        }
    }

    pub fn maybe_get_permissioned_address_mut(
        &mut self,
        addr: String,
    ) -> Result<&mut PermissionedAddress, CustomError> {
        let this_wallet_opt: Option<&mut PermissionedAddress> = self
            .permissioned_addresses
            .iter_mut()
            .find(|a| a.matches_address(addr.clone()));
        match this_wallet_opt {
            None => Err(CustomError::PermissionedAddressDoesNotExist {}),
            Some(wal) => Ok(wal),
        }
    }

    pub fn check_and_update_spend_limits(
        &mut self,
        deps: Deps,
        asset_unifier_contract_address: String,
        current_time: Timestamp,
        addr: String,
        spend: Vec<Coin>,
    ) -> Result<SourcedCoins, CustomError> {
        if check_owner(deps, addr.clone()) {
            return Ok(get_admin_sourced_coin());
        }
        let this_wallet = self.maybe_get_permissioned_address_mut(addr)?;

        // check if we should reset to full spend limit again
        // (i.e. reset time has passed)
        // spend limits run against either active or beneficiaries:
        // you can't use both limits towards the same TX
        let should_reset_beneficiary = this_wallet.should_reset_beneficiary(current_time);
        let should_reset_active = this_wallet.should_reset_active(current_time);
        // try against active spend limit
        let mut cached_error: Result<SourcedCoins, CustomError> =
            Err(CustomError::Std(cosmwasm_std::StdError::GenericErr {
                msg: "Uninitialized cached_error".to_string(),
            }));
        if should_reset_active & this_wallet.is_active_permissioned_address() {
            let new_dt = this_wallet.reset_period(current_time, false);
            match new_dt {
                Ok(()) => {}
                Err(e) => {
                    cached_error = Err(CustomError::CustomError { val: e.to_string() });
                }
            };
        }
        if should_reset_beneficiary && this_wallet.is_beneficiary() {
            let new_dt = this_wallet.reset_period(current_time, true);
            if cached_error == Ok(get_admin_sourced_coin()) {
                match new_dt {
                    Ok(()) => {}
                    Err(e) => {
                        cached_error = Err(CustomError::CustomError { val: e.to_string() });
                    }
                }
            };
        }
        if this_wallet.is_active_permissioned_address() {
            match this_wallet.process_spend_vec(
                deps,
                asset_unifier_contract_address.clone(),
                spend.clone(),
                false,
            ) {
                Ok(coin) => return Ok(coin),
                Err(e) => {
                    cached_error = Err(CustomError::CustomError { val: e.to_string() });
                }
            }
        }
        if this_wallet.is_beneficiary() {
            return this_wallet
                .process_spend_vec(deps, asset_unifier_contract_address, spend, true)
                .map_err(|e| CustomError::CustomError { val: e.to_string() });
        }
        cached_error
    }

    pub fn check_spend_limits(
        &self,
        deps: Deps,
        asset_unifier_contract_address: String,
        current_time: Timestamp,
        addr: String,
        spend: Vec<Coin>,
    ) -> Result<SourcedCoins, CustomError> {
        if check_owner(deps, addr.clone())
        {
            return Ok(get_admin_sourced_coin());
        }
        let this_wallet = self.maybe_get_permissioned_address(addr)?;

        // check if we should reset to full spend limit again
        // (i.e. reset time has passed)
        let cached_result: Result<SourcedCoins, CustomError> =
            if this_wallet.should_reset_active(current_time) {
                println!("should reset active");
                this_wallet
                    .check_spend_vec(
                        deps,
                        asset_unifier_contract_address.clone(),
                        spend.clone(),
                        true,
                        false,
                    )
                    .map_err(|e| CustomError::CustomError { val: e.to_string() })
            } else {
                println!("should not reset active");
                this_wallet
                    .check_spend_vec(
                        deps,
                        asset_unifier_contract_address.clone(),
                        spend.clone(),
                        false,
                        false,
                    )
                    .map_err(|e| CustomError::CustomError { val: e.to_string() })
            };

        match cached_result {
            Err(e) => {
                if this_wallet.should_reset_beneficiary(current_time) {
                    println!("should reset beneficiary");
                    if let Ok(coin) = this_wallet.check_spend_vec(
                        deps,
                        asset_unifier_contract_address,
                        spend,
                        true,
                        false,
                    ) {
                        return Ok(coin);
                    };
                } else if let Ok(coin) = this_wallet.check_spend_vec(
                    deps,
                    asset_unifier_contract_address,
                    spend,
                    false,
                    false,
                ) {
                    println!("should not reset beneficiary");
                    return Ok(coin);
                }
                Err(e)
            }
            Ok(val) => Ok(val),
        }
    }
}
