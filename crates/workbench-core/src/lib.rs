//! 科研工作台的独立业务层；不依赖窗口、网络服务或前端状态。

pub mod error;
pub mod limits;
pub mod protocol;
pub mod process;
mod schema;
pub mod storage;
pub mod types;
pub mod validation;
pub mod verification_types;
mod verification;
mod verification_storage;
mod migration;
mod workbench;

pub use error::{CoreError, CoreResult};
pub use workbench::Workbench;
