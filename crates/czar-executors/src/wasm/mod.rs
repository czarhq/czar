mod cli;
mod embedded;
mod executor;
mod support;

pub use executor::WasmExecutor;

#[cfg(test)]
mod tests;
