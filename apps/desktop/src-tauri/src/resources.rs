use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct EngineResources {
    pub node: PathBuf,
    pub entry: PathBuf,
    pub working_directory: PathBuf,
}

impl EngineResources {
    pub fn resolve(app: &tauri::AppHandle) -> Result<Self, String> {
        let executable = std::env::current_exe().map_err(|error| error.to_string())?;
        let bundled = executable
            .parent()
            .is_some_and(|directory| directory.join("../Resources/engine/main.mjs").is_file());
        let resource_directory = if bundled {
            Some(
                tauri::Manager::path(app)
                    .resource_dir()
                    .map_err(|error| error.to_string())?,
            )
        } else {
            None
        };
        Self::resolve_from_executable(resource_directory, &executable)
    }

    fn resolve_from_executable(
        resource_directory: Option<PathBuf>,
        executable: &Path,
    ) -> Result<Self, String> {
        let directory = executable
            .parent()
            .ok_or_else(|| "无法确定应用可执行文件目录。".to_owned())?;
        let bundled_resources = directory.join("../Resources");
        if bundled_resources.join("engine/main.mjs").is_file() {
            let directory =
                resource_directory.ok_or_else(|| "无法确定应用资源目录。".to_owned())?;
            return Ok(Self::bundled(directory));
        }
        if cfg!(debug_assertions) && directory.join(".cargo-lock").is_file() {
            return Ok(Self::development(Path::new(env!("CARGO_MANIFEST_DIR"))));
        }
        Ok(Self::portable(directory.to_path_buf()))
    }

    pub fn development(manifest_directory: &Path) -> Self {
        let workspace = manifest_directory
            .parent()
            .and_then(Path::parent)
            .and_then(Path::parent)
            .expect("desktop manifest is inside apps/desktop")
            .to_path_buf();
        let engine = workspace.join("apps/engine");
        Self {
            node: PathBuf::from(std::env::var_os("NODE").unwrap_or_else(|| "node".into())),
            entry: engine.join("src/main.ts"),
            working_directory: engine,
        }
    }

    pub fn packaged(manifest_directory: &Path) -> Self {
        let workspace = manifest_directory
            .parent()
            .and_then(Path::parent)
            .and_then(Path::parent)
            .expect("desktop manifest is inside apps/desktop")
            .to_path_buf();
        let mut resources = Self::bundled(workspace.join("apps/desktop/src-tauri/resources"));
        resources.node = workspace.join(format!(
            "apps/desktop/src-tauri/resources/runtime/{}",
            if cfg!(windows) {
                "node.exe"
            } else {
                "bin/node"
            }
        ));
        resources
    }

    pub fn bundled(resource_directory: PathBuf) -> Self {
        let engine = resource_directory.join("engine");
        let node = resource_directory.join("runtime").join(if cfg!(windows) {
            "node.exe"
        } else {
            "bin/node"
        });
        Self {
            node,
            entry: engine.join("main.mjs"),
            working_directory: engine,
        }
    }

    pub fn portable(directory: PathBuf) -> Self {
        let resources = directory.join("resources");
        let node = directory.join(if cfg!(windows) { "node.exe" } else { "node" });
        let mut result = Self::bundled(resources);
        result.node = node;
        result
    }

    pub fn custom(node: PathBuf, entry: PathBuf, working_directory: PathBuf) -> Self {
        Self {
            node,
            entry,
            working_directory,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn development_paths_point_at_the_actual_engine_workspace() {
        let resources = EngineResources::development(Path::new(env!("CARGO_MANIFEST_DIR")));
        assert!(resources.entry.ends_with("apps/engine/src/main.ts"));
        assert!(resources.entry.is_file());
        assert!(resources.working_directory.join("package.json").is_file());
    }

    #[test]
    fn bundled_paths_keep_the_runtime_and_engine_under_one_resource_root() {
        let resources = EngineResources::bundled(PathBuf::from("/app/Resources"));
        assert!(resources.node.ends_with(if cfg!(windows) {
            "runtime/node.exe"
        } else {
            "runtime/bin/node"
        }));
        assert_eq!(
            resources.entry,
            PathBuf::from("/app/Resources/engine/main.mjs")
        );
        assert_eq!(
            resources.working_directory,
            PathBuf::from("/app/Resources/engine")
        );
    }

    #[test]
    fn debug_app_bundle_uses_resources_instead_of_the_source_workspace() {
        let temporary =
            std::env::temp_dir().join(format!("sonasub-resource-test-{}", std::process::id()));
        let executable_directory = temporary.join("SonaSub.app/Contents/MacOS");
        let resource_directory = temporary.join("SonaSub.app/Contents/Resources/engine");
        std::fs::create_dir_all(&executable_directory).expect("create test executable directory");
        std::fs::create_dir_all(&resource_directory).expect("create test resource directory");
        std::fs::write(resource_directory.join("main.mjs"), "")
            .expect("create test engine artifact");
        let executable = executable_directory.join("sonasub-desktop");
        let resources = EngineResources::resolve_from_executable(
            Some(temporary.join("SonaSub.app/Contents/Resources")),
            &executable,
        )
        .expect("resolve debug app bundle");
        assert!(resources
            .entry
            .ends_with("SonaSub.app/Contents/Resources/engine/main.mjs"));
        assert!(!resources.entry.ends_with("apps/engine/src/main.ts"));
        std::fs::remove_dir_all(temporary).expect("remove test resource directory");
    }
}
