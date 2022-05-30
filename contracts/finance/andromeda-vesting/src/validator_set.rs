use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Coin, CosmosMsg, DistributionMsg, StakingMsg, StdError, StdResult, Uint128};
use std::cmp::Ordering;
use std::collections::VecDeque;

pub const DEFAULT_WEIGHT: u8 = 10;

#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema)]
pub struct ValidatorResponse {
    pub(crate) address: String,
    pub(crate) staked: Uint128,
    pub(crate) weight: u8,
}

#[derive(Eq, PartialEq, Serialize, Deserialize, Debug, Clone, JsonSchema)]
pub struct Validator {
    pub(crate) address: String,
    pub(crate) staked: u128,
    pub(crate) weight: u8,
}

impl PartialOrd for Validator {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Validator {
    fn cmp(&self, other: &Self) -> Ordering {
        //
        (self.staked.saturating_mul(other.weight as u128))
            .cmp(&(other.staked.saturating_mul(self.weight as u128)))
    }
}

#[derive(Serialize, Debug, Deserialize, Clone, PartialEq, Default, JsonSchema)]
pub struct ValidatorSet {
    validators: VecDeque<Validator>,
}

impl ValidatorSet {
    pub fn to_query_response(&self) -> Vec<ValidatorResponse> {
        self.validators
            .clone()
            .into_iter()
            .map(|v| ValidatorResponse {
                address: v.address,
                staked: Uint128::new(v.staked),
                weight: v.weight,
            })
            .collect()
    }

    pub fn next_to_unbond(&self) -> Option<&Validator> {
        if self.validators.is_empty() {
            return None;
        }
        self.validators.front()
    }

    pub fn remove(&mut self, address: &str, force: bool) -> StdResult<Option<Validator>> {
        let pos = self.exists(address);
        if pos.is_none() {
            return Err(StdError::generic_err(format!(
                "Failed to remove validator: {}, doesn't exist",
                address
            )));
        }

        let val = self.validators.get(pos.unwrap()).ok_or_else(|| {
            StdError::generic_err(format!(
                "Failed to remove validator: {}, failed to get from validator list",
                address
            ))
        })?;

        if !force && val.staked != 0 {
            return Err(StdError::generic_err(format!(
                "Failed to remove validator: {}, you need to undelegate {}uscrt first or set the flag force=true",
                address, val.staked
            )));
        }

        Ok(self.validators.remove(pos.unwrap()))
    }

    pub fn total_staked(&self) -> u128 {
        self.validators.iter().map(|val| val.staked).sum()
    }

    pub fn add(&mut self, address: String, weight: Option<u8>) {
        if self.exists(&address).is_none() {
            self.validators.push_back(Validator {
                address,
                staked: 0,
                weight: weight.unwrap_or(DEFAULT_WEIGHT),
            })
        }
    }

    pub fn change_weight(&mut self, address: &str, weight: Option<u8>) -> StdResult<()> {
        let pos = self.exists(address);
        if pos.is_none() {
            return Err(StdError::generic_err(format!(
                "Failed to remove validator: {}, doesn't exist",
                address
            )));
        }

        let val = self.validators.get_mut(pos.unwrap()).ok_or_else(|| {
            StdError::generic_err(format!(
                "Failed to remove validator: {}, failed to get from validator list",
                address
            ))
        })?;

        val.weight = weight.unwrap_or(DEFAULT_WEIGHT);

        Ok(())
    }

    pub fn unbond(&mut self, to_unbond: u128) -> StdResult<String> {
        if self.validators.is_empty() {
            return Err(StdError::generic_err(
                "Failed to get validator to unbond - validator set is empty",
            ));
        }

        let val = self.validators.front_mut().unwrap();
        val.staked = val.staked.saturating_sub(to_unbond);
        Ok(val.address.clone())
    }

    pub fn stake(&mut self, to_stake: u128) -> StdResult<String> {
        if self.validators.is_empty() {
            return Err(StdError::generic_err(
                "Failed to get validator to stake - validator set is empty",
            ));
        }

        let val = self.validators.back_mut().unwrap();
        val.staked += to_stake;
        Ok(val.address.clone())
    }

    pub fn stake_at(&mut self, address: &str, to_stake: u128) -> StdResult<()> {
        if self.validators.is_empty() {
            return Err(StdError::generic_err(
                "Failed to get validator to stake - validator set is empty",
            ));
        }

        for val in self.validators.iter_mut() {
            if val.address == address {
                val.staked += to_stake;
                return Ok(());
            }
        }

        Err(StdError::generic_err(
            "Failed to get validator to stake - validator not found",
        ))
    }

    pub fn exists(&self, address: &str) -> Option<usize> {
        self.validators.iter().position(|v| v.address == address)
    }

    // call this after every stake or unbond call
    pub fn rebalance(&mut self) {
        if self.validators.len() < 2 {
            return;
        }

        self.validators.make_contiguous().sort_by(|a, b| b.cmp(a));
    }

    pub fn withdraw_rewards_messages(&self, addresses: Option<Vec<String>>) -> Vec<CosmosMsg> {
        if let Some(validators) = addresses {
            self.validators
                .iter()
                .filter(|&val| validators.contains(&val.address) && val.staked > 0)
                .map(|val| withdraw_to_self(&val.address))
                .collect()
        } else {
            self.validators
                .iter()
                .filter(|&val| val.staked > 0)
                .map(|val| withdraw_to_self(&val.address))
                .collect()
        }
    }

    pub fn unbond_all(&self) -> Vec<CosmosMsg> {
        self.validators
            .iter()
            .filter(|&val| val.staked > 0)
            .map(|val| undelegate_msg(&val.address, val.staked))
            .collect()
    }

    pub fn zero(&mut self) {
        if self.validators.is_empty() {
            return;
        }

        for val in self.validators.iter_mut() {
            val.staked = 0;
        }
    }
}

pub fn undelegate_msg(validator: &str, amount: u128) -> CosmosMsg {
    CosmosMsg::Staking(StakingMsg::Undelegate {
        validator: validator.to_string(),
        amount: Coin {
            denom: "uscrt".to_string(),
            amount: Uint128::new(amount),
        },
    })
}

pub fn withdraw_to_self(validator: &str) -> CosmosMsg {
    CosmosMsg::Distribution(DistributionMsg::WithdrawDelegatorReward {
        validator: validator.to_string(),
    })
}
