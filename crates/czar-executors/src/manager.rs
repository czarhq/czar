use anyhow::Result;
use async_trait::async_trait;
#[cfg(any(feature = "cloudrun", feature = "worker"))]
use std::collections::HashMap;
use std::sync::Arc;
#[cfg(feature = "cloudrun")]
use tokio::sync::RwLock;

use czar_core::executor::Executor;
use czar_core::executor_manager::ExecutorManager as ExecutorManagerTrait;
use czar_core::models::{ExecutorType, Workflow};
#[cfg(feature = "worker")]
use czar_core::worker_client::WorkerClient;
use czar_core::worker_registry::{WorkerConfig, worker_metadata_from_workflow};

#[cfg(feature = "cloudrun")]
use crate::cloud_run::CloudRunClient;
#[cfg(feature = "library")]
use crate::library::LibraryExecutor;
#[cfg(feature = "process")]
use crate::process::ProcessExecutor;
#[cfg(feature = "process")]
use crate::python::PythonExecutor;
#[cfg(feature = "wasm")]
use crate::wasm::WasmExecutor;
#[cfg(feature = "worker")]
use crate::worker::WorkerExecutor;

#[cfg(feature = "cloudrun")]
type CloudRunClientCache = Arc<RwLock<HashMap<(String, String), Arc<CloudRunClient>>>>;

pub struct ExecutorManager {
    #[cfg(feature = "process")]
    process_executor: Arc<ProcessExecutor>,
    #[cfg(feature = "process")]
    python_executor: Arc<PythonExecutor>,
    #[cfg(feature = "library")]
    library_executor: Arc<LibraryExecutor>,
    #[cfg(feature = "wasm")]
    wasm_executor: Arc<WasmExecutor>,
    #[cfg(feature = "cloudrun")]
    cloudrun_clients: CloudRunClientCache,
    #[cfg(feature = "worker")]
    worker_tokens: HashMap<String, String>,
}

impl Default for ExecutorManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ExecutorManager {
    pub fn new() -> Self {
        Self {
            #[cfg(feature = "process")]
            process_executor: Arc::new(ProcessExecutor::new(None)),
            // workflow_root is None: the orchestrator stores absolute python_path in
            // compiled task params, so PythonExecutor resolves from those directly.
            #[cfg(feature = "process")]
            python_executor: Arc::new(PythonExecutor::new(None, None)),
            #[cfg(feature = "library")]
            library_executor: Arc::new(LibraryExecutor::new()),
            #[cfg(feature = "wasm")]
            wasm_executor: Arc::new(WasmExecutor::new(None)),
            #[cfg(feature = "cloudrun")]
            cloudrun_clients: Arc::new(RwLock::new(HashMap::new())),
            #[cfg(feature = "worker")]
            worker_tokens: HashMap::new(),
        }
    }

    /// Use the configured workers' bearer tokens when dispatching to them.
    pub fn with_workers(self, workers: &[WorkerConfig]) -> Self {
        #[cfg(feature = "worker")]
        {
            let mut manager = self;
            manager.worker_tokens = workers
                .iter()
                .filter_map(|worker| Some((worker.url.clone(), worker.token.clone()?)))
                .collect();
            manager
        }
        #[cfg(not(feature = "worker"))]
        {
            let _ = workers;
            self
        }
    }
}

#[async_trait]
impl ExecutorManagerTrait for ExecutorManager {
    async fn get_executor(
        &self,
        executor_type: &str,
        workflow: &Workflow,
    ) -> Result<Arc<dyn Executor>> {
        // If this workflow was sourced from a remote worker, route all task
        // execution back to that worker regardless of the task's executor_type.
        if let Some(meta) = worker_metadata_from_workflow(workflow) {
            #[cfg(feature = "worker")]
            {
                let token = self.worker_tokens.get(&meta.worker_url).cloned();
                let client = WorkerClient::new(meta.worker_url).with_token(token);
                return Ok(Arc::new(WorkerExecutor::from_client(Arc::new(client))));
            }
            #[cfg(not(feature = "worker"))]
            {
                let _ = meta;
                anyhow::bail!(
                    "Worker executor is unavailable on this build. Enable the 'worker' feature."
                );
            }
        }

        if executor_type.starts_with("custom:") {
            #[cfg(feature = "process")]
            {
                return Ok(self.process_executor.clone());
            }
            #[cfg(not(feature = "process"))]
            {
                anyhow::bail!(
                    "Custom Python executors are unavailable on this build. Enable the 'process' feature."
                );
            }
        }

        #[cfg(not(feature = "cloudrun"))]
        let _ = workflow;
        let executor_type = ExecutorType::parse(executor_type)
            .ok_or_else(|| anyhow::anyhow!("Unknown executor type: {}", executor_type))?;

        let executor: Arc<dyn Executor> = match executor_type {
            #[cfg(feature = "process")]
            ExecutorType::Container => self.process_executor.clone(),
            #[cfg(not(feature = "process"))]
            ExecutorType::Container => {
                anyhow::bail!(
                    "Container executor is unavailable on this build. Enable the 'process' feature."
                );
            }
            #[cfg(feature = "process")]
            ExecutorType::Process => self.process_executor.clone(),
            #[cfg(not(feature = "process"))]
            ExecutorType::Process => {
                anyhow::bail!(
                    "Process executor is unavailable on this build. Enable the 'process' feature."
                );
            }
            #[cfg(feature = "process")]
            ExecutorType::Python => self.python_executor.clone(),
            #[cfg(not(feature = "process"))]
            ExecutorType::Python => {
                anyhow::bail!(
                    "Python executor is unavailable on this build. Enable the 'process' feature."
                );
            }
            #[cfg(feature = "library")]
            ExecutorType::Library => self.library_executor.clone(),
            #[cfg(not(feature = "library"))]
            ExecutorType::Library => {
                anyhow::bail!(
                    "Library executor is unavailable on this build. Enable the 'library' feature."
                );
            }
            #[cfg(feature = "wasm")]
            ExecutorType::Wasm => self.wasm_executor.clone(),
            #[cfg(not(feature = "wasm"))]
            ExecutorType::Wasm => {
                anyhow::bail!(
                    "The native Wasm executor is unavailable on this build. Enable the 'wasm' feature."
                );
            }
            // CloudRunClient is created dynamically and cached per (project, region) pair,
            // unlike other executors which are static singletons initialized at startup.
            #[cfg(feature = "cloudrun")]
            ExecutorType::CloudRun => {
                let key = (workflow.project.clone(), workflow.region.clone());
                let existing = {
                    let clients = self.cloudrun_clients.read().await;
                    clients.get(&key).cloned()
                };
                if let Some(client) = existing {
                    client
                } else {
                    let client = Arc::new(CloudRunClient::new(key.0.clone(), key.1.clone()).await?);
                    client.clone().start_polling_task();
                    let mut clients = self.cloudrun_clients.write().await;
                    clients.insert(key, client.clone());
                    client
                }
            }
            #[cfg(not(feature = "cloudrun"))]
            ExecutorType::CloudRun => {
                anyhow::bail!(
                    "CloudRun executor is unavailable on this build. Enable the 'cloudrun' feature."
                );
            }
        };

        Ok(executor)
    }
}

#[cfg(test)]
mod tests;
