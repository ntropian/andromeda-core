use cosmwasm_schema::cw_serde;
use cosmwasm_std::{ensure, Addr, Api, Deps, QuerierWrapper, Storage};

use crate::ADOContract;
use andromeda_os::{kernel::QueryMsg as KernelQueryMsg, vfs::QueryMsg as VFSQueryMessage};
use common::error::ContractError;

#[cw_serde]
enum AppQueryMsg {
    ComponentExists { name: String },
}

impl<'a> ADOContract<'a> {
    pub fn get_app_contract(&self, storage: &dyn Storage) -> Result<Option<Addr>, ContractError> {
        Ok(self.app_contract.may_load(storage)?)
    }

    pub(crate) fn validate_andr_addresses(
        &self,
        deps: Deps,
        mut addresses: Vec<String>,
    ) -> Result<(), ContractError> {
        let app_contract = self.get_app_contract(deps.storage)?;
        let kernel_address = self.get_kernel_address(deps.storage);
        match kernel_address {
            Ok(kernel_address) => {
                let vfs_address = self.get_vfs_address(&deps.querier, kernel_address)?;
                ensure!(
                    app_contract.is_some(),
                    ContractError::AppContractNotSpecified {}
                );
                #[cfg(feature = "modules")]
                {
                    let modules = self.load_modules(deps.storage)?;
                    if !modules.is_empty() {
                        let andr_addresses: Vec<String> =
                            modules.into_iter().map(|m| m.address).collect();
                        addresses.extend(andr_addresses);
                    }
                }
                let app_contract = app_contract.unwrap();
                for address in addresses {
                    self.validate_andr_address(
                        deps.api,
                        &deps.querier,
                        address,
                        Some(app_contract.clone()),
                        vfs_address.clone(),
                    )?;
                }
                Ok(())
            }
            Err(_) => {
                for address in addresses {
                    deps.api.addr_validate(&address)?;
                }
                Ok(())
            }
        }
    }

    pub fn validate_andr_address(
        &self,
        api: &dyn Api,
        querier: &QuerierWrapper,
        identifier: String,
        app_contract: Option<Addr>,
        vfs_address: Addr,
    ) -> Result<(), ContractError> {
        // If the address passes this check then it doesn't refer to a app component by
        // name.
        if api.addr_validate(&identifier).is_err() || identifier.contains('/') {
            // Check app contract for component if using local reference
            if identifier.starts_with("./") {
                ensure!(
                    app_contract.is_some(),
                    ContractError::AppContractNotSpecified {}
                );
                ensure!(
                    self.component_exists(querier, identifier.clone(), app_contract.unwrap())?,
                    ContractError::InvalidComponent { name: identifier }
                );
            } else {
                // Otherwise check VFS
                ensure!(
                    self.validate_vfs(querier, identifier, vfs_address)?,
                    ContractError::InvalidAddress {}
                )
            }
        }
        Ok(())
    }

    pub fn get_vfs_address(
        &self,
        querier: &QuerierWrapper,
        kernel_address: Addr,
    ) -> Result<Addr, ContractError> {
        let query = KernelQueryMsg::KeyAddress {
            key: "vfs".to_string(),
        };
        Ok(querier.query_wasm_smart(kernel_address, &query)?)
    }

    /// Checks the given component name against the registered app contract to ensure it exists
    fn component_exists(
        &self,
        querier: &QuerierWrapper,
        name: String,
        app_contract: Addr,
    ) -> Result<bool, ContractError> {
        Ok(querier.query_wasm_smart(app_contract, &AppQueryMsg::ComponentExists { name })?)
    }

    /// Validates a given path agains the VFS
    pub(crate) fn validate_vfs(
        &self,
        querier: &QuerierWrapper,
        path: String,
        vfs_address: Addr,
    ) -> Result<bool, ContractError> {
        let query = VFSQueryMessage::ResolvePath { path };
        let query_resp = querier.query_wasm_smart::<Addr>(vfs_address, &query);
        Ok(query_resp.is_ok())
    }
}
