use std::{
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::Mutex,
    time::Duration,
};

use sha2::{Digest, Sha256};

use crate::config::ConfigStore;

use super::install::{
    group_expected_sources, qwen3_06b_runtime_files, qwen3_17b_runtime_files, qwen3_archive_spec,
    ModelInstallFile, ModelInstallGroup, MODELSCOPE_API, QWEN3_17B_REVISION,
};
use super::registry::ASR_QWEN3_17B_ID;

pub(super) static DOWNLOAD_DIR: Mutex<Option<PathBuf>> = Mutex::new(None);

pub(super) fn download_dir(state: &crate::session::SessionManager) -> Result<PathBuf, String> {
    let configured = DOWNLOAD_DIR
        .lock()
        .map_err(|error| format!("Failed to read the download directory: {error}"))?
        .clone();
    if let Some(directory) = configured {
        return Ok(directory);
    }
    let config_dir = state.config_dir();
    let store = ConfigStore::new(config_dir.join("model-download-dir.json"));
    let loaded = store
        .load_raw()
        .and_then(|text| serde_json::from_str::<PathBuf>(&text).ok())
        .filter(|path| path.is_absolute());
    Ok(loaded.unwrap_or_else(|| state.model_root().join("downloads")))
}

pub(super) fn download_destination(
    state: &crate::session::SessionManager,
    group_id: &str,
) -> Result<PathBuf, String> {
    Ok(download_dir(state)?.join(format!("{group_id}.download")))
}

fn pinned(file: &ModelInstallFile) -> Result<(&str, u64), String> {
    let sha256 = file
        .expected_sha256
        .as_deref()
        .ok_or_else(|| format!("{} has no pinned checksum.", file.relative))?;
    Ok((sha256, file.min_bytes))
}

fn find_source(directory: &Path, source_name: &str) -> Result<PathBuf, String> {
    if let Some(found) = find_nested_file(directory, source_name, 3) {
        return Ok(found);
    }
    Err(format!(
        "{source_name} was not found in {}",
        directory.display()
    ))
}

fn find_nested_file(directory: &Path, name: &str, depth: usize) -> Option<PathBuf> {
    let direct = directory.join(name);
    if direct.is_file() {
        return Some(direct);
    }
    if depth == 0 {
        return None;
    }
    for entry in fs::read_dir(directory).ok()?.flatten() {
        if entry.file_type().is_ok_and(|file_type| file_type.is_dir()) {
            if let Some(found) = find_nested_file(&entry.path(), name, depth - 1) {
                return Some(found);
            }
        }
    }
    None
}

fn install_sources(
    state: &crate::session::SessionManager,
    group: &ModelInstallGroup,
    directory: &Path,
) -> Result<(), String> {
    let expected = group_expected_sources(group);
    for file in &group.files {
        let (sha256, min_bytes) = pinned(file)?;
        let source_name = expected
            .iter()
            .find(|(relative, _)| *relative == file.relative)
            .map(|(_, name)| *name)
            .unwrap_or(file.source_name.as_str());
        let source = find_source(directory, source_name)?;
        let target = state.model_root().join(&file.relative);
        copy_verified(&source, &target, sha256, min_bytes)?;
    }
    Ok(())
}

fn extract_tar_bz2(archive: &Path, target: &Path) -> Result<(), String> {
    fs::create_dir_all(target)
        .map_err(|error| format!("failed to create {}: {error}", target.display()))?;
    let file = File::open(archive)
        .map_err(|error| format!("failed to read {}: {error}", archive.display()))?;
    let decoder = bzip2::read::BzDecoder::new(file);
    let mut archive = tar::Archive::new(decoder);
    archive
        .unpack(target)
        .map_err(|error| format!("failed to extract {}: {error}", target.display()))?;
    Ok(())
}

fn install_archive(
    state: &crate::session::SessionManager,
    group: &ModelInstallGroup,
    archive: &Path,
) -> Result<(), String> {
    if group.download_kind == "file" {
        return install_direct_file(&state.model_root(), group, archive);
    }
    let temporary = archive.with_extension("extracted");
    let _ = fs::remove_dir_all(&temporary);
    let result = if group.download_kind == "tarBz2" {
        extract_tar_bz2(archive, &temporary)
            .and_then(|()| install_sources(state, group, &temporary))
    } else {
        Err(format!(
            "Unsupported archive type `{}`.",
            group.download_kind
        ))
    };
    let _ = fs::remove_dir_all(&temporary);
    result
}

fn install_direct_file(
    root: &Path,
    group: &ModelInstallGroup,
    source: &Path,
) -> Result<(), String> {
    let [file] = group.files.as_slice() else {
        return Err(format!(
            "The `{}` group must contain exactly one file for a direct download.",
            group.id
        ));
    };
    let (sha256, min_bytes) = pinned(file)?;
    copy_verified(source, &root.join(&file.relative), sha256, min_bytes)
}

fn download_file(url: &str, destination: &Path, expected_bytes: u64) -> Result<(), String> {
    if let Ok(metadata) = fs::metadata(destination) {
        if metadata.is_file() && metadata.len() == expected_bytes {
            return Ok(());
        }
    }
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    let temporary = destination.with_extension("part");
    let result: Result<(), String> = (|| {
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(Duration::from_secs(15))
            .timeout_read(Duration::from_secs(30))
            .timeout_write(Duration::from_secs(30))
            .build();
        let response = agent
            .get(url)
            .call()
            .map_err(|error| format!("failed to download {}: {error}", redact_url(url)))?;
        let mut reader = response.into_reader();
        let mut output = File::create(&temporary)
            .map_err(|error| format!("failed to create {}: {error}", temporary.display()))?;
        let mut buffer = [0u8; 128 * 1024];
        loop {
            let read = reader
                .read(&mut buffer)
                .map_err(|error| format!("failed to read {}: {error}", redact_url(url)))?;
            if read == 0 {
                break;
            }
            output
                .write_all(&buffer[..read])
                .map_err(|error| format!("failed to write {}: {error}", temporary.display()))?;
        }
        output
            .sync_all()
            .map_err(|error| format!("failed to flush {}: {error}", temporary.display()))?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result?;
    let actual = fs::metadata(&temporary)
        .map_err(|error| format!("failed to inspect {}: {error}", temporary.display()))?
        .len();
    if actual != expected_bytes {
        let _ = fs::remove_file(&temporary);
        return Err(format!(
            "downloaded {} bytes from {}; expected {expected_bytes}",
            actual,
            redact_url(url)
        ));
    }
    fs::rename(&temporary, destination)
        .map_err(|error| format!("failed to store {}: {error}", destination.display()))?;
    Ok(())
}

fn verified_digest(path: &Path) -> Result<String, String> {
    let mut input =
        File::open(path).map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 128 * 1024];
    loop {
        let read = input
            .read(&mut buffer)
            .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn verify_source_file(source: &Path, sha256: &str, min_bytes: u64) -> Result<(), String> {
    let metadata = fs::metadata(source)
        .map_err(|error| format!("failed to inspect {}: {error}", source.display()))?;
    if metadata.len() < min_bytes {
        return Err(format!(
            "{} is {} bytes; at least {min_bytes} bytes are required.",
            source.display(),
            metadata.len()
        ));
    }
    let actual = verified_digest(source)?;
    if !actual.eq_ignore_ascii_case(sha256) {
        return Err(format!(
            "checksum mismatch for {}: expected {sha256}, got {actual}.",
            source.display()
        ));
    }
    Ok(())
}

fn install_preverified_file(source: &Path, target: &Path) -> Result<(), String> {
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    let temporary = target.with_extension(format!("install-{}-tmp", std::process::id()));
    let result = fs::copy(source, &temporary)
        .map_err(|error| format!("failed to stage {}: {error}", target.display()))
        .and_then(|_| {
            fs::rename(&temporary, target)
                .map_err(|error| format!("failed to install {}: {error}", target.display()))
        });
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn install_verified_file(
    source: &Path,
    target: &Path,
    sha256: &str,
    min_bytes: u64,
) -> Result<(), String> {
    verify_source_file(source, sha256, min_bytes)?;
    install_preverified_file(source, target)
}

fn expand_import_sources(sources: &[String]) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    for source in sources {
        collect_import_files(Path::new(source), &mut files)?;
    }
    files.sort();
    files.dedup();
    Ok(files)
}

fn collect_import_files(path: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    let metadata = fs::metadata(path)
        .map_err(|error| format!("failed to inspect {}: {error}", path.display()))?;
    if metadata.is_file() {
        files.push(path.to_owned());
        return Ok(());
    }
    if !metadata.is_dir() {
        return Err(format!(
            "{} is neither a file nor a directory.",
            path.display()
        ));
    }
    let entries = fs::read_dir(path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    for entry in entries {
        let entry = entry.map_err(|error| format!("failed to read {}: {error}", path.display()))?;
        let file_type = entry
            .file_type()
            .map_err(|error| format!("failed to inspect {}: {error}", entry.path().display()))?;
        if file_type.is_symlink() {
            continue;
        }
        collect_import_files(&entry.path(), files)?;
    }
    Ok(())
}

fn urlencode(path: &str) -> String {
    let mut encoded = String::new();
    for byte in path.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'/' | b'~') {
            encoded.push(byte as char);
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    encoded
}

fn redact_url(url: &str) -> &str {
    url.rsplit_once('/').map(|(_, name)| name).unwrap_or(url)
}

fn zstd_to_memory(path: &Path) -> Result<Vec<u8>, String> {
    let data =
        fs::read(path).map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    zstd::decode_all(data.as_slice())
        .map_err(|error| format!("failed to decompress {}: {error}", path.display()))
}

pub(super) fn download_group(
    state: &crate::session::SessionManager,
    group: &ModelInstallGroup,
) -> Result<(), String> {
    let destination = download_destination(state, &group.id)?;
    fs::create_dir_all(&destination)
        .map_err(|error| format!("failed to create {}: {error}", destination.display()))?;

    if let Some((archive_name, archive_sha256)) = qwen3_archive_spec(&group.id) {
        let archive = destination.join(archive_name);
        download_file(&group.download_url, &archive, group.download_bytes)?;
        let actual = verified_digest(&archive)?;
        if !actual.eq_ignore_ascii_case(archive_sha256) {
            let _ = fs::remove_file(&archive);
            return Err(format!(
                "checksum mismatch for {}: expected {archive_sha256}, got {actual}.",
                archive.display()
            ));
        }
        let extracted = archive.with_extension("extracted");
        let _ = fs::remove_dir_all(&extracted);
        let result = extract_tar_bz2(&archive, &extracted).and_then(|()| {
            let root = extracted.join(group.version.as_str());
            for (relative, source_name) in qwen3_06b_runtime_files() {
                let file = group
                    .files
                    .iter()
                    .find(|file| file.relative == *relative)
                    .ok_or_else(|| format!("No checksum registered for {relative}."))?;
                let (sha256, min_bytes) = pinned(file)?;
                let source = find_source(&root, source_name)?;
                install_verified_file(
                    &source,
                    &state.model_root().join(relative),
                    sha256,
                    min_bytes,
                )?;
            }
            Ok(())
        });
        let _ = fs::remove_dir_all(&extracted);
        return result;
    }

    if group.id == ASR_QWEN3_17B_ID {
        for (source_path, file_name, size, sha256) in qwen3_17b_runtime_files() {
            let relative = format!("sherpa/{}/{}", group.id, file_name);
            let installed = state.model_root().join(&relative);
            if let Ok(actual) = verified_digest(&installed) {
                if actual.eq_ignore_ascii_case(sha256) {
                    continue;
                }
            }
            let url = format!(
                "{MODELSCOPE_API}?Revision={QWEN3_17B_REVISION}&FilePath={}",
                urlencode(source_path)
            );
            let archive = destination.join(file_name);
            download_file(&url, &archive, *size)?;
            let actual = verified_digest(&archive)?;
            if !actual.eq_ignore_ascii_case(sha256) {
                let _ = fs::remove_file(&archive);
                return Err(format!(
                    "checksum mismatch for {}: expected {sha256}, got {actual}.",
                    relative
                ));
            }
            let target_dir = if source_path.starts_with("tokenizer/") {
                "tokenizer"
            } else {
                ""
            };
            let target = if target_dir.is_empty() {
                installed
            } else {
                state
                    .model_root()
                    .join(format!("sherpa/{}/{target_dir}", group.id))
                    .join(file_name)
            };
            install_verified_file(&archive, &target, sha256, *size)?;
        }
        return Ok(());
    }

    if group.download_kind == "bergamot" {
        for (index, file) in group.files.iter().enumerate() {
            let (sha256, min_bytes) = pinned(file)?;
            let installed = state.model_root().join(&file.relative);
            if let Ok(bytes) = fs::read(&installed) {
                let actual = format!("{:x}", Sha256::digest(&bytes));
                if (bytes.len() as u64) >= min_bytes && actual.eq_ignore_ascii_case(sha256) {
                    continue;
                }
            }
            let url = group
                .download_urls
                .get(index)
                .ok_or_else(|| format!("No download URL for {}.", file.relative))?;
            let expected_bytes = group
                .download_bytes_each
                .get(index)
                .copied()
                .ok_or_else(|| format!("No download size for {}.", file.relative))?;
            let archive = destination.join(&file.source_name);
            download_file(url, &archive, expected_bytes)?;
            let bytes = zstd_to_memory(&archive)?;
            if (bytes.len() as u64) < min_bytes {
                return Err(format!(
                    "{} is {} bytes; at least {min_bytes} bytes are required.",
                    file.relative,
                    bytes.len()
                ));
            }
            let actual = format!("{:x}", Sha256::digest(&bytes));
            if !actual.eq_ignore_ascii_case(sha256) {
                return Err(format!(
                    "checksum mismatch for {}: expected {sha256}, got {actual}.",
                    file.relative
                ));
            }
            let target = state.model_root().join(&file.relative);
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)
                    .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
            }
            let temporary = target.with_extension(format!("download-{}-tmp", std::process::id()));
            fs::write(&temporary, &bytes)
                .map_err(|error| format!("failed to write {}: {error}", temporary.display()))?;
            fs::rename(&temporary, &target)
                .map_err(|error| format!("failed to install {}: {error}", target.display()))?;
        }
        return Ok(());
    }

    let archive = destination.join(format!("{}.{}", group.id, group.download_kind));
    download_file(&group.download_url, &archive, group.download_bytes)?;
    install_archive(state, group, &archive)
}

pub(super) fn import_group(
    state: &crate::session::SessionManager,
    group: &ModelInstallGroup,
    sources: &[String],
) -> Result<(), String> {
    let mut archives = Vec::new();
    let mut files = Vec::new();
    let mut installed = 0usize;
    for path in expand_import_sources(sources)? {
        if path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension == "bz2")
        {
            archives.push(path);
        } else {
            files.push(path);
        }
    }
    if group.id == ASR_QWEN3_17B_ID {
        let mut selected = Vec::new();
        let mut missing = Vec::new();
        for (source_path, file_name, size, sha256) in qwen3_17b_runtime_files() {
            let matches = archives
                .iter()
                .chain(files.iter())
                .find(|path| path.file_name().and_then(|name| name.to_str()) == Some(*file_name));
            let Some(source) = matches else {
                missing.push(*file_name);
                continue;
            };
            let target_dir = if source_path.starts_with("tokenizer/") {
                "tokenizer"
            } else {
                ""
            };
            let relative = if target_dir.is_empty() {
                format!("sherpa/{}/{}", group.id, file_name)
            } else {
                format!("sherpa/{}/{target_dir}/{file_name}", group.id)
            };
            verify_source_file(source, sha256, *size)?;
            selected.push((source.clone(), state.model_root().join(relative)));
        }
        if !missing.is_empty() {
            return Err(format!(
                "Select all runtime files for this model, including the tokenizer files. Missing: {}.",
                missing.join(", ")
            ));
        }
        for (source, target) in selected {
            install_preverified_file(&source, &target)?;
        }
        return Ok(());
    }
    if let Some((archive_name, archive_sha256)) = qwen3_archive_spec(&group.id) {
        if !archives.is_empty() {
            let [archive] = archives.as_slice() else {
                return Err(format!(
                    "Select only the {archive_name} archive for this model."
                ));
            };
            let actual = verified_digest(archive)?;
            if !actual.eq_ignore_ascii_case(archive_sha256) {
                return Err(format!(
                    "checksum mismatch for {}: expected {archive_sha256}, got {actual}.",
                    archive.display()
                ));
            }
            let extracted = archive.with_extension("extracted");
            let _ = fs::remove_dir_all(&extracted);
            let result = extract_tar_bz2(archive, &extracted).and_then(|()| {
                let root = extracted.join(group.version.as_str());
                for (relative, source_name) in qwen3_06b_runtime_files() {
                    let file = group
                        .files
                        .iter()
                        .find(|file| file.relative == *relative)
                        .ok_or_else(|| format!("No checksum registered for {relative}."))?;
                    let (sha256, min_bytes) = pinned(file)?;
                    let source = find_source(&root, source_name)?;
                    install_verified_file(
                        &source,
                        &state.model_root().join(relative),
                        sha256,
                        min_bytes,
                    )?;
                }
                Ok(())
            });
            let _ = fs::remove_dir_all(&extracted);
            return result;
        }

        let mut selected = Vec::new();
        let mut missing = Vec::new();
        for (relative, source_name) in qwen3_06b_runtime_files() {
            let Some(source) = files
                .iter()
                .find(|path| path.file_name().and_then(|name| name.to_str()) == Some(*source_name))
            else {
                missing.push(*source_name);
                continue;
            };
            let file = group
                .files
                .iter()
                .find(|file| file.relative == *relative)
                .ok_or_else(|| format!("No checksum registered for {relative}."))?;
            let (sha256, min_bytes) = pinned(file)?;
            verify_source_file(source, sha256, min_bytes)?;
            selected.push((source.clone(), state.model_root().join(relative)));
        }
        if !missing.is_empty() {
            return Err(format!(
                "Select the archive or all extracted runtime files for this model. Missing: {}.",
                missing.join(", ")
            ));
        }
        for (source, target) in selected {
            install_preverified_file(&source, &target)?;
        }
        return Ok(());
    }
    if !archives.is_empty() {
        for archive in archives {
            install_archive(state, group, &archive)?;
            installed += 1;
        }
    }
    for source in files {
        let name = source
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| format!("{} has no file name.", source.display()))?;
        let Some(file) = group
            .files
            .iter()
            .find(|file| is_import_name(group, file, name))
        else {
            continue;
        };
        let (sha256, min_bytes) = pinned(file)?;
        let bytes = if group.download_kind == "bergamot" {
            zstd_to_memory(&source)?
        } else {
            fs::read(&source)
                .map_err(|error| format!("failed to read {}: {error}", source.display()))?
        };
        if (bytes.len() as u64) < min_bytes {
            return Err(format!(
                "{} is {} bytes; at least {min_bytes} bytes are required.",
                file.relative,
                bytes.len()
            ));
        }
        let actual = format!("{:x}", Sha256::digest(&bytes));
        if !actual.eq_ignore_ascii_case(sha256) {
            return Err(format!(
                "checksum mismatch for {}: expected {sha256}, got {actual}.",
                file.relative
            ));
        }
        let target = state.model_root().join(&file.relative);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
        }
        let temporary = target.with_extension(format!("import-{}-tmp", std::process::id()));
        fs::write(&temporary, &bytes)
            .map_err(|error| format!("failed to write {}: {error}", temporary.display()))?;
        fs::rename(&temporary, &target)
            .map_err(|error| format!("failed to install {}: {error}", target.display()))?;
        installed += 1;
    }
    if installed == 0 {
        return Err(
            "None of the selected files match this model. Select its official archive or the runtime files listed on its card."
                .to_owned(),
        );
    }
    Ok(())
}

fn is_import_name(group: &ModelInstallGroup, file: &ModelInstallFile, name: &str) -> bool {
    if group.download_kind == "bergamot" {
        return file.source_name == name
            || Path::new(&file.relative)
                .file_name()
                .and_then(|file_name| file_name.to_str())
                == Some(name);
    }
    group_expected_sources(group)
        .iter()
        .find(|(relative, _)| *relative == file.relative)
        .is_some_and(|(_, source_name)| *source_name == name)
        || file.source_name == name
}

fn copy_verified(
    source: &Path,
    target: &Path,
    expected_sha256: &str,
    min_bytes: u64,
) -> Result<(), String> {
    let mut input = File::open(source)
        .map_err(|error| format!("failed to read {}: {error}", source.display()))?;
    let metadata = input
        .metadata()
        .map_err(|error| format!("failed to inspect {}: {error}", source.display()))?;
    if metadata.len() < min_bytes {
        return Err(format!(
            "{} is {} bytes; at least {min_bytes} bytes are required.",
            source.display(),
            metadata.len()
        ));
    }
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    let temporary = target.with_extension(format!("install-{}-tmp", std::process::id()));
    let mut hasher = Sha256::new();
    {
        let mut output = File::create(&temporary)
            .map_err(|error| format!("failed to create {}: {error}", temporary.display()))?;
        let mut buffer = [0u8; 64 * 1024];
        loop {
            let read = input
                .read(&mut buffer)
                .map_err(|error| format!("failed to read {}: {error}", source.display()))?;
            if read == 0 {
                break;
            }
            hasher.update(&buffer[..read]);
            output
                .write_all(&buffer[..read])
                .map_err(|error| format!("failed to write {}: {error}", temporary.display()))?;
        }
        output
            .sync_all()
            .map_err(|error| format!("failed to flush {}: {error}", temporary.display()))?;
    }
    let actual = format!("{:x}", hasher.finalize());
    if !actual.eq_ignore_ascii_case(expected_sha256) {
        let _ = fs::remove_file(&temporary);
        return Err(format!(
            "checksum mismatch for {}: expected {expected_sha256}, got {actual}.",
            source.display()
        ));
    }
    fs::rename(&temporary, target)
        .map_err(|error| format!("failed to install {}: {error}", target.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        download_file, expand_import_sources, find_source, install_direct_file, is_import_name,
        verify_source_file, ModelInstallFile, ModelInstallGroup,
    };
    use std::fs;
    use std::io::{Read, Write};

    use sha2::{Digest, Sha256};

    #[test]
    fn download_file_streams_the_expected_payload() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let payload = b"sonasub-model-download".to_vec();
        let served = payload.clone();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0u8; 1024];
            let _ = stream.read(&mut request);
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                served.len()
            )
            .unwrap();
            stream.write_all(&served).unwrap();
        });
        let directory =
            std::env::temp_dir().join(format!("sonasub-download-test-{}", std::process::id()));
        fs::create_dir_all(&directory).unwrap();
        let destination = directory.join("payload.bin");
        download_file(
            &format!("http://{address}/payload.bin"),
            &destination,
            payload.len() as u64,
        )
        .unwrap();
        server.join().unwrap();
        assert_eq!(fs::read(&destination).unwrap(), payload);
        let _ = fs::remove_file(&destination);
        let _ = fs::remove_dir(&directory);
    }

    #[test]
    fn direct_download_installs_a_single_file() {
        let payload = b"sonasub-single-file-model".to_vec();
        let sha256 = format!("{:x}", Sha256::digest(&payload));
        let group = ModelInstallGroup {
            id: "direct-test".to_owned(),
            label: "Direct test".to_owned(),
            version: "test".to_owned(),
            category: "supporting".to_owned(),
            upstream: "https://example.invalid".to_owned(),
            download_kind: "file".to_owned(),
            download_url: "https://example.invalid/model.onnx".to_owned(),
            download_bytes: payload.len() as u64,
            download_urls: Vec::new(),
            download_bytes_each: Vec::new(),
            archive_sha256: None,
            files: vec![ModelInstallFile {
                relative: "sherpa/direct-test.onnx".to_owned(),
                expected_sha256: Some(sha256),
                min_bytes: payload.len() as u64,
                source_name: "direct-test.onnx".to_owned(),
                present: false,
                bytes: 0,
            }],
        };
        let directory =
            std::env::temp_dir().join(format!("sonasub-install-test-{}", std::process::id()));
        let source = directory.join("direct-test.onnx");
        fs::create_dir_all(&directory).unwrap();
        fs::write(&source, &payload).unwrap();
        install_direct_file(&directory, &group, &source).unwrap();
        assert_eq!(
            fs::read(directory.join("sherpa/direct-test.onnx")).unwrap(),
            payload
        );
        let _ = fs::remove_dir_all(&directory);
    }

    #[test]
    fn archive_sources_are_found_in_a_nested_model_directory() {
        let directory =
            std::env::temp_dir().join(format!("sonasub-archive-test-{}", std::process::id()));
        let nested = directory.join("sherpa-onnx-model-directory");
        fs::create_dir_all(&nested).unwrap();
        let expected = nested.join("model.int8.onnx");
        fs::write(&expected, b"model").unwrap();
        assert_eq!(
            find_source(&directory, "model.int8.onnx").unwrap(),
            expected
        );
        let _ = fs::remove_dir_all(&directory);
    }

    #[test]
    fn bergamot_import_accepts_runtime_and_attachment_names() {
        let group = ModelInstallGroup {
            id: "bergamot-test".to_owned(),
            label: "Bergamot test".to_owned(),
            version: "test".to_owned(),
            category: "translation".to_owned(),
            upstream: "https://example.invalid".to_owned(),
            download_kind: "bergamot".to_owned(),
            download_url: "https://example.invalid/model.zst".to_owned(),
            download_bytes: 1,
            download_urls: Vec::new(),
            download_bytes_each: Vec::new(),
            archive_sha256: None,
            files: Vec::new(),
        };
        let file = ModelInstallFile {
            relative: "bergamot/en-zh-Hans/model.bin".to_owned(),
            expected_sha256: None,
            min_bytes: 1,
            source_name: "model.enzh.intgemm.alphas.bin.zst".to_owned(),
            present: false,
            bytes: 0,
        };
        assert!(is_import_name(&group, &file, "model.bin"));
        assert!(is_import_name(
            &group,
            &file,
            "model.enzh.intgemm.alphas.bin.zst"
        ));
        assert!(!is_import_name(&group, &file, "other.bin"));
    }

    #[test]
    fn directory_import_expands_nested_runtime_files() {
        let directory =
            std::env::temp_dir().join(format!("sonasub-directory-import-{}", std::process::id()));
        let tokenizer = directory.join("tokenizer");
        fs::create_dir_all(&tokenizer).unwrap();
        fs::write(directory.join("conv_frontend.onnx"), b"model").unwrap();
        fs::write(tokenizer.join("vocab.json"), b"tokenizer").unwrap();

        let files = expand_import_sources(&[directory.display().to_string()]).unwrap();
        let names = files
            .iter()
            .filter_map(|path| path.file_name().and_then(|name| name.to_str()))
            .collect::<Vec<_>>();
        assert_eq!(names.len(), 2);
        assert!(names.contains(&"conv_frontend.onnx"));
        assert!(names.contains(&"vocab.json"));
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn runtime_import_rejects_a_wrong_checksum() {
        let directory =
            std::env::temp_dir().join(format!("sonasub-checksum-test-{}", std::process::id()));
        fs::create_dir_all(&directory).unwrap();
        let source = directory.join("model.onnx");
        fs::write(&source, b"unexpected").unwrap();
        let error = verify_source_file(
            &source,
            "0000000000000000000000000000000000000000000000000000000000000000",
            b"unexpected".len() as u64,
        )
        .expect_err("a wrong checksum must fail");
        assert!(error.contains("checksum mismatch"));
        let _ = fs::remove_dir_all(directory);
    }
}
