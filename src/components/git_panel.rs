use crate::git::{FileStatus, GitFileChange};
use gpui::*;
use gpui_component::{
    accordion::Accordion,
    button::{Button, ButtonVariants},
    input::{Input, InputState},
    tag::Tag,
    v_flex, ActiveTheme, IconName, Sizable,
};

pub struct GitPanel {
    pub changes: Vec<GitFileChange>,
    pub commit_message: Entity<InputState>,
}

impl GitPanel {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let commit_message =
            cx.new(|cx| InputState::new(window, cx).placeholder("Commit message..."));
        Self {
            changes: Vec::new(),
            commit_message,
        }
    }

    pub fn set_changes(&mut self, changes: Vec<GitFileChange>) {
        self.changes = changes;
    }

    fn render_file_row(change: &GitFileChange, cx: &Context<Self>) -> impl IntoElement {
        let tag_element = match change.status {
            FileStatus::New => Tag::success().small().child("U"),
            FileStatus::Modified => Tag::warning().small().child("M"),
            FileStatus::Deleted => Tag::danger().small().child("D"),
            _ => Tag::secondary().small().child("?"),
        };

        let file_name = change
            .path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        div()
            .id(ElementId::Name(format!("git-file-{}", file_name).into()))
            .flex()
            .items_center()
            .gap_2()
            .px_2()
            .py_1()
            .rounded(px(4.0))
            .cursor_pointer()
            .hover(|s| s.bg(cx.theme().secondary))
            .child(tag_element)
            .child(
                div()
                    .flex_1()
                    .text_sm()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis()
                    .child(file_name),
            )
    }
}

impl Render for GitPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let staged: Vec<_> = self.changes.iter().filter(|c| c.is_staged).collect();
        let unstaged: Vec<_> = self.changes.iter().filter(|c| !c.is_staged).collect();

        let staged_count = staged.len();
        let unstaged_count = unstaged.len();

        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(cx.theme().sidebar)
            .child(
                // Scrollable content area
                div().flex_1().overflow_hidden().p_3().child(
                    v_flex()
                        .gap_3()
                        // Staged Changes Section
                        .child(
                            Accordion::new("git-accordion")
                                .multiple(true)
                                .item(|item| {
                                    item.title(format!("Staged Changes ({})", staged_count))
                                        .child(if staged.is_empty() {
                                            div()
                                                .text_xs()
                                                .text_color(cx.theme().muted_foreground)
                                                .p_2()
                                                .child("No staged changes")
                                                .into_any_element()
                                        } else {
                                            v_flex()
                                                .gap_1()
                                                .children(
                                                    staged
                                                        .iter()
                                                        .map(|c| Self::render_file_row(c, cx)),
                                                )
                                                .into_any_element()
                                        })
                                })
                                .item(|item| {
                                    item.title(format!("Unstaged Changes ({})", unstaged_count))
                                        .child(if unstaged.is_empty() {
                                            div()
                                                .text_xs()
                                                .text_color(cx.theme().muted_foreground)
                                                .p_2()
                                                .child("No unstaged changes")
                                                .into_any_element()
                                        } else {
                                            v_flex()
                                                .gap_1()
                                                .children(
                                                    unstaged
                                                        .iter()
                                                        .map(|c| Self::render_file_row(c, cx)),
                                                )
                                                .into_any_element()
                                        })
                                }),
                        ),
                ),
            )
            // Commit section at bottom
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .p_3()
                    .border_t_1()
                    .border_color(cx.theme().border)
                    .child(Input::new(&self.commit_message))
                    .child(
                        Button::new("commit")
                            .primary()
                            .icon(IconName::Check)
                            .label("Commit")
                            .w_full(),
                    ),
            )
    }
}
