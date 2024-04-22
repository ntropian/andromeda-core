pub mod economics_msg;
// pub mod reply;
// pub mod testing;
pub mod adodb;
pub mod economics;
pub mod kernel;
pub mod mock_builder;
pub mod vfs;

#[cfg(not(target_arch = "wasm32"))]
pub mod mock;
#[cfg(not(target_arch = "wasm32"))]
pub mod mock_contract;
#[cfg(not(target_arch = "wasm32"))]
pub use adodb::MockADODB;
#[cfg(not(target_arch = "wasm32"))]
pub use economics::MockEconomics;
#[cfg(not(target_arch = "wasm32"))]
pub use kernel::MockKernel;
#[cfg(not(target_arch = "wasm32"))]
pub use mock::MockAndromeda;
#[cfg(not(target_arch = "wasm32"))]
pub use mock_contract::MockADO;
#[cfg(not(target_arch = "wasm32"))]
pub use mock_contract::MockContract;
#[cfg(not(target_arch = "wasm32"))]
pub use vfs::MockVFS;
