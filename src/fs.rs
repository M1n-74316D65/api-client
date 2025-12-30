use crate::types::{FileEntry, HttpMethod, SavedRequest};
use std::path::PathBuf;

/// Scan folder for request files
pub fn scan_folder(folder: &PathBuf) -> Vec<FileEntry> {
    let mut saved_requests = Vec::new();
    if let Ok(entries) = std::fs::read_dir(folder) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                if ext == "json" || ext == "yaml" || ext == "yml" {
                    // Try to parse the method from the file
                    let method = parse_method_from_file(&path);
                    let name = path
                        .file_stem()
                        .and_then(|n| n.to_str())
                        .unwrap_or("Unknown")
                        .to_string();

                    saved_requests.push(FileEntry { name, path, method });
                }
            }
        }
    }
    // sort by name
    saved_requests.sort_by(|a, b| a.name.cmp(&b.name));
    saved_requests
}

/// Parse HTTP method from a saved request file
pub fn parse_method_from_file(path: &PathBuf) -> Option<HttpMethod> {
    if let Ok(content) = std::fs::read_to_string(path) {
        if let Ok(request) = serde_json::from_str::<SavedRequest>(&content) {
            return match request.method.to_uppercase().as_str() {
                "GET" => Some(HttpMethod::Get),
                "POST" => Some(HttpMethod::Post),
                "PUT" => Some(HttpMethod::Put),
                "DELETE" => Some(HttpMethod::Delete),
                "PATCH" => Some(HttpMethod::Patch),
                _ => None,
            };
        }
    }
    None
}
