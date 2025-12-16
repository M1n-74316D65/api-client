use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::badge::Badge;
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::divider::Divider;
use gpui_component::input::{Input, InputState};
use gpui_component::scroll::{ScrollableElement, Scrollbar};
use gpui_component::tab::{Tab, TabBar};
use gpui_component::*;
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

    fn bg_color(&self) -> Hsla {
        match self {
            HttpMethod::Get => hsla(0.35, 0.4, 0.15, 1.0),
            HttpMethod::Post => hsla(0.55, 0.4, 0.15, 1.0),
            HttpMethod::Put => hsla(0.12, 0.4, 0.15, 1.0),
            HttpMethod::Delete => hsla(0.0, 0.4, 0.15, 1.0),
            HttpMethod::Patch => hsla(0.75, 0.3, 0.15, 1.0),
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

pub struct App {
    url_input: Entity<InputState>,
    body_input: Entity<InputState>,
    params: Vec<KeyValuePair>,
    headers: Vec<KeyValuePair>,
    response_body: String,
    scroll_handle: ScrollHandle,
    sidebar_scroll: ScrollHandle,
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
}

impl App {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let url_input = cx.new(|cx| {
            let mut state = InputState::new(window, cx);
            state.set_placeholder("Enter request URL...", window, cx);
            state.set_value("https://httpbin.org/get", window, cx);
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

        Self {
            url_input,
            body_input,
            params,
            headers,
            response_body: String::new(),
            scroll_handle: ScrollHandle::new(),
            sidebar_scroll: ScrollHandle::new(),
            method: HttpMethod::Get,
            active_tab: RequestTab::Params,
            is_loading: false,
            response_status: None,
            response_time: None,
            // Sidebar state
            sidebar_visible: true,
            current_folder: None,
            saved_requests: Vec::new(),
            selected_request: None,
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

                            // Try to format JSON response
                            app.response_body = if let Ok(json) =
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

            if let Some(folder) = folder {
                let path = folder.path().to_path_buf();
                cx.update(|_window, cx| {
                    this.update(cx, |app, cx| {
                        app.current_folder = Some(path);
                        app.load_folder(cx);
                        cx.notify();
                    });
                });
            }
        })
        .detach();
    }

    /// Load requests from current folder
    fn load_folder(&mut self, _cx: &mut Context<Self>) {
        self.saved_requests.clear();

        if let Some(folder) = &self.current_folder {
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

                            self.saved_requests.push(FileEntry { name, path, method });
                        }
                    }
                }
            }
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

    /// Load a saved request into the editor
    fn load_request(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(entry) = self.saved_requests.get(index) {
            if let Ok(content) = std::fs::read_to_string(&entry.path) {
                if let Ok(request) = serde_json::from_str::<SavedRequest>(&content) {
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
            .w(px(240.0))
            .h_full()
            .flex()
            .flex_col()
            .bg(hsla(0.0, 0.0, 0.05, 1.0))
            .border_r_1()
            .border_color(hsla(0.0, 0.0, 0.15, 1.0))
            // Header
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .p_3()
                    .border_b_1()
                    .border_color(hsla(0.0, 0.0, 0.15, 1.0))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                Icon::new(IconName::FolderOpen)
                                    .text_color(hsla(0.12, 0.7, 0.5, 1.0)),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(hsla(0.0, 0.0, 0.8, 1.0))
                                    .child("Requests"),
                            ),
                    )
                    .child(
                        div()
                            .id("open-folder-btn")
                            .p_1()
                            .rounded(px(4.0))
                            .cursor_pointer()
                            .hover(|s| s.bg(hsla(0.0, 0.0, 0.15, 1.0)))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, window, cx| {
                                    this.open_folder(window, cx);
                                    cx.notify();
                                }),
                            )
                            .child(
                                Icon::new(IconName::FolderOpen)
                                    .text_color(hsla(0.0, 0.0, 0.5, 1.0)),
                            ),
                    ),
            )
            // Folder path
            .child(
                div()
                    .px_3()
                    .py_2()
                    .text_xs()
                    .text_color(hsla(0.0, 0.0, 0.5, 1.0))
                    .child(folder_name),
            )
            // File list
            .child(div().flex_1().overflow_y_scrollbar().children(
                self.saved_requests.iter().enumerate().map(|(i, entry)| {
                    let is_selected = self.selected_request == Some(i);
                    let method_color = entry
                        .method
                        .as_ref()
                        .map(|m| m.color())
                        .unwrap_or(hsla(0.0, 0.0, 0.5, 1.0));
                    let method_str = entry.method.as_ref().map(|m| m.as_str()).unwrap_or("???");
                    let name = entry.name.clone();

                    div()
                        .id(ElementId::Name(format!("request-{}", i).into()))
                        .flex()
                        .items_center()
                        .gap_2()
                        .px_3()
                        .py_2()
                        .cursor_pointer()
                        .bg(if is_selected {
                            hsla(0.55, 0.4, 0.2, 1.0)
                        } else {
                            hsla(0.0, 0.0, 0.0, 0.0)
                        })
                        .hover(|s| s.bg(hsla(0.0, 0.0, 0.1, 1.0)))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, _, window, cx| {
                                this.load_request(i, window, cx);
                            }),
                        )
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::BOLD)
                                .text_color(method_color)
                                .min_w(px(40.0))
                                .child(method_str),
                        )
                        .child(
                            div()
                                .text_sm()
                                .text_color(hsla(0.0, 0.0, 0.8, 1.0))
                                .overflow_x_hidden()
                                .child(name),
                        )
                }),
            ))
            // Empty state
            .when(self.saved_requests.is_empty(), |this| {
                this.child(
                    div()
                        .flex_1()
                        .flex()
                        .flex_col()
                        .items_center()
                        .justify_center()
                        .gap_2()
                        .p_4()
                        .child(Icon::new(IconName::FolderOpen).text_color(hsla(0.0, 0.0, 0.3, 1.0)))
                        .child(
                            div()
                                .text_sm()
                                .text_color(hsla(0.0, 0.0, 0.4, 1.0))
                                .child("No requests"),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(hsla(0.0, 0.0, 0.3, 1.0))
                                .child("Click folder icon to open"),
                        ),
                )
            })
    }

    fn render_title_bar(&self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        TitleBar::new().child(
            div()
                .flex()
                .items_center()
                .gap_3()
                .child(Icon::new(IconName::Globe).text_color(hsla(0.55, 0.8, 0.6, 1.0)))
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::BOLD)
                        .text_color(hsla(0.0, 0.0, 0.95, 1.0))
                        .child("API Client"),
                )
                .child(Badge::new().child("v1.0")),
        )
    }

    fn render_request_bar(&self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let method = self.method.clone();
        let method_color = method.color();
        let method_bg = method.bg_color();
        let method_text = method.as_str();

        div()
            .flex()
            .items_center()
            .gap_3()
            .p_4()
            .bg(hsla(0.0, 0.0, 0.11, 1.0))
            .child(
                // Method selector button with icon
                div()
                    .id("method-selector")
                    .flex()
                    .items_center()
                    .gap_2()
                    .px_3()
                    .py_2()
                    .rounded(px(8.0))
                    .bg(method_bg)
                    .border_1()
                    .border_color(method_color.opacity(0.3))
                    .cursor_pointer()
                    .hover(|s| s.bg(method_bg.opacity(0.8)))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| {
                            this.method = this.method.next();
                            cx.notify();
                        }),
                    )
                    .child(
                        div()
                            .font_weight(FontWeight::BOLD)
                            .text_sm()
                            .text_color(method_color)
                            .child(method_text),
                    )
                    .child(Icon::new(IconName::ChevronDown).text_color(method_color.opacity(0.7))),
            )
            .child(
                div()
                    .flex_1()
                    .px_3()
                    .py_1()
                    .rounded(px(8.0))
                    .bg(hsla(0.0, 0.0, 0.06, 1.0))
                    .border_1()
                    .border_color(hsla(0.0, 0.0, 0.2, 1.0))
                    .child(Input::new(&self.url_input).appearance(false)),
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
            .bg(hsla(0.0, 0.0, 0.09, 1.0))
            .border_b_1()
            .border_color(hsla(0.0, 0.0, 0.15, 1.0))
            .child(
                TabBar::new("request-tabs")
                    .child(
                        Tab::new()
                            .selected(active_tab == RequestTab::Params)
                            .child(div().flex().items_center().gap_2().child("Params").when(
                                param_count > 0,
                                |this| {
                                    this.child(
                                        div()
                                            .px_1()
                                            .rounded(px(4.0))
                                            .bg(hsla(0.55, 0.5, 0.3, 1.0))
                                            .text_xs()
                                            .child(format!("{}", param_count)),
                                    )
                                },
                            ))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.active_tab = RequestTab::Params;
                                cx.notify();
                            })),
                    )
                    .child(
                        Tab::new()
                            .selected(active_tab == RequestTab::Headers)
                            .child(div().flex().items_center().gap_2().child("Headers").when(
                                header_count > 0,
                                |this| {
                                    this.child(
                                        div()
                                            .px_1()
                                            .rounded(px(4.0))
                                            .bg(hsla(0.12, 0.5, 0.3, 1.0))
                                            .text_xs()
                                            .child(format!("{}", header_count)),
                                    )
                                },
                            ))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.active_tab = RequestTab::Headers;
                                cx.notify();
                            })),
                    )
                    .child(
                        Tab::new()
                            .selected(active_tab == RequestTab::Body)
                            .child("Body")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.active_tab = RequestTab::Body;
                                cx.notify();
                            })),
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
            .bg(hsla(0.0, 0.0, 0.06, 1.0))
            .border_1()
            .border_color(hsla(0.0, 0.0, 0.15, 1.0))
            .child(
                div()
                    .flex_1()
                    .child(Input::new(&pair.key).appearance(false)),
            )
            .child(div().text_color(hsla(0.0, 0.0, 0.3, 1.0)).child("="))
            .child(
                div()
                    .flex_1()
                    .child(Input::new(&pair.value).appearance(false)),
            )
            .child(
                div()
                    .id(ElementId::Name(
                        format!(
                            "delete-{}-{}",
                            if is_param { "param" } else { "header" },
                            index
                        )
                        .into(),
                    ))
                    .p_1()
                    .rounded(px(4.0))
                    .cursor_pointer()
                    .text_color(hsla(0.0, 0.0, 0.4, 1.0))
                    .hover(|s| {
                        s.text_color(hsla(0.0, 0.7, 0.6, 1.0))
                            .bg(hsla(0.0, 0.5, 0.2, 0.3))
                    })
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| {
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
                        }),
                    )
                    .child(Icon::new(IconName::Delete)),
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
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .mb_4()
                            .child(Icon::new(IconName::Search).text_color(hsla(0.0, 0.0, 0.5, 1.0)))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(hsla(0.0, 0.0, 0.5, 1.0))
                                    .child("Query parameters will be appended to the URL"),
                            ),
                    )
                    .children(rows)
                    .child(
                        div()
                            .id("add-param")
                            .flex()
                            .items_center()
                            .gap_2()
                            .mt_2()
                            .px_3()
                            .py_2()
                            .rounded(px(6.0))
                            .text_sm()
                            .cursor_pointer()
                            .text_color(hsla(0.55, 0.7, 0.5, 1.0))
                            .border_1()
                            .border_color(hsla(0.55, 0.5, 0.3, 0.3))
                            .hover(|s| s.bg(hsla(0.55, 0.5, 0.2, 0.2)))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, window, cx| {
                                    this.add_param(window, cx);
                                }),
                            )
                            .child(Icon::new(IconName::Plus))
                            .child("Add Parameter"),
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
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .mb_4()
                            .child(
                                Icon::new(IconName::Settings).text_color(hsla(0.0, 0.0, 0.5, 1.0)),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(hsla(0.0, 0.0, 0.5, 1.0))
                                    .child("HTTP headers to include in the request"),
                            ),
                    )
                    .children(rows)
                    .child(
                        div()
                            .id("add-header")
                            .flex()
                            .items_center()
                            .gap_2()
                            .mt_2()
                            .px_3()
                            .py_2()
                            .rounded(px(6.0))
                            .text_sm()
                            .cursor_pointer()
                            .text_color(hsla(0.12, 0.7, 0.5, 1.0))
                            .border_1()
                            .border_color(hsla(0.12, 0.5, 0.3, 0.3))
                            .hover(|s| s.bg(hsla(0.12, 0.5, 0.2, 0.2)))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, window, cx| {
                                    this.add_header(window, cx);
                                }),
                            )
                            .child(Icon::new(IconName::Plus))
                            .child("Add Header"),
                    )
                    .into_any_element()
            }
            RequestTab::Body => div()
                .size_full()
                .flex()
                .flex_col()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .mb_4()
                        .child(Icon::new(IconName::File).text_color(hsla(0.0, 0.0, 0.5, 1.0)))
                        .child(
                            div()
                                .text_xs()
                                .text_color(hsla(0.0, 0.0, 0.5, 1.0))
                                .child("Request body for POST, PUT, PATCH requests"),
                        ),
                )
                .child(
                    div()
                        .flex_1()
                        .p_3()
                        .rounded(px(8.0))
                        .bg(hsla(0.0, 0.0, 0.04, 1.0))
                        .border_1()
                        .border_color(hsla(0.0, 0.0, 0.15, 1.0))
                        .child(Input::new(&self.body_input).appearance(false)),
                )
                .into_any_element(),
        };

        div()
            .flex_1()
            .p_4()
            .bg(hsla(0.0, 0.0, 0.07, 1.0))
            .child(content)
    }

    fn render_response_panel(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> impl IntoElement {
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
                            .bg(hsla(0.0, 0.0, 0.15, 1.0))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(hsla(0.0, 0.0, 0.6, 1.0))
                                    .child(format!("{}ms", self.response_time.unwrap_or(0))),
                            ),
                    )
                })
                .into_any_element()
        } else {
            div()
                .flex()
                .items_center()
                .gap_2()
                .text_sm()
                .text_color(hsla(0.0, 0.0, 0.4, 1.0))
                .child(Icon::new(IconName::Info))
                .child("Send a request to see the response")
                .into_any_element()
        };

        let response_lines: Vec<_> = self
            .response_body
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
                    .text_color(hsla(0.0, 0.0, 0.8, 1.0))
                    .child(line_content)
            })
            .collect();

        div()
            .flex_1()
            .flex()
            .flex_col()
            .min_h(px(200.0))
            .bg(hsla(0.0, 0.0, 0.05, 1.0))
            .child(Divider::horizontal())
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .p_3()
                    .bg(hsla(0.0, 0.0, 0.08, 1.0))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                Icon::new(IconName::ArrowDown).text_color(hsla(0.0, 0.0, 0.6, 1.0)),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(hsla(0.0, 0.0, 0.8, 1.0))
                                    .child("Response"),
                            ),
                    )
                    .child(status_badge),
            )
            .child(
                div()
                    .id("response-scroll")
                    .flex_1()
                    .overflow_y_scrollbar()
                    .p_4()
                    .bg(hsla(0.0, 0.0, 0.03, 1.0))
                    .children(response_lines),
            )
            .child(Scrollbar::vertical(&self.scroll_handle))
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

impl Render for App {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(hsla(0.0, 0.0, 0.07, 1.0))
            .text_color(hsla(0.0, 0.0, 0.9, 1.0))
            .font_family("Inter, SF Pro Display, system-ui, sans-serif")
            .child(self.render_title_bar(window, cx))
            .child(
                // Main content area with sidebar
                div()
                    .flex_1()
                    .flex()
                    .overflow_hidden()
                    // Sidebar
                    .when(self.sidebar_visible, |this| {
                        this.child(self.render_sidebar(window, cx))
                    })
                    // Main panel
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .flex_col()
                            .overflow_hidden()
                            .child(self.render_request_bar(window, cx))
                            .child(self.render_tabs(window, cx))
                            .child(self.render_request_panel(window, cx))
                            .child(self.render_response_panel(window, cx)),
                    ),
            )
    }
}
