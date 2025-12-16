use gpui::*;
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::{Input, InputState};
use gpui_component::scroll::Scrollbar;
use gpui_component::tab::{Tab, TabBar};
use gpui_component::*;

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

pub struct App {
    url_input: Entity<InputState>,
    body_input: Entity<InputState>,
    headers_input: Entity<InputState>,
    response_body: String,
    scroll_handle: ScrollHandle,
    method: HttpMethod,
    active_tab: RequestTab,
    is_loading: bool,
    response_status: Option<(u16, String)>,
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
            state.set_placeholder("Request body (JSON)...", window, cx);
            state
        });

        let headers_input = cx.new(|cx| {
            let mut state = InputState::new(window, cx);
            state.set_placeholder("Headers (Key: Value)", window, cx);
            state
        });

        Self {
            url_input,
            body_input,
            headers_input,
            response_body: String::new(),
            scroll_handle: ScrollHandle::new(),
            method: HttpMethod::Get,
            active_tab: RequestTab::Params,
            is_loading: false,
            response_status: None,
        }
    }

    fn send_request(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let url = self.url_input.read(cx).value().to_string();
        let body = self.body_input.read(cx).value().to_string();
        let headers_text = self.headers_input.read(cx).value().to_string();
        let method = self.method.clone();

        if url.is_empty() {
            return;
        }

        self.is_loading = true;
        self.response_status = None;
        self.response_body.clear();
        cx.notify();

        cx.spawn_in(window, async move |this, cx| {
            let result = Self::execute_request(&url, &method, &body, &headers_text).await;

            cx.update(|_window, cx| {
                this.update(cx, |app, cx| {
                    app.is_loading = false;
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
        headers_text: &str,
    ) -> Result<(u16, String), String> {
        let client = reqwest::Client::new();

        let mut builder = match method {
            HttpMethod::Get => client.get(url),
            HttpMethod::Post => client.post(url),
            HttpMethod::Put => client.put(url),
            HttpMethod::Delete => client.delete(url),
            HttpMethod::Patch => client.patch(url),
        };

        // Parse and add headers
        for line in headers_text.lines() {
            if let Some((key, value)) = line.split_once(':') {
                builder = builder.header(key.trim(), value.trim());
            }
        }

        // Add body for methods that support it
        if !body.is_empty()
            && matches!(
                method,
                HttpMethod::Post | HttpMethod::Put | HttpMethod::Patch
            )
        {
            builder = builder
                .header("Content-Type", "application/json")
                .body(body.to_string());
        }

        let response = builder.send().await.map_err(|e| e.to_string())?;
        let status = response.status().as_u16();
        let text = response.text().await.map_err(|e| e.to_string())?;

        Ok((status, text))
    }

    fn render_title_bar(&self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        TitleBar::new().child(
            div().flex().items_center().gap_2().child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::BOLD)
                    .text_color(hsla(0.0, 0.0, 0.95, 1.0))
                    .child("⚡ API Client"),
            ),
        )
    }

    fn render_request_bar(&self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let method = self.method.clone();
        let method_color = method.color();
        let method_text = method.as_str();

        div()
            .flex()
            .gap_3()
            .p_4()
            .bg(hsla(0.0, 0.0, 0.12, 1.0))
            .border_b_1()
            .border_color(hsla(0.0, 0.0, 0.2, 1.0))
            .child(
                // Method selector button (cycles through methods on click)
                div()
                    .id("method-selector")
                    .px_3()
                    .py_2()
                    .rounded(px(6.0))
                    .bg(hsla(0.0, 0.0, 0.18, 1.0))
                    .border_1()
                    .border_color(hsla(0.0, 0.0, 0.25, 1.0))
                    .cursor_pointer()
                    .hover(|s| s.bg(hsla(0.0, 0.0, 0.22, 1.0)))
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
                    ),
            )
            .child(div().flex_1().child(Input::new(&self.url_input)))
            .child(
                Button::new("send")
                    .primary()
                    .label("Send")
                    .loading(self.is_loading)
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.send_request(window, cx);
                    })),
            )
    }

    fn render_tabs(&self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let active_tab = self.active_tab.clone();

        div()
            .bg(hsla(0.0, 0.0, 0.10, 1.0))
            .border_b_1()
            .border_color(hsla(0.0, 0.0, 0.2, 1.0))
            .child(
                TabBar::new("request-tabs")
                    .child(
                        Tab::new()
                            .selected(active_tab == RequestTab::Params)
                            .child("Params")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.active_tab = RequestTab::Params;
                                cx.notify();
                            })),
                    )
                    .child(
                        Tab::new()
                            .selected(active_tab == RequestTab::Headers)
                            .child("Headers")
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

    fn render_request_panel(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .flex_1()
            .p_4()
            .bg(hsla(0.0, 0.0, 0.08, 1.0))
            .child(match self.active_tab {
                RequestTab::Params => div()
                    .text_sm()
                    .text_color(hsla(0.0, 0.0, 0.5, 1.0))
                    .child("Query parameters are extracted from the URL")
                    .into_any_element(),
                RequestTab::Headers => div()
                    .size_full()
                    .child(Input::new(&self.headers_input).appearance(false))
                    .into_any_element(),
                RequestTab::Body => div()
                    .size_full()
                    .child(Input::new(&self.body_input).appearance(false))
                    .into_any_element(),
            })
    }

    fn render_response_panel(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let status_badge = if let Some((code, text)) = &self.response_status {
            let (bg_color, text_color) = if *code >= 200 && *code < 300 {
                (hsla(0.35, 0.6, 0.25, 1.0), hsla(0.35, 0.8, 0.65, 1.0))
            } else if *code >= 400 {
                (hsla(0.0, 0.6, 0.25, 1.0), hsla(0.0, 0.8, 0.65, 1.0))
            } else {
                (hsla(0.0, 0.0, 0.2, 1.0), hsla(0.0, 0.0, 0.7, 1.0))
            };

            div()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .px_2()
                        .py_1()
                        .rounded(px(4.0))
                        .bg(bg_color)
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .text_color(text_color)
                        .child(format!("{} {}", code, text)),
                )
                .into_any_element()
        } else {
            div()
                .text_sm()
                .text_color(hsla(0.0, 0.0, 0.5, 1.0))
                .child("No response yet")
                .into_any_element()
        };

        // Build response lines as separate elements
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
                    .text_color(hsla(0.0, 0.0, 0.85, 1.0))
                    .child(line_content)
            })
            .collect();

        div()
            .flex_1()
            .flex()
            .flex_col()
            .min_h(px(200.0))
            .bg(hsla(0.0, 0.0, 0.06, 1.0))
            .border_t_1()
            .border_color(hsla(0.0, 0.0, 0.2, 1.0))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .p_3()
                    .bg(hsla(0.0, 0.0, 0.10, 1.0))
                    .border_b_1()
                    .border_color(hsla(0.0, 0.0, 0.2, 1.0))
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(hsla(0.0, 0.0, 0.8, 1.0))
                            .child("Response"),
                    )
                    .child(status_badge),
            )
            .child(
                div()
                    .id("response-scroll")
                    .flex_1()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll_handle)
                    .p_4()
                    .bg(hsla(0.0, 0.0, 0.04, 1.0))
                    .children(response_lines),
            )
            .child(Scrollbar::vertical(&self.scroll_handle))
    }
}

impl Render for App {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(hsla(0.0, 0.0, 0.08, 1.0))
            .text_color(hsla(0.0, 0.0, 0.9, 1.0))
            .font_family("Inter, SF Pro Display, system-ui, sans-serif")
            .child(self.render_title_bar(window, cx))
            .child(self.render_request_bar(window, cx))
            .child(self.render_tabs(window, cx))
            .child(self.render_request_panel(window, cx))
            .child(self.render_response_panel(window, cx))
    }
}
