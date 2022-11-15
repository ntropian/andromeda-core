use andromeda_fungible_tokens::cw20_exchange::Sale;
use common::app::AndrAddress;
use cosmwasm_std::Uint128;
use cw_asset::AssetInfo;
use cw_storage_plus::{Item, Map};

pub const TOKEN_ADDRESS: Item<AndrAddress> = Item::new("token_address");
pub const SALE: Map<&str, Sale> = Map::new("sale");
