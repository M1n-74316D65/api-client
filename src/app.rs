use gpui::prelude::*;
use gpui::*;
use gpui_component::badge::Badge;
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::divider::Divider;
use gpui_component::input::{Input, InputState};
use gpui_component::resizable::{h_resizable, resizable_panel, v_resizable};
use gpui_component::scroll::{ScrollableElement, Scrollbar};
use gpui_component::spinner::Spinner;
use gpui_component::tab::{Tab, TabBar};
use gpui_component::tag::Tag;
use gpui_component::theme::{ActiveTheme, Theme, ThemeMode};
use gpui_component::tooltip::Tooltip;
use gpui_component::*;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::components::git_panel::GitPanel;
use crate::git::GitService;

// Define keyboard actions
actions!(
    api_client,
    [
        SendRequest,
        SaveRequest,
        NewRequest,
        OpenFolder,
        ToggleSidebar,
        ToggleTheme,
        CloseWindow
    ]
);

/// Application configuration
#[derive(Debug, Serialize, Deserialize, Default)]
struct AppConfig {
    last_opened_folder: Option<PathBuf>,
}

impl AppConfig {
    fn path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("api-client")
            .join("config.json")
    }

    fn load() -> Self {
        let path = Self::path();
        if let Ok(content) = std::fs::read_to_string(&path) {
            serde_json::from_str(&content).unwrap_or_default()
        } else {
            Self::default()
        }
    }

    fn save(&self) {
        let path = Self::path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(content) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(path, content);
        }
    }
}

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
    fn as_str(&self) -> &'static str {
        match self {
            HttpMethod::Get => "GET",
            HttpMethod::Post => "POST",
            HttpMethod::Put => "PUT",
            HttpMethod::Delete => "DELETE",
            HttpMethod::Patch => "PATCH",
        }
    }

    fn color(&self) -> Hsla {
        match self {
            HttpMethod::Get => hsla(0.35, 0.8, 0.45, 1.0), // Green
            HttpMethod::Post => hsla(0.55, 0.8, 0.45, 1.0), // Blue
            HttpMethod::Put => hsla(0.12, 0.8, 0.50, 1.0), // Orange
            HttpMethod::Delete => hsla(0.0, 0.8, 0.50, 1.0), // Red
            HttpMethod::Patch => hsla(0.75, 0.6, 0.55, 1.0), // Purple
        }
    }

    fn next(&self) -> HttpMethod {
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
    key: Entity<InputState>,
    value: Entity<InputState>,
    enabled: bool,
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

pub struct App {
    url_input: Entity<InputState>,
    name_input: Entity<InputState>,
    body_input: Entity<InputState>,
    params: Vec<KeyValuePair>,
    headers: Vec<KeyValuePair>,
    response_body: String,
    response_is_large: bool,
    scroll_handle: ScrollHandle,
    method: HttpMethod,
    active_tab: RequestTab,
    is_loading: bool,
    response_status: Option<(u16, String)>,
    response_time: Option<u128>,
    // Sidebar state
    sidebar_visible: bool,
    current_folder: Option<PathBuf>,
    saved_requests: Vec<FileEntry>,
    selected_request: Option<usize>,
    // Rename state
    rename_input: Entity<InputState>,
    renaming_index: Option<usize>,
    // Git state
    git_service: Option<std::rc::Rc<GitService>>,
    git_panel: Entity<GitPanel>,
    sidebar_tab: SidebarTab,
    current_branch: Option<String>,
    _subscription: Subscription,
}

impl App {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let url_input = cx.new(|cx| {
            let mut state = InputState::new(window, cx);
            state.set_placeholder("Enter request URL...", window, cx);
            state.set_value("https://httpbin.org/get", window, cx);
            state
        });

        let name_input = cx.new(|cx| {
            let mut state = InputState::new(window, cx);
            state.set_placeholder("Request Name", window, cx);
            state.set_value("New Request", window, cx);
            state
        });

        let rename_input = cx.new(|cx| {
            let mut state = InputState::new(window, cx);
            state.set_placeholder("New Name", window, cx);
            state
        });

        let body_input = cx.new(|cx| {
            let mut state = InputState::new(window, cx);
            state.set_placeholder("Enter JSON body...", window, cx);
            state
        });

        // Create initial empty param rows
        let params = vec![Self::create_kv_pair(window, cx, "", "")];

        // Create initial header rows
        let headers = vec![
            Self::create_kv_pair(window, cx, "Content-Type", "application/json"),
            Self::create_kv_pair(window, cx, "", ""),
        ];

        // Load config
        let config = AppConfig::load();
        let current_folder = config.last_opened_folder;
        let saved_requests = if let Some(folder) = &current_folder {
            Self::scan_folder(folder)
        } else {
            Vec::new()
        };

        let mut app = Self {
            url_input,
            name_input,
            body_input,
            params,
            headers,
            response_body: String::new(),
            response_is_large: false,
            scroll_handle: ScrollHandle::new(),
            method: HttpMethod::Get,
            active_tab: RequestTab::Params,
            is_loading: false,
            response_status: None,
            response_time: None,
            // Sidebar state
            sidebar_visible: true,
            current_folder,
            saved_requests,
            selected_request: None,
            rename_input,
            renaming_index: None,
            git_service: None,
            git_panel: cx.new(|cx| GitPanel::new(window, cx)),
            sidebar_tab: SidebarTab::Files,
            current_branch: None,
            _subscription: cx.on_release(|_, cx| {
                cx.quit();
            }),
        };

        app.init_git(cx);
        app
    }

    fn init_git(&mut self, cx: &mut Context<Self>) {
        if let Some(folder) = &self.current_folder {
            if let Ok(service) = GitService::new(folder) {
                self.git_service = Some(std::rc::Rc::new(service));
                self.refresh_git_status(cx);
            } else {
                self.git_service = None;
            }
        }
    }

    fn refresh_git_status(&mut self, cx: &mut Context<Self>) {
        if let Some(service) = &self.git_service {
            if let Ok(branch) = service.get_current_branch() {
                self.current_branch = Some(branch);
            }
            if let Ok(changes) = service.get_status() {
                self.git_panel.update(cx, |panel, cx| {
                    panel.set_changes(changes);
                    cx.notify();
                });
            }
        }
    }

    fn create_kv_pair(
        window: &mut Window,
        cx: &mut Context<Self>,
        key: &str,
        value: &str,
    ) -> KeyValuePair {
        let key_owned = key.to_string();
        let value_owned = value.to_string();

        let key_input = cx.new(|cx| {
            let mut state = InputState::new(window, cx);
            state.set_placeholder("Key", window, cx);
            if !key_owned.is_empty() {
                state.set_value(&key_owned, window, cx);
            }
            state
        });
        let value_input = cx.new(|cx| {
            let mut state = InputState::new(window, cx);
            state.set_placeholder("Value", window, cx);
            if !value_owned.is_empty() {
                state.set_value(&value_owned, window, cx);
            }
            state
        });
        KeyValuePair {
            key: key_input,
            value: value_input,
            enabled: true,
        }
    }

    fn add_param(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let pair = Self::create_kv_pair(window, cx, "", "");
        self.params.push(pair);
        cx.notify();
    }

    fn add_header(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let pair = Self::create_kv_pair(window, cx, "", "");
        self.headers.push(pair);
        cx.notify();
    }

    fn build_url_with_params(&self, cx: &Context<Self>) -> String {
        let base_url = self.url_input.read(cx).value().to_string();

        let params: Vec<(String, String)> = self
            .params
            .iter()
            .filter(|p| p.enabled)
            .map(|p| {
                (
                    p.key.read(cx).value().to_string(),
                    p.value.read(cx).value().to_string(),
                )
            })
            .filter(|(k, _)| !k.is_empty())
            .collect();

        if params.is_empty() {
            return base_url;
        }

        let query = params
            .iter()
            .map(|(k, v)| format!("{}={}", urlencoding(k), urlencoding(v)))
            .collect::<Vec<_>>()
            .join("&");

        if base_url.contains('?') {
            format!("{}&{}", base_url, query)
        } else {
            format!("{}?{}", base_url, query)
        }
    }

    fn get_headers(&self, cx: &Context<Self>) -> Vec<(String, String)> {
        self.headers
            .iter()
            .filter(|h| h.enabled)
            .map(|h| {
                (
                    h.key.read(cx).value().to_string(),
                    h.value.read(cx).value().to_string(),
                )
            })
            .filter(|(k, _)| !k.is_empty())
            .collect()
    }

    fn send_request(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // Auto-save request
        self.save_request(window, cx);

        let url = self.build_url_with_params(cx);
        let body = self.body_input.read(cx).value().to_string();
        let headers = self.get_headers(cx);
        let method = self.method.clone();

        if url.is_empty() {
            return;
        }

        self.is_loading = true;
        self.response_status = None;
        self.response_body.clear();
        self.response_is_large = false;
        self.response_time = None;
        cx.notify();

        cx.spawn_in(window, async move |this, cx| {
            let start = std::time::Instant::now();
            let result = Self::execute_request(&url, &method, &body, &headers).await;
            let elapsed = start.elapsed().as_millis();

            cx.update(|_window, cx| {
                this.update(cx, |app, cx| {
                    app.is_loading = false;
                    app.response_time = Some(elapsed);
                    match result {
                        Ok((status, body)) => {
                            let status_text = if status >= 200 && status < 300 {
                                "OK"
                            } else if status >= 400 && status < 500 {
                                "Client Error"
                            } else if status >= 500 {
                                "Server Error"
                            } else {
                                "Response"
                            };
                            app.response_status = Some((status, status_text.to_string()));

                            app.response_is_large = body.len() > MAX_RESPONSE_DISPLAY_BYTES;

                            // Try to format JSON response when it's safe to display.
                            app.response_body = if app.response_is_large {
                                body
                            } else if let Ok(json) =
                                serde_json::from_str::<serde_json::Value>(&body)
                            {
                                serde_json::to_string_pretty(&json).unwrap_or(body)
                            } else {
                                body
                            };
                        }
                        Err(e) => {
                            app.response_status = Some((0, "Error".to_string()));
                            app.response_body = format!("Error: {}", e);
                            app.response_is_large = false;
                        }
                    }
                    cx.notify();
                })
            })
        })
        .detach();
    }

    async fn execute_request(
        url: &str,
        method: &HttpMethod,
        body: &str,
        headers: &[(String, String)],
    ) -> Result<(u16, String), String> {
        let client = reqwest::Client::new();

        let mut builder = match method {
            HttpMethod::Get => client.get(url),
            HttpMethod::Post => client.post(url),
            HttpMethod::Put => client.put(url),
            HttpMethod::Delete => client.delete(url),
            HttpMethod::Patch => client.patch(url),
        };

        // Add headers
        for (key, value) in headers {
            builder = builder.header(key.as_str(), value.as_str());
        }

        // Add body for methods that support it
        if !body.is_empty()
            && matches!(
                method,
                HttpMethod::Post | HttpMethod::Put | HttpMethod::Patch
            )
        {
            builder = builder.body(body.to_string());
        }

        let response = builder.send().await.map_err(|e| e.to_string())?;
        let status = response.status().as_u16();
        let text = response.text().await.map_err(|e| e.to_string())?;

        Ok((status, text))
    }

    /// Open folder dialog and load requests
    fn open_folder(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // Spawn async task to show folder picker
        cx.spawn_in(window, async move |this, cx| {
            // Show native folder picker dialog
            let folder = rfd::AsyncFileDialog::new()
                .set_title("Select Requests Folder")
                .pick_folder()
                .await;

            if let Some(path) = folder.map(|f| f.path().to_path_buf()) {
                let _ = this.update(cx, |app, cx| {
                    app.current_folder = Some(path.clone());

                    // Save config
                    let config = AppConfig {
                        last_opened_folder: Some(path),
                    };
                    config.save();

                    app.load_folder(cx);
                    cx.notify();
                });
            }
        })
        .detach();
    }

    /// Scan folder for request files
    fn scan_folder(folder: &PathBuf) -> Vec<FileEntry> {
        let mut saved_requests = Vec::new();
        if let Ok(entries) = std::fs::read_dir(folder) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                    if ext == "json" || ext == "yaml" || ext == "yml" {
                        // Try to parse the method from the file
                        let method = Self::parse_method_from_file(&path);
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

    /// Load requests from current folder
    fn load_folder(&mut self, _cx: &mut Context<Self>) {
        if let Some(folder) = &self.current_folder {
            self.saved_requests = Self::scan_folder(folder);
        } else {
            self.saved_requests.clear();
        }
    }

    /// Parse HTTP method from a saved request file
    fn parse_method_from_file(path: &PathBuf) -> Option<HttpMethod> {
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

    /// Save current request to file
    fn save_request(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        if let Some(folder) = &self.current_folder {
            let url = self.url_input.read(cx).value().to_string();
            let body = self.body_input.read(cx).value().to_string();
            let method = self.method.as_str().to_string();
            let name = self.name_input.read(cx).value().to_string();

            let mut headers = std::collections::HashMap::new();
            for kv in &self.headers {
                let key = kv.key.read(cx).value().to_string();
                let value = kv.value.read(cx).value().to_string();
                if !key.is_empty() {
                    headers.insert(key, value);
                }
            }

            // If name is empty, provide a default
            let name = if name.is_empty() {
                format!("New Request {}", self.saved_requests.len() + 1)
            } else {
                name
            };

            let request = SavedRequest {
                name: name.clone(),
                method,
                url,
                headers,
                body,
            };

            if let Ok(json) = serde_json::to_string_pretty(&request) {
                let path = if let Some(idx) = self.selected_request {
                    // Overwrite existing file
                    self.saved_requests[idx].path.clone()
                } else {
                    // Create new file
                    let safe_name: String = name
                        .chars()
                        .map(|c| if c.is_alphanumeric() { c } else { '-' })
                        .collect();
                    folder.join(format!("{}.json", safe_name))
                };

                if std::fs::write(&path, json).is_ok() {
                    self.load_folder(cx);

                    // If we just saved to a specific path, find it and select it
                    if let Some(idx) = self.saved_requests.iter().position(|r| r.path == path) {
                        self.selected_request = Some(idx);
                    }
                }
            }
        }
    }

    /// Save as new request
    fn save_new_request(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.selected_request = None;
        self.save_request(window, cx);
    }

    /// Load a saved request into the editor
    fn load_request(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(entry) = self.saved_requests.get(index) {
            if let Ok(content) = std::fs::read_to_string(&entry.path) {
                if let Ok(request) = serde_json::from_str::<SavedRequest>(&content) {
                    // Set name
                    self.name_input.update(cx, |state, cx| {
                        state.set_value(&request.name, window, cx);
                    });

                    // Set method
                    self.method = match request.method.to_uppercase().as_str() {
                        "GET" => HttpMethod::Get,
                        "POST" => HttpMethod::Post,
                        "PUT" => HttpMethod::Put,
                        "DELETE" => HttpMethod::Delete,
                        "PATCH" => HttpMethod::Patch,
                        _ => HttpMethod::Get,
                    };

                    // Set URL
                    self.url_input.update(cx, |state, cx| {
                        state.set_value(&request.url, window, cx);
                    });

                    // Set body
                    if !request.body.is_empty() {
                        self.body_input.update(cx, |state, cx| {
                            state.set_value(&request.body, window, cx);
                        });
                    }

                    // Clear and set headers
                    self.headers.clear();
                    for (key, value) in request.headers.iter() {
                        self.headers
                            .push(Self::create_kv_pair(window, cx, key, value));
                    }
                    // Add empty row for new headers
                    self.headers.push(Self::create_kv_pair(window, cx, "", ""));

                    self.selected_request = Some(index);
                    cx.notify();
                }
            }
        }
    }

    /// Delete a request
    fn delete_request(&mut self, index: usize, _window: &mut Window, cx: &mut Context<Self>) {
        if let Some(folder) = &self.current_folder {
            if let Some(request) = self.saved_requests.get(index) {
                let name = if request.name.ends_with(".json") {
                    request.name.clone()
                } else {
                    format!("{}.json", request.name)
                };
                let path = folder.join(&name);

                // Attempt to delete file
                if let Err(e) = std::fs::remove_file(&path) {
                    eprintln!("Failed to delete file {:?}: {}", path, e);
                    return;
                }

                // Remove from list
                self.saved_requests.remove(index);

                // Update selected index
                if let Some(selected) = self.selected_request {
                    if selected == index {
                        self.selected_request = None;
                    } else if selected > index {
                        self.selected_request = Some(selected - 1);
                    }
                }

                cx.notify();
            }
        }
    }

    /// Start renaming a request
    fn start_renaming(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(request) = self.saved_requests.get(index) {
            self.renaming_index = Some(index);
            // remove .json extension for editing
            let name_str = if request.name.ends_with(".json") {
                &request.name[..request.name.len() - 5]
            } else {
                &request.name
            };
            let name = name_str.to_string();

            let input_entity = self.rename_input.clone();
            input_entity.update(cx, |state, cx| {
                state.set_value(&name, window, cx);
                // state.focus_handle(cx).focus(window); // Keeping focus commented for safety first, can enable later
            });
            cx.notify();
        }
    }

    /// Cancel renaming
    fn cancel_renaming(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.renaming_index = None;
        cx.notify();
    }

    /// Confirm renaming
    fn confirm_renaming(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        if let Some(index) = self.renaming_index {
            if let Some(folder) = &self.current_folder {
                if let Some(request) = self.saved_requests.get(index) {
                    let new_name = self.rename_input.read(cx).value().to_string();
                    let safe_name = urlencoding(&new_name)
                        .replace("%", "")
                        .replace("/", "")
                        .replace("\\", "");

                    if safe_name.is_empty() {
                        return;
                    }

                    let old_filename = if request.name.ends_with(".json") {
                        request.name.clone()
                    } else {
                        format!("{}.json", request.name)
                    };

                    let new_filename = format!("{}.json", safe_name);
                    let old_path = folder.join(&old_filename);
                    let new_path = folder.join(&new_filename);

                    if let Err(e) = std::fs::rename(&old_path, &new_path) {
                        eprintln!("Failed to rename file: {}", e);
                    } else {
                        // Update the entry in the list
                        if let Some(entry) = self.saved_requests.get_mut(index) {
                            entry.name = new_filename;
                        }
                    }
                }
            }
        }
        self.renaming_index = None;
        cx.notify();
    }

    /// Render the sidebar
    fn render_sidebar(&self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let folder_name: String = self
            .current_folder
            .as_ref()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("No folder")
            .to_string();

        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(cx.theme().sidebar)
            .border_r_1()
            .border_color(cx.theme().sidebar_border)
            // Header
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .p_3()
                    .border_b_1()
                    .border_color(cx.theme().sidebar_border)
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(cx.theme().sidebar_foreground)
                            .child(if self.sidebar_tab == SidebarTab::Files {
                                "Requests"
                            } else {
                                "Git Changes"
                            }),
                    )
                    .child(if self.sidebar_tab == SidebarTab::Files {
                        div()
                            .id("open-folder-btn")
                            .p_1()
                            .rounded(px(4.0))
                            .cursor_pointer()
                            .hover(|s| s.bg(cx.theme().sidebar_accent))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, window, cx| {
                                    this.open_folder(window, cx);
                                    cx.notify();
                                }),
                            )
                            .tooltip(|window, cx| Tooltip::new("Open Folder").build(window, cx))
                            .child(
                                Icon::new(IconName::FolderOpen)
                                    .text_color(cx.theme().sidebar_foreground),
                            )
                            .into_any_element()
                    } else {
                        div().into_any_element()
                    }),
            )
            // Folder path
            .child(
                div()
                    .px_3()
                    .py_2()
                    .text_xs()
                    .text_color(cx.theme().sidebar_foreground.opacity(0.7))
                    .child(folder_name.clone()),
            )
            // File list
            // File list or Empty State
            .child(if self.sidebar_tab == SidebarTab::Files {
                if self.saved_requests.is_empty() {
                    let (message, sub_message, icon) = if self.current_folder.is_some() {
                        (
                            "No requests",
                            "Create a new request to get started",
                            IconName::File,
                        )
                    } else {
                        (
                            "No folder open",
                            "Open a folder to see your requests",
                            IconName::FolderOpen,
                        )
                    };

                    div()
                        .flex_1()
                        .flex()
                        .flex_col()
                        .items_center()
                        .justify_center()
                        .gap_3()
                        .text_color(cx.theme().muted_foreground)
                        .child(
                            Icon::new(icon)
                                .size(px(32.0))
                                .text_color(cx.theme().muted_foreground.opacity(0.5)),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .items_center()
                                .gap_1()
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(FontWeight::MEDIUM)
                                        .child(message),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground.opacity(0.7))
                                        .child(sub_message),
                                ),
                        )
                        .into_any_element()
                } else {
                    div()
                        .flex_1()
                        .overflow_y_scrollbar()
                        .children(self.saved_requests.iter().enumerate().map(|(i, entry)| {
                            let is_selected = self.selected_request == Some(i);
                            let method_color = entry
                                .method
                                .as_ref()
                                .map(|m| m.color())
                                .unwrap_or(cx.theme().muted_foreground);
                            let method_str =
                                entry.method.as_ref().map(|m| m.as_str()).unwrap_or("???");
                            let name = entry.name.clone();
                            let is_renaming = self.renaming_index == Some(i);

                            div()
                                .id(ElementId::Name(format!("request-{}", i).into()))
                                .group("request-item")
                                .flex()
                                .items_center()
                                .gap_2()
                                .px_3()
                                .py(px(6.0)) // Tighter, refined spacing
                                .cursor_pointer()
                                .bg(if is_selected {
                                    cx.theme().accent.opacity(0.15)
                                } else {
                                    gpui::transparent_black()
                                })
                                .hover(|s| s.bg(cx.theme().muted.opacity(0.5)))
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(move |this, _, window, cx| {
                                        this.load_request(i, window, cx);
                                    }),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .justify_between()
                                        .w_full()
                                        .child(if is_renaming {
                                            div()
                                                .flex()
                                                .items_center()
                                                .gap_2()
                                                .flex_1()
                                                .child(
                                                    div().flex_1().child(
                                                        Input::new(&self.rename_input)
                                                            .appearance(false),
                                                    ),
                                                )
                                                .child(
                                                    div()
                                                        .cursor_pointer()
                                                        .child(
                                                            Icon::new(IconName::Check)
                                                                .size(px(14.0))
                                                                .text_color(cx.theme().primary),
                                                        )
                                                        .on_mouse_down(
                                                            MouseButton::Left,
                                                            cx.listener(
                                                                move |this, _, window, cx| {
                                                                    cx.stop_propagation();
                                                                    this.confirm_renaming(
                                                                        window, cx,
                                                                    );
                                                                },
                                                            ),
                                                        ),
                                                )
                                                .child(
                                                    div()
                                                        .cursor_pointer()
                                                        .child(
                                                            Icon::new(IconName::Close)
                                                                .size(px(14.0))
                                                                .text_color(hsla(
                                                                    0.0, 0.6, 0.4, 1.0,
                                                                )),
                                                        )
                                                        .on_mouse_down(
                                                            MouseButton::Left,
                                                            cx.listener(
                                                                move |this, _, window, cx| {
                                                                    cx.stop_propagation();
                                                                    this.cancel_renaming(
                                                                        window, cx,
                                                                    );
                                                                },
                                                            ),
                                                        ),
                                                )
                                        } else {
                                            div()
                                                .flex()
                                                .items_center()
                                                .gap_3()
                                                .flex_1()
                                                .child(
                                                    Tag::new()
                                                        .small()
                                                        .bg(method_color.opacity(0.15))
                                                        .text_color(method_color)
                                                        .child(method_str),
                                                )
                                                .child(
                                                    div()
                                                        .text_sm()
                                                        .overflow_hidden()
                                                        .whitespace_nowrap()
                                                        .text_ellipsis()
                                                        .child(name),
                                                )
                                        })
                                        .child(
                                            div()
                                                .flex()
                                                .items_center()
                                                .gap_1()
                                                .invisible()
                                                .group_hover("request-item", |s| s.visible())
                                                .when(!is_renaming, |this| {
                                                    this.child(
                                                        div()
                                                            .p_1()
                                                            .rounded_sm()
                                                            .hover(|s| s.bg(cx.theme().muted))
                                                            .child(
                                                                Icon::new(IconName::Settings)
                                                                    .size(px(14.0))
                                                                    .text_color(
                                                                        cx.theme().muted_foreground,
                                                                    ),
                                                            )
                                                            .on_mouse_down(
                                                                MouseButton::Left,
                                                                cx.listener(
                                                                    move |this, _, window, cx| {
                                                                        cx.stop_propagation();
                                                                        this.start_renaming(
                                                                            i, window, cx,
                                                                        );
                                                                    },
                                                                ),
                                                            ),
                                                    )
                                                    .child(
                                                        div()
                                                            .p_1()
                                                            .rounded_sm()
                                                            .hover(|s| {
                                                                s.bg(hsla(0.0, 0.6, 0.4, 0.2))
                                                            })
                                                            .child(
                                                                Icon::new(IconName::Delete)
                                                                    .size(px(14.0))
                                                                    .text_color(
                                                                        cx.theme().muted_foreground,
                                                                    ),
                                                            )
                                                            .on_mouse_down(
                                                                MouseButton::Left,
                                                                cx.listener(
                                                                    move |this, _, window, cx| {
                                                                        cx.stop_propagation();
                                                                        this.delete_request(
                                                                            i, window, cx,
                                                                        );
                                                                    },
                                                                ),
                                                            ),
                                                    )
                                                }),
                                        ),
                                )
                        }))
                        .into_any_element()
                }
            } else {
                div()
                    .id("git-panel")
                    .size_full()
                    .child(self.git_panel.clone())
                    .into_any_element()
            })
    }

    fn render_title_bar(&self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let folder_name = self
            .current_folder
            .as_ref()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("");

        TitleBar::new().child(
            div()
                .w_full()
                .flex()
                .items_center()
                .justify_between()
                .px_4()
                .px_4()
                .when(cfg!(target_os = "macos"), |s| s.pl(px(80.0))) // Traffic lights padding
                .on_mouse_down(MouseButton::Left, |_, window, _| window.start_window_move())
                // Left Section: Toggle + Logo
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_3()
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation()) // Prevent drag on controls
                        .child(
                            div()
                                .id("sidebar-toggle")
                                .cursor_pointer()
                                .p_1()
                                .rounded(px(4.0))
                                .hover(|s| s.bg(cx.theme().accent.opacity(0.2)))
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(|this, _, _, cx| {
                                        this.sidebar_visible = !this.sidebar_visible;
                                        cx.notify();
                                    }),
                                )
                                .tooltip(|window, cx| {
                                    Tooltip::new("Toggle Sidebar").build(window, cx)
                                })
                                .child(
                                    Icon::new(if self.sidebar_visible {
                                        IconName::PanelLeftClose
                                    } else {
                                        IconName::PanelLeftOpen
                                    })
                                    .text_color(cx.theme().muted_foreground),
                                ),
                        )
                        .child(Icon::new(IconName::Globe).text_color(cx.theme().primary))
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::BOLD)
                                .child("API Client"),
                        ),
                )
                // Center Section: Workspace Info
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(if folder_name.is_empty() {
                            "No folder opened".to_string()
                        } else {
                            format!("Workspace: {}", folder_name)
                        }),
                )
                // Right Section: Theme Toggle + Version
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        // Window Controls for non-macOS
                        .when(cfg!(not(target_os = "macos")), |this| {
                            this.child(
                                div()
                                    .flex()
                                    .items_center()
                                    .child(
                                        div() // Minimize
                                            .id("minimize-window")
                                            .cursor_pointer()
                                            .p_2()
                                            .hover(|s| s.bg(cx.theme().accent.opacity(0.1)))
                                            .on_mouse_down(MouseButton::Left, |_, window, _| {
                                                window.minimize_window();
                                            })
                                            .child(
                                                Icon::new(IconName::ArrowDown)
                                                    .size(px(14.0))
                                                    .text_color(cx.theme().foreground),
                                            ),
                                    )
                                    .child(
                                        div() // Maximize / Restore
                                            .id("maximize-window")
                                            .cursor_pointer()
                                            .p_2()
                                            .hover(|s| s.bg(cx.theme().accent.opacity(0.1)))
                                            .on_mouse_down(MouseButton::Left, |_, window, _| {
                                                window.zoom_window();
                                            })
                                            .child(
                                                Icon::new(IconName::Plus)
                                                    .size(px(14.0))
                                                    .text_color(cx.theme().foreground),
                                            ),
                                    )
                                    .child(
                                        div() // Close
                                            .id("close-window")
                                            .cursor_pointer()
                                            .p_2()
                                            .hover(|s| s.bg(hsla(0.0, 0.9, 0.5, 0.8)).text_color(gpui::white()))
                                            .on_mouse_down(MouseButton::Left, |_, window, _| {
                                                window.remove_window();
                                            })
                                            .child(
                                                Icon::new(IconName::Close)
                                                    .size(px(14.0))
                                                    .text_color(cx.theme().foreground),
                                            ),
                                    )
                            )
                        })
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .child(
                            div()
                                .id("theme-toggle")
                                .cursor_pointer()
                                .p_1()
                                .rounded(px(4.0))
                                .hover(|s| s.bg(cx.theme().accent.opacity(0.2)))
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(|_this, _, window, cx| {
                                        let is_dark = Theme::global(cx).is_dark();
                                        let new_mode = if is_dark {
                                            ThemeMode::Light
                                        } else {
                                            ThemeMode::Dark
                                        };
                                        Theme::change(new_mode, Some(window), cx);
                                        cx.notify();
                                    }),
                                )
                                .tooltip(|window, cx| {
                                    let mode_text = if Theme::global(cx).is_dark() {
                                        "Switch to Light Mode"
                                    } else {
                                        "Switch to Dark Mode"
                                    };
                                    Tooltip::new(mode_text).build(window, cx)
                                })
                                .child(
                                    Icon::new(if cx.theme().mode.is_dark() {
                                        IconName::Sun
                                    } else {
                                        IconName::Moon
                                    })
                                    .text_color(cx.theme().muted_foreground),
                                ),
                        )
                        .child(Badge::new().child("v0.1.0")),
                ),
        )
    }

    fn render_request_bar(&self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (method_bg, method_color, method_text) = match self.method {
            HttpMethod::Get => (
                hsla(0.35, 0.6, 0.15, 1.0),
                hsla(0.35, 0.8, 0.65, 1.0),
                "GET",
            ),
            HttpMethod::Post => (hsla(0.6, 0.6, 0.15, 1.0), hsla(0.6, 0.8, 0.65, 1.0), "POST"),
            HttpMethod::Put => (hsla(0.1, 0.6, 0.15, 1.0), hsla(0.1, 0.8, 0.65, 1.0), "PUT"),
            HttpMethod::Delete => (
                hsla(0.0, 0.6, 0.15, 1.0),
                hsla(0.0, 0.8, 0.65, 1.0),
                "ERR", // DELETE is too long for icon style sometimes, but DELETE is standard
            ),
            HttpMethod::Patch => (hsla(0.5, 0.6, 0.15, 1.0), hsla(0.5, 0.8, 0.65, 1.0), "PTCH"),
        };
        let method_text = if self.method == HttpMethod::Delete {
            "DEL"
        } else {
            method_text
        };

        div()
            .flex()
            .flex_col()
            .gap_3()
            .p_4()
            .bg(cx.theme().secondary)
            // Row 1: Name Input
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(cx.theme().muted_foreground)
                            .child("Name"),
                    )
                    .child(
                        div()
                            .flex_1()
                            .px_3()
                            .py_1()
                            .rounded(px(8.0))
                            .bg(cx.theme().input)
                            .border_1()
                            .border_color(cx.theme().border)
                            .child(Input::new(&self.name_input).appearance(false)),
                    ),
            )
            // Row 2: Request Details
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(
                        // Method selector with dropdown menu
                        Button::new("method-selector")
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        div()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(method_color)
                                            .child(method_text),
                                    )
                                    .child(
                                        Icon::new(IconName::ChevronDown)
                                            .size(px(14.0))
                                            .text_color(method_color.opacity(0.7)),
                                    ),
                            )
                            .bg(method_bg)
                            .border_1()
                            .border_color(method_color.opacity(0.3))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.method = this.method.next();
                                cx.notify();
                            })),
                    )
                    .child(
                        div()
                            .flex_1()
                            .px_3()
                            .py_1()
                            .rounded(px(8.0))
                            .bg(cx.theme().input)
                            .border_1()
                            .border_color(cx.theme().border)
                            .child(
                                Input::new(&self.url_input).appearance(false).prefix(
                                    Icon::new(IconName::Globe)
                                        .small()
                                        .text_color(cx.theme().muted_foreground),
                                ),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                Button::new("save-req")
                                    .icon(IconName::File)
                                    .label("Save")
                                    .outline()
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.save_request(window, cx);
                                    })),
                            )
                            .child(
                                Button::new("save-new-req")
                                    .icon(IconName::Plus)
                                    .label("New")
                                    .ghost()
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.save_new_request(window, cx);
                                    })),
                            )
                            .child(
                                Button::new("send")
                                    .primary()
                                    .icon(IconName::ArrowRight)
                                    .label("Send")
                                    .loading(self.is_loading)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.send_request(window, cx);
                                    })),
                            ),
                    ),
            )
    }

    fn render_tabs(&self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let active_tab = self.active_tab.clone();
        let param_count = self
            .params
            .iter()
            .filter(|p| {
                let key = p.key.read(cx).value().to_string();
                !key.is_empty()
            })
            .count();
        let header_count = self
            .headers
            .iter()
            .filter(|h| {
                let key = h.key.read(cx).value().to_string();
                !key.is_empty()
            })
            .count();

        div()
            .flex()
            .items_center()
            .px_4()
            .py_2()
            .bg(cx.theme().muted)
            .border_b_1()
            .border_color(cx.theme().border)
            .child(
                TabBar::new("request-tabs")
                    .pill()
                    .selected_index(match active_tab {
                        RequestTab::Params => 0,
                        RequestTab::Headers => 1,
                        RequestTab::Body => 2,
                    })
                    .on_click(cx.listener(|this, index, _, cx| {
                        this.active_tab = match index {
                            0 => RequestTab::Params,
                            1 => RequestTab::Headers,
                            _ => RequestTab::Body,
                        };
                        cx.notify();
                    }))
                    .child(
                        Tab::new().child(
                            h_flex()
                                .items_center()
                                .gap_2()
                                .child(Icon::new(IconName::Search).size(px(14.0)))
                                .child("Params")
                                .when(param_count > 0, |this| {
                                    this.child(
                                        div()
                                            .px_1()
                                            .py_0p5()
                                            .text_xs()
                                            .bg(cx.theme().accent)
                                            .text_color(cx.theme().accent_foreground)
                                            .rounded_sm()
                                            .child(format!("{}", param_count)),
                                    )
                                }),
                        ),
                    )
                    .child(
                        Tab::new().child(
                            h_flex()
                                .items_center()
                                .gap_2()
                                .child(Icon::new(IconName::Settings).size(px(14.0)))
                                .child("Headers")
                                .when(header_count > 0, |this| {
                                    this.child(
                                        div()
                                            .px_1()
                                            .py_0p5()
                                            .text_xs()
                                            .bg(cx.theme().accent)
                                            .text_color(cx.theme().accent_foreground)
                                            .rounded_sm()
                                            .child(format!("{}", header_count)),
                                    )
                                }),
                        ),
                    )
                    .child(
                        Tab::new().child(
                            h_flex()
                                .items_center()
                                .gap_2()
                                .child(Icon::new(IconName::File).size(px(14.0)))
                                .child("Body"),
                        ),
                    ),
            )
    }

    fn render_kv_row(
        &self,
        index: usize,
        pair: &KeyValuePair,
        is_param: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let id = if is_param {
            format!("param-{}", index)
        } else {
            format!("header-{}", index)
        };

        div()
            .id(ElementId::Name(id.into()))
            .flex()
            .items_center()
            .gap_3()
            .mb_2()
            .p_2()
            .rounded(px(6.0))
            .bg(cx.theme().muted)
            .border_1()
            .border_color(cx.theme().border)
            .child(
                div()
                    .flex_1()
                    .child(Input::new(&pair.key).appearance(false)),
            )
            .child(div().text_color(cx.theme().muted_foreground).child("="))
            .child(
                div()
                    .flex_1()
                    .child(Input::new(&pair.value).appearance(false)),
            )
            .child(
                Button::new(ElementId::Name(
                    format!(
                        "delete-{}-{}",
                        if is_param { "param" } else { "header" },
                        index
                    )
                    .into(),
                ))
                .icon(IconName::Delete)
                .ghost()
                .on_click(cx.listener(move |this, _, _, cx| {
                    if is_param {
                        if this.params.len() > 1 {
                            this.params.remove(index);
                        }
                    } else {
                        if this.headers.len() > 1 {
                            this.headers.remove(index);
                        }
                    }
                    cx.notify();
                })),
            )
    }

    fn render_request_panel(
        &self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let content = match self.active_tab {
            RequestTab::Params => {
                let rows: Vec<_> = self
                    .params
                    .iter()
                    .enumerate()
                    .map(|(i, pair)| self.render_kv_row(i, pair, true, cx))
                    .collect();

                div()
                    .size_full()
                    .flex()
                    .flex_col()
                    .pb_4()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .mb_4()
                            .child(
                                Icon::new(IconName::Search).text_color(cx.theme().muted_foreground),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child("Query parameters will be appended to the URL"),
                            ),
                    )
                    // Column headers
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .mb_2()
                            .px_2()
                            .child(
                                div()
                                    .flex_1()
                                    .text_xs()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(cx.theme().muted_foreground)
                                    .child("Key"),
                            )
                            .child(
                                div()
                                    .w(px(14.0))
                                    .text_color(cx.theme().muted_foreground)
                                    .child(""),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .text_xs()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(cx.theme().muted_foreground)
                                    .child("Value"),
                            )
                            .child(div().w(px(28.0))),
                    )
                    .children(rows)
                    .child(
                        div().mb_4().child(
                            Button::new("add-param")
                                .icon(IconName::Plus)
                                .label("Add Parameter")
                                .outline()
                                .w_full()
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.add_param(window, cx);
                                })),
                        ),
                    )
                    .into_any_element()
            }
            RequestTab::Headers => {
                let rows: Vec<_> = self
                    .headers
                    .iter()
                    .enumerate()
                    .map(|(i, pair)| self.render_kv_row(i, pair, false, cx))
                    .collect();

                div()
                    .size_full()
                    .flex()
                    .flex_col()
                    .pb_4()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .mb_4()
                            .child(
                                Icon::new(IconName::Settings)
                                    .text_color(cx.theme().muted_foreground),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child("HTTP headers to include in the request"),
                            ),
                    )
                    // Column headers
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .mb_2()
                            .px_2()
                            .child(
                                div()
                                    .flex_1()
                                    .text_xs()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(cx.theme().muted_foreground)
                                    .child("Header Name"),
                            )
                            .child(
                                div()
                                    .w(px(14.0))
                                    .text_color(cx.theme().muted_foreground)
                                    .child(""),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .text_xs()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(cx.theme().muted_foreground)
                                    .child("Value"),
                            )
                            .child(div().w(px(28.0))),
                    )
                    .children(rows)
                    .child(
                        div().mb_4().child(
                            Button::new("add-header")
                                .icon(IconName::Plus)
                                .label("Add Header")
                                .outline()
                                .w_full()
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.add_header(window, cx);
                                })),
                        ),
                    )
                    .into_any_element()
            }
            RequestTab::Body => div()
                .size_full()
                .flex()
                .flex_col()
                .pb_4()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .mb_4()
                        .child(Icon::new(IconName::File).text_color(cx.theme().muted_foreground))
                        .child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child("Request body for POST, PUT, PATCH requests"),
                        ),
                )
                .child(
                    div()
                        .flex_1()
                        .p_3()
                        .mb_4()
                        .rounded(px(8.0))
                        .bg(cx.theme().muted)
                        .border_1()
                        .border_color(cx.theme().border)
                        .child(Input::new(&self.body_input).appearance(false)),
                )
                .into_any_element(),
        };

        div().flex_1().p_4().bg(cx.theme().muted).child(content)
    }

    fn render_response_panel(
        &self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let has_response = !self.response_body.is_empty();
        let response_too_large = self.response_is_large;
        let status_badge = if let Some((code, text)) = &self.response_status {
            let (bg_color, text_color, icon) = if *code >= 200 && *code < 300 {
                (
                    hsla(0.35, 0.6, 0.25, 1.0),
                    hsla(0.35, 0.8, 0.65, 1.0),
                    IconName::Check,
                )
            } else if *code >= 400 {
                (
                    hsla(0.0, 0.6, 0.25, 1.0),
                    hsla(0.0, 0.8, 0.65, 1.0),
                    IconName::Close,
                )
            } else if *code == 0 {
                (
                    hsla(0.0, 0.6, 0.25, 1.0),
                    hsla(0.0, 0.8, 0.65, 1.0),
                    IconName::TriangleAlert,
                )
            } else {
                (
                    hsla(0.12, 0.6, 0.25, 1.0),
                    hsla(0.12, 0.8, 0.65, 1.0),
                    IconName::Info,
                )
            };

            div()
                .flex()
                .items_center()
                .gap_3()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_1()
                        .px_2()
                        .py_1()
                        .rounded(px(6.0))
                        .bg(bg_color)
                        .child(Icon::new(icon).text_color(text_color))
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::BOLD)
                                .text_color(text_color)
                                .child(format!("{} {}", code, text)),
                        ),
                )
                .when(self.response_time.is_some(), |this| {
                    this.child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            .px_2()
                            .py_1()
                            .rounded(px(6.0))
                            .bg(cx.theme().muted)
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(format!("{}ms", self.response_time.unwrap_or(0))),
                            ),
                    )
                })
                .into_any_element()
        } else {
            div().into_any_element()
        };

        let response_lines: Vec<_> = if response_too_large {
            Vec::new()
        } else {
            self.response_body
                .lines()
                .enumerate()
                .map(|(i, line)| {
                    let line_content: String = if line.is_empty() {
                        " ".to_string()
                    } else {
                        line.to_string()
                    };
                    div()
                        .id(ElementId::Name(format!("line-{}", i).into()))
                        .text_xs()
                        .font_family("monospace")
                        .text_color(cx.theme().foreground)
                        .child(line_content)
                })
                .collect()
        };

        div()
            .flex_1()
            .flex()
            .flex_col()
            .min_h(px(200.0))
            .bg(cx.theme().background)
            .child(Divider::horizontal())
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .p_3()
                    .bg(cx.theme().muted)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                Icon::new(IconName::ArrowDown)
                                    .text_color(cx.theme().muted_foreground),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(cx.theme().foreground)
                                    .child("Response"),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .when(has_response, |this| {
                                this.child(
                                    Button::new("copy-response")
                                        .icon(IconName::Copy)
                                        .label("Copy")
                                        .ghost()
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.copy_response(cx);
                                        })),
                                )
                            })
                            .when(has_response, |this| {
                                this.child(
                                    Button::new("save-response")
                                        .icon(IconName::ArrowDown)
                                        .label("Save")
                                        .ghost()
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.save_response_to_file(window, cx);
                                        })),
                                )
                            })
                            .child(status_badge),
                    ),
            )
            .child(if self.is_loading {
                // Show loading spinner while request is in progress
                div()
                    .id("response-loading")
                    .flex_1()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap_3()
                    .bg(cx.theme().muted)
                    .child(Spinner::new().color(cx.theme().primary))
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child("Sending request..."),
                    )
                    .into_any_element()
            } else if response_too_large {
                let response_size = format_size(self.response_body.len());
                div()
                    .id("response-scroll")
                    .flex_1()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap_2()
                    .p_4()
                    .bg(cx.theme().muted)
                    .child(Icon::new(IconName::TriangleAlert).text_color(hsla(0.12, 0.7, 0.5, 1.0)))
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(cx.theme().foreground)
                            .child("Response too large to display"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(format!("Size: {}", response_size)),
                    )
                    .into_any_element()
            } else if !has_response && self.response_status.is_none() {
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap_4()
                    .p_8()
                    .bg(cx.theme().muted.opacity(0.3))
                    .child(
                        div()
                            .p_4()
                            .rounded_full()
                            .bg(cx.theme().background)
                            .border_1()
                            .border_color(cx.theme().border)
                            .child(
                                Icon::new(IconName::ArrowRight)
                                    .size(px(32.0))
                                    .text_color(cx.theme().primary),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .items_center()
                            .gap_1()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::MEDIUM)
                                    .child("Ready to send"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child("Enter a URL and click Send to see the response"),
                            ),
                    )
                    .into_any_element()
            } else {
                div()
                    .id("response-scroll")
                    .flex_1()
                    .overflow_y_scrollbar()
                    .p_4()
                    .bg(cx.theme().muted)
                    .children(response_lines)
                    .into_any_element()
            })
            .child(Scrollbar::vertical(&self.scroll_handle))
    }
    fn render_status_bar(&self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let branch_name = self
            .current_branch
            .clone()
            .unwrap_or_else(|| "No Repo".to_string());

        div()
            .w_full()
            .h(px(28.0))
            .flex()
            .items_center()
            .justify_between()
            .px_3()
            .bg(cx.theme().muted)
            .border_t_1()
            .border_color(cx.theme().border)
            .text_xs()
            .text_color(cx.theme().muted_foreground)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .cursor_pointer()
                            .flex()
                            .items_center()
                            .gap_1()
                            .hover(|s| s.text_color(cx.theme().foreground))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _, cx| {
                                    this.sidebar_tab = if this.sidebar_tab == SidebarTab::Git {
                                        SidebarTab::Files
                                    } else {
                                        SidebarTab::Git
                                    };
                                    if this.sidebar_tab == SidebarTab::Git {
                                        this.refresh_git_status(cx);
                                    }
                                    cx.notify();
                                }),
                            )
                            .child(Icon::new(IconName::Globe).size(px(14.0)))
                            .child(branch_name),
                    )
                    .child(Divider::vertical())
                    .child(if self.is_loading {
                        "Sending request..."
                    } else {
                        "Ready"
                    }),
            )
            .child(div().flex().items_center().gap_2().child("v0.1.0"))
            .into_any_element()
    }
}

/// Simple URL encoding helper
fn urlencoding(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' | '.' | '~' => c.to_string(),
            ' ' => "+".to_string(),
            _ => format!("%{:02X}", c as u32),
        })
        .collect()
}

const MAX_RESPONSE_DISPLAY_BYTES: usize = 100_000;

fn format_size(bytes: usize) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = 1024.0 * 1024.0;
    let bytes_f = bytes as f64;

    if bytes_f >= MB {
        format!("{:.1} MB", bytes_f / MB)
    } else if bytes_f >= KB {
        format!("{:.1} KB", bytes_f / KB)
    } else {
        format!("{} B", bytes)
    }
}

impl Render for App {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .font_family("Inter, SF Pro Display, system-ui, sans-serif")
            .key_context("ApiClient")
            .on_action(cx.listener(|this, _: &SendRequest, window, cx| {
                this.send_request(window, cx);
            }))
            .on_action(cx.listener(|this, _: &SaveRequest, window, cx| {
                this.save_request(window, cx);
            }))
            .on_action(cx.listener(|this, _: &NewRequest, window, cx| {
                this.save_new_request(window, cx);
            }))
            .on_action(cx.listener(|this, _: &OpenFolder, window, cx| {
                this.open_folder(window, cx);
            }))
            .on_action(cx.listener(|this, _: &ToggleSidebar, _, cx| {
                this.sidebar_visible = !this.sidebar_visible;
                cx.notify();
            }))
            .on_action(cx.listener(|_this, _: &ToggleTheme, window, cx| {
                let current_mode = cx.theme().mode;
                let new_mode = match current_mode {
                    ThemeMode::Light => ThemeMode::Dark,
                    ThemeMode::Dark => ThemeMode::Light,
                };
                Theme::change(new_mode, Some(window), cx);
            }))
            .on_action(cx.listener(|_this, _: &CloseWindow, window, _cx| {
                window.remove_window();
            }))
            .child(self.render_title_bar(window, cx))
            .child(
                h_resizable("main-split")
                    .child(
                        resizable_panel()
                            .size(px(250.0))
                            .visible(self.sidebar_visible)
                            .child(self.render_sidebar(window, cx)),
                    )
                    .child(
                        resizable_panel().child(
                            v_resizable("content-split")
                                .child(
                                    resizable_panel().child(
                                        div()
                                            .size_full()
                                            .flex()
                                            .flex_col()
                                            .child(self.render_request_bar(window, cx))
                                            .child(self.render_tabs(window, cx))
                                            .child(self.render_request_panel(window, cx)),
                                    ),
                                )
                                .child(
                                    resizable_panel().child(self.render_response_panel(window, cx)),
                                ),
                        ),
                    ),
            )
            .child(self.render_status_bar(window, cx))
    }
}

impl App {
    fn copy_response(&self, cx: &mut Context<Self>) {
        if self.response_body.is_empty() {
            return;
        }
        cx.write_to_clipboard(ClipboardItem::new_string(self.response_body.clone()));
    }

    fn save_response_to_file(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.response_body.is_empty() {
            return;
        }

        let response_text = self.response_body.clone();
        cx.spawn_in(window, async move |_this, _cx| {
            let file = rfd::AsyncFileDialog::new()
                .set_title("Save Response")
                .set_file_name("response.txt")
                .save_file()
                .await;

            if let Some(file) = file {
                let path = file.path().to_path_buf();
                let _ = std::fs::write(path, response_text);
            }
        })
        .detach();
    }
}
