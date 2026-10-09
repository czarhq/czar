use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct UiBundle {
    dist_dir: PathBuf,
}

impl UiBundle {
    pub fn new(dist_dir: PathBuf) -> Result<Self, UiBundleError> {
        if is_ui_bundle_dir(&dist_dir) {
            Ok(Self { dist_dir })
        } else {
            Err(UiBundleError::Missing {
                searched: vec![dist_dir],
            })
        }
    }

    pub fn resolve() -> Result<Self, UiBundleError> {
        let mut searched = Vec::new();

        if let Some(env_path) = std::env::var_os("CZAR_UI_DIST_DIR") {
            let candidate = PathBuf::from(env_path);
            if is_ui_bundle_dir(&candidate) {
                return Ok(Self {
                    dist_dir: candidate,
                });
            }
            searched.push(candidate);
        }

        let manifest_candidate =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../czar-web-react/dist");
        if is_ui_bundle_dir(&manifest_candidate) {
            return Ok(Self {
                dist_dir: manifest_candidate,
            });
        }
        searched.push(manifest_candidate);

        if let Ok(current_dir) = std::env::current_dir() {
            for ancestor in current_dir.ancestors() {
                for suffix in ["crates/czar-web-react/dist", "czar-web-react/dist"] {
                    let candidate = ancestor.join(suffix);
                    if searched.iter().any(|path| path == &candidate) {
                        continue;
                    }
                    if is_ui_bundle_dir(&candidate) {
                        return Ok(Self {
                            dist_dir: candidate,
                        });
                    }
                    searched.push(candidate);
                }
            }
        }

        // Installed from crates.io: the UI is built into the binary.
        if let Some(dir) = extract_embedded() {
            return Ok(Self { dist_dir: dir });
        }

        Err(UiBundleError::Missing { searched })
    }

    pub fn dist_dir(&self) -> &Path {
        &self.dist_dir
    }

    pub fn assets_dir(&self) -> PathBuf {
        self.dist_dir.join("assets")
    }

    pub fn index_html(&self) -> PathBuf {
        self.dist_dir.join("index.html")
    }

    pub fn config_js(&self) -> PathBuf {
        self.dist_dir.join("config.js")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UiBundleError {
    Missing { searched: Vec<PathBuf> },
}

impl UiBundleError {
    pub fn missing_message(&self) -> String {
        match self {
            Self::Missing { searched } => {
                let locations = searched
                    .iter()
                    .map(|path| format!("  - {}", path.display()))
                    .collect::<Vec<_>>()
                    .join("\n");
                format!(
                    "React UI bundle not found. Build it with `just build-frontend` or `pnpm --dir crates/czar-web-react run build`.\nSearched:\n{}",
                    locations
                )
            }
        }
    }
}

impl Default for UiBundleError {
    fn default() -> Self {
        Self::Missing {
            searched: Vec::new(),
        }
    }
}

mod embedded {
    include!(concat!(env!("OUT_DIR"), "/embedded_ui.rs"));
}

/// Write the embedded UI (if this build has one) to a per-version folder in
/// the temp directory, once, and return it.
fn extract_embedded() -> Option<PathBuf> {
    if embedded::FILES.is_empty() {
        return None;
    }
    let dir = std::env::temp_dir().join(format!("czar-ui-{}", embedded::ID));
    if is_ui_bundle_dir(&dir) {
        return Some(dir);
    }
    // Write into a fresh folder, then rename, so two servers starting at once
    // never serve a half-written bundle.
    let staging =
        std::env::temp_dir().join(format!("czar-ui-{}-{}", embedded::ID, std::process::id()));
    for (rel, bytes) in embedded::FILES {
        let path = staging.join(rel);
        std::fs::create_dir_all(path.parent()?).ok()?;
        std::fs::write(&path, bytes).ok()?;
    }
    if std::fs::rename(&staging, &dir).is_err() {
        let _ = std::fs::remove_dir_all(&staging);
    }
    is_ui_bundle_dir(&dir).then_some(dir)
}

fn is_ui_bundle_dir(path: &Path) -> bool {
    path.join("index.html").is_file() && path.join("config.js").is_file()
}
