mod local;
mod store;

pub use local::LocalObjectStore;
pub use store::ObjectStore;

#[cfg(test)]
mod tests;
