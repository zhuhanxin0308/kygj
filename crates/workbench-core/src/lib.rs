//! 科研工作台的独立业务层；不依赖窗口、网络服务或前端状态。

pub mod error;
pub mod limits;
pub mod protocol;
pub mod process;
mod schema;
pub mod storage;
pub mod types;
pub mod validation;

pub use error::{CoreError, CoreResult};
