//! 请求处理器模块
//!
//! 每个子模块对应一个 API 路径前缀：
//! * `user`         — `/v1/users/*`
//! * `item`         — `/v1/items/*`
//! * `space`        — `/v1/spaces/*`
//! * `history`      — `/v1/history/*`
//! * `image`        — `/v1/images/*`
//! * `sync`         — `/v1/sync/*`
//! * `collaborator` — `/v1/{items,spaces}/{id}/collaborators/*`
//! * `category`     — `/v1/categories/*`
//! * `tag`          — `/v1/tags/*`

pub mod category;
pub mod collaborator;
pub mod item;
pub mod space;
pub mod history;
pub mod image;
pub mod sync;
pub mod tag;
pub mod user;
