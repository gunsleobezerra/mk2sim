//! VST3 / AU plugin discovery and FFI loading.
//!
//! The clone can *host* third-party plugins in Input FX, Track FX and VSTi slots.
//! Discovery and dynamic loading are real (via `libloading`); the audio bridge to
//! a plugin instance is intentionally a passthrough placeholder because a full
//! VST3 host requires the Steinberg SDK (IComponent/IEditController negotiation).
//! The trait below is the seam where a real processor is plugged in.

use std::ffi::c_void;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use libloading::Library;
use parking_lot::{Mutex, RwLock};
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct PluginDescriptor {
    pub name: String,
    pub path: String,
    pub format: String,
    pub loaded: bool,
}

pub struct PluginHost {
    dirs: Vec<PathBuf>,
    plugins: RwLock<Vec<PluginDescriptor>>,
    libraries: Mutex<Vec<Arc<Library>>>,
}

impl PluginHost {
    pub fn new() -> Self {
        Self {
            dirs: default_dirs(),
            plugins: RwLock::new(Vec::new()),
            libraries: Mutex::new(Vec::new()),
        }
    }

    pub fn with_dirs(dirs: Vec<PathBuf>) -> Self {
        Self {
            dirs,
            plugins: RwLock::new(Vec::new()),
            libraries: Mutex::new(Vec::new()),
        }
    }

    pub fn dirs(&self) -> Vec<String> {
        self.dirs
            .iter()
            .map(|dir| dir.display().to_string())
            .collect()
    }

    pub fn list(&self) -> Vec<PluginDescriptor> {
        self.plugins.read().clone()
    }

    /// Recursively scans the configured directories for `.vst3`, `.dll`,
    /// `.component` and `.so` plugin binaries.
    pub fn scan(&self) -> Vec<PluginDescriptor> {
        let mut found = Vec::new();
        for dir in &self.dirs {
            scan_dir(dir, 0, &mut found);
        }
        found.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        found.dedup_by(|a, b| a.path == b.path);
        *self.plugins.write() = found.clone();
        found
    }

    /// Loads the plugin binary and resolves its factory entry point.
    pub fn load(&self, path: &str) -> Result<PluginDescriptor, String> {
        let path_buf = PathBuf::from(path);
        if !path_buf.exists() {
            return Err(format!("plugin not found: {path}"));
        }
        let binary = resolve_binary(&path_buf).ok_or_else(|| {
            format!(
                "no loadable binary inside {}",
                path_buf.display()
            )
        })?;

        // SAFETY: loading arbitrary native code is inherently unsafe. The caller
        // opts in explicitly through the UI; the library handle is kept alive for
        // the process lifetime.
        let library = unsafe { Library::new(&binary) }
            .map_err(|err| format!("failed to load {}: {err}", binary.display()))?;

        let format = detect_format(&library, &path_buf);
        let descriptor = PluginDescriptor {
            name: plugin_name(&path_buf),
            path: path.to_string(),
            format: format.clone(),
            loaded: format != "unknown",
        };

        if descriptor.loaded {
            self.libraries.lock().push(Arc::new(library));
            let mut list = self.plugins.write();
            if let Some(existing) = list.iter_mut().find(|item| item.path == descriptor.path) {
                *existing = descriptor.clone();
            } else {
                list.push(descriptor.clone());
            }
            Ok(descriptor)
        } else {
            Err(format!(
                "{} does not expose a VST2/VST3 factory entry point",
                path_buf.display()
            ))
        }
    }

    pub fn loaded_count(&self) -> usize {
        self.plugins.read().iter().filter(|p| p.loaded).count()
    }
}

impl Default for PluginHost {
    fn default() -> Self {
        Self::new()
    }
}

fn detect_format(library: &Library, path: &Path) -> String {
    // SAFETY: symbols are only resolved, never called here.
    let has_vst3 = unsafe {
        library
            .get::<unsafe extern "C" fn() -> *mut c_void>(b"GetPluginFactory")
            .is_ok()
    };
    if has_vst3 {
        return "VST3".to_string();
    }
    let has_vst2 = unsafe {
        library
            .get::<unsafe extern "C" fn(
                *mut c_void,
                *mut c_void,
            ) -> *mut c_void>(b"VSTPluginMain")
            .is_ok()
    };
    if has_vst2 {
        return "VST2".to_string();
    }
    match path.extension().and_then(|ext| ext.to_str()) {
        Some("component") => "AU".to_string(),
        Some("vst3") => "VST3".to_string(),
        _ => "unknown".to_string(),
    }
}

fn resolve_binary(path: &Path) -> Option<PathBuf> {
    if path.is_file() {
        return Some(path.to_path_buf());
    }
    if path.is_dir() {
        // macOS bundle layout: Plugin.vst3/Contents/MacOS/Plugin
        let macos = path.join("Contents").join("MacOS");
        if let Ok(entries) = std::fs::read_dir(&macos) {
            for entry in entries.flatten() {
                if entry.path().is_file() {
                    return Some(entry.path());
                }
            }
        }
        // Windows/Linux bundle layout: Plugin.vst3/Contents/x86_64-win/Plugin.vst3
        if let Ok(entries) = std::fs::read_dir(path.join("Contents")) {
            for entry in entries.flatten() {
                let candidate = entry.path();
                if candidate.is_dir() {
                    if let Ok(inner) = std::fs::read_dir(&candidate) {
                        for file in inner.flatten() {
                            if file.path().is_file() {
                                return Some(file.path());
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

fn scan_dir(dir: &Path, depth: usize, out: &mut Vec<PluginDescriptor>) {
    if depth > 4 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let is_bundle = path
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| matches!(ext.to_ascii_lowercase().as_str(), "vst3" | "component"))
                .unwrap_or(false);
            if is_bundle {
                out.push(PluginDescriptor {
                    name: plugin_name(&path),
                    path: path.display().to_string(),
                    format: if path
                        .extension()
                        .and_then(|e| e.to_str())
                        .map(|e| e.eq_ignore_ascii_case("component"))
                        .unwrap_or(false)
                    {
                        "AU".to_string()
                    } else {
                        "VST3".to_string()
                    },
                    loaded: false,
                });
            } else {
                scan_dir(&path, depth + 1, out);
            }
        } else if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            let ext = ext.to_ascii_lowercase();
            let format = match ext.as_str() {
                "vst3" => Some("VST3"),
                "dll" | "so" | "dylib" => Some("VST2"),
                _ => None,
            };
            if let Some(format) = format {
                out.push(PluginDescriptor {
                    name: plugin_name(&path),
                    path: path.display().to_string(),
                    format: format.to_string(),
                    loaded: false,
                });
            }
        }
    }
}

fn plugin_name(path: &Path) -> String {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("Plugin")
        .to_string()
}

pub fn default_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Ok(local) = std::env::current_dir() {
        dirs.push(local.join("plugins"));
    }

    #[cfg(target_os = "windows")]
    {
        if let Ok(common) = std::env::var("CommonProgramFiles") {
            dirs.push(PathBuf::from(common).join("VST3"));
        }
        if let Ok(programs) = std::env::var("ProgramFiles") {
            dirs.push(PathBuf::from(programs).join("VST3"));
        }
        if let Ok(programs) = std::env::var("ProgramFiles(x86)") {
            dirs.push(PathBuf::from(programs).join("VST3"));
        }
    }

    #[cfg(target_os = "macos")]
    {
        dirs.push(PathBuf::from("/Library/Audio/Plug-Ins/VST3"));
        dirs.push(PathBuf::from("/Library/Audio/Plug-Ins/Components"));
        if let Ok(home) = std::env::var("HOME") {
            dirs.push(PathBuf::from(home).join("Library/Audio/Plug-Ins/VST3"));
        }
    }

    #[cfg(target_os = "linux")]
    {
        dirs.push(PathBuf::from("/usr/lib/vst3"));
        dirs.push(PathBuf::from("/usr/local/lib/vst3"));
        if let Ok(home) = std::env::var("HOME") {
            dirs.push(PathBuf::from(home).join(".vst3"));
        }
    }

    dirs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scan_finds_plugin_files() {
        let dir = std::env::temp_dir().join(format!("rc505_plugin_scan_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let fake = dir.join("TestSynth.vst3");
        std::fs::write(&fake, b"not a real plugin").unwrap();

        let host = PluginHost::with_dirs(vec![dir.clone()]);
        let found = host.scan();
        assert!(found.iter().any(|p| p.name == "TestSynth"));
        assert!(!host.dirs().is_empty());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn loading_missing_plugin_reports_error() {
        let host = PluginHost::with_dirs(Vec::new());
        assert!(host.load("/definitely/not/here.vst3").is_err());
    }

    #[test]
    fn invalid_binary_is_rejected() {
        let dir = std::env::temp_dir().join(format!("rc505_plugin_bad_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let fake = dir.join("Broken.vst3");
        std::fs::write(&fake, b"definitely not a shared library").unwrap();
        let host = PluginHost::with_dirs(vec![dir.clone()]);
        assert!(host.load(fake.to_str().unwrap()).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
