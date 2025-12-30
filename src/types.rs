use gpui::*;
use gpui_component::input::InputState;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// HTTP Methods supported by the client
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Delete,
    Patch,
}

impl HttpMethod {
    pub fn as_str(&self) -> &'static str {
        match self {
            HttpMethod::Get => "GET",
            HttpMethod::Post => "POST",
            HttpMethod::Put => "PUT",
            HttpMethod::Delete => "DELETE",
            HttpMethod::Patch => "PATCH",
        }
    }

    pub fn color(&self) -> Hsla {
        match self {
            HttpMethod::Get => hsla(0.35, 0.8, 0.45, 1.0), // Green
            HttpMethod::Post => hsla(0.55, 0.8, 0.45, 1.0), // Blue
            HttpMethod::Put => hsla(0.12, 0.8, 0.50, 1.0), // Orange
            HttpMethod::Delete => hsla(0.0, 0.8, 0.50, 1.0), // Red
            HttpMethod::Patch => hsla(0.75, 0.6, 0.55, 1.0), // Purple
        }
    }

    pub fn next(&self) -> HttpMethod {
        match self {
            HttpMethod::Get => HttpMethod::Post,
            HttpMethod::Post => HttpMethod::Put,
            HttpMethod::Put => HttpMethod::Delete,
            HttpMethod::Delete => HttpMethod::Patch,
            HttpMethod::Patch => HttpMethod::Get,
        }
    }
}

/// Request tabs
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RequestTab {
    Params,
    Headers,
    Body,
}

/// Key-Value pair for params and headers
#[derive(Clone)]
pub struct KeyValuePair {
    pub key: Entity<InputState>,
    pub value: Entity<InputState>,
    pub enabled: bool,
}

/// Saved request file format
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SavedRequest {
    pub name: String,
    pub method: String,
    pub url: String,
    #[serde(default)]
    pub headers: std::collections::HashMap<String, String>,
    #[serde(default)]
    pub body: String,
}

/// Sidebar file entry
#[derive(Clone, Debug)]
pub struct FileEntry {
    pub name: String,
    pub path: PathBuf,
    pub method: Option<HttpMethod>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SidebarTab {
    Files,
    Git,
}
