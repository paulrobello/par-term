//! Shared shader installation logic.
//!
//! Used by both the CLI (`install-shaders` command) and the UI (shader install dialog).

use crate::config::Config;
use std::io::Read;
use std::path::Path;

/// Install shaders from GitHub release
/// Returns the number of shaders installed
pub fn install_shaders() -> Result<usize, String> {
    const REPO: &str = "paulrobello/par-term";
    let shaders_dir = Config::shaders_dir();

    // Fetch latest release info
    let api_url = format!("https://api.github.com/repos/{}/releases/latest", REPO);
    let download_url = get_shaders_download_url(&api_url, REPO)?;

    // Download the zip file
    let zip_data = download_file(&download_url)?;

    // Create shaders directory if it doesn't exist
    std::fs::create_dir_all(&shaders_dir)
        .map_err(|e| format!("Failed to create shaders directory: {}", e))?;

    // Extract shaders
    extract_shaders(&zip_data, &shaders_dir)?;

    // Count installed shaders
    let count = count_shader_files(&shaders_dir);

    Ok(count)
}

/// Get the download URL for shaders.zip from the latest release
pub fn get_shaders_download_url(api_url: &str, repo: &str) -> Result<String, String> {
    let mut body = ureq::get(api_url)
        .header("User-Agent", "par-term")
        .call()
        .map_err(|e| format!("Failed to fetch release info: {}", e))?
        .into_body();

    let body_str = body
        .read_to_string()
        .map_err(|e| format!("Failed to read response body: {}", e))?;

    // Parse JSON to find shaders.zip browser_download_url
    // We need the browser_download_url, not the api url
    let search_pattern = "\"browser_download_url\":\"";
    let target_file = "shaders.zip";

    // Find the shaders.zip entry by looking for browser_download_url containing shaders.zip
    for (i, _) in body_str.match_indices(search_pattern) {
        let url_start = i + search_pattern.len();
        if let Some(url_end) = body_str[url_start..].find('"') {
            let url = &body_str[url_start..url_start + url_end];
            if url.ends_with(target_file) {
                return Ok(url.to_string());
            }
        }
    }

    Err(format!(
        "Could not find shaders.zip in the latest release.\n\
         Please check https://github.com/{}/releases",
        repo
    ))
}

/// Download a file from URL and return its contents
pub fn download_file(url: &str) -> Result<Vec<u8>, String> {
    let mut body = ureq::get(url)
        .header("User-Agent", "par-term")
        .call()
        .map_err(|e| format!("Failed to download file: {}", e))?
        .into_body();

    let mut bytes = Vec::new();
    body.as_reader()
        .read_to_end(&mut bytes)
        .map_err(|e| format!("Failed to read download: {}", e))?;

    Ok(bytes)
}

/// Extract shaders from zip data to target directory
pub fn extract_shaders(zip_data: &[u8], target_dir: &Path) -> Result<(), String> {
    use std::io::Cursor;
    use zip::ZipArchive;

    let reader = Cursor::new(zip_data);
    let mut archive = ZipArchive::new(reader).map_err(|e| format!("Failed to open zip: {}", e))?;

    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| format!("Failed to read zip entry: {}", e))?;

        let outpath = match file.enclosed_name() {
            Some(path) => path.to_owned(),
            None => continue,
        };

        if file.is_dir() {
            continue;
        }

        // Handle paths - the zip contains "shaders/" prefix
        let relative_path = outpath.strip_prefix("shaders/").unwrap_or(&outpath);

        if relative_path.as_os_str().is_empty() {
            continue;
        }

        let final_path = target_dir.join(relative_path);

        // Create parent directories if needed
        if let Some(parent) = final_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create directory: {}", e))?;
        }

        // Extract file
        let mut outfile = std::fs::File::create(&final_path)
            .map_err(|e| format!("Failed to create file: {}", e))?;
        std::io::copy(&mut file, &mut outfile)
            .map_err(|e| format!("Failed to write file: {}", e))?;
    }

    Ok(())
}

/// Count .glsl files in directory
pub fn count_shader_files(dir: &Path) -> usize {
    let mut count = 0;
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            if let Some(ext) = entry.path().extension()
                && ext == "glsl"
            {
                count += 1;
            }
        }
    }
    count
}

/// Check if directory contains any .glsl files
pub fn has_shader_files(dir: &Path) -> bool {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            if let Some(ext) = entry.path().extension()
                && ext == "glsl"
            {
                return true;
            }
        }
    }
    false
}
