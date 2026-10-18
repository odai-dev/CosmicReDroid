// src/ui/image_hub.rs
// Image Factory view for CosmicReDroid with Dockerfile synthesis and build execution.

use crate::app::Message;
use crate::models::{AndroidVersion, ImageDownloadProgress};
use cosmic::iced::Alignment;
use cosmic::widget;
use cosmic::Element;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone)]
pub struct ImageFactoryState {
    pub selected_version: AndroidVersion,
    pub include_libndk: bool,
    pub include_gapps: bool,
    pub include_magisk: bool,
    pub is_building: bool,
    pub build_logs: Vec<String>,
}

impl Default for ImageFactoryState {
    fn default() -> Self {
        Self {
            selected_version: AndroidVersion::Android13,
            include_libndk: true,
            include_gapps: true,
            include_magisk: false,
            is_building: false,
            build_logs: vec!["Ready to generate and build custom ReDroid images.".to_string()],
        }
    }
}

impl ImageFactoryState {
    pub fn generate_dockerfile(&self) -> String {
        let tag = self.selected_version.image_tag();
        let mut df = format!("FROM {}\n\n", tag);

        if self.include_libndk {
            df.push_str("# Add libndk ARM translation layer\n");
            df.push_str("ADD https://github.com/remote-android/redroid-modules/releases/download/v1.0.0/libndk.tar.xz /\n");
            df.push_str("RUN tar -xvf /libndk.tar.xz -C / && rm /libndk.tar.xz\n\n");
        }

        if self.include_gapps {
            df.push_str("# Add MindTheGapps Google Play Services\n");
            df.push_str("ADD https://github.com/remote-android/redroid-modules/releases/download/v1.0.0/gapps.tar.xz /\n");
            df.push_str("RUN tar -xvf /gapps.tar.xz -C / && rm /gapps.tar.xz\n\n");
        }

        if self.include_magisk {
            df.push_str("# Add Kitsune Magisk Systemless Root\n");
            df.push_str("ADD https://github.com/remote-android/redroid-modules/releases/download/v1.0.0/magisk.tar.xz /\n");
            df.push_str("RUN tar -xvf /magisk.tar.xz -C / && rm /magisk.tar.xz\n\n");
        }

        df
    }
}

pub fn view<'a>(
    state: &'a ImageFactoryState,
    available_images: &'a HashSet<String>,
    active_downloads: &'a HashMap<String, ImageDownloadProgress>,
) -> Element<'a, Message> {
    let mut content = widget::Column::new()
        .spacing(16)
        .padding(16)
        .width(cosmic::iced::Length::Fill);

    // Header
    let header = widget::Column::new()
        .spacing(4)
        .push(widget::text::title2("Image Management & Factory"))
        .push(widget::text::body(
            "Download official ReDroid base images or synthesize customized builds with ARM translation and Google Play.",
        ));
    content = content.push(header);

    // Section 1: Official ReDroid Base Images
    let mut images_list = widget::Column::new().spacing(12);

    for version in AndroidVersion::ALL {
        let tag = version.image_tag();
        let is_downloaded = available_images.contains(tag);
        let download = active_downloads.get(tag);

        let mut row_content = widget::Column::new().spacing(8);

        let status_badge = if is_downloaded {
            widget::container(
                widget::Row::new()
                    .spacing(6)
                    .align_y(Alignment::Center)
                    .push(widget::icon::from_name("object-select-symbolic").size(14))
                    .push(widget::text::caption("Cached Locally")),
            )
            .padding([2, 8])
            .class(cosmic::theme::Container::Card)
        } else if download.is_some() {
            widget::container(
                widget::Row::new()
                    .spacing(6)
                    .align_y(Alignment::Center)
                    .push(widget::icon::from_name("folder-download-symbolic").size(14))
                    .push(widget::text::caption("Downloading...")),
            )
            .padding([2, 8])
            .class(cosmic::theme::Container::Card)
        } else {
            widget::container(
                widget::Row::new()
                    .spacing(6)
                    .align_y(Alignment::Center)
                    .push(widget::icon::from_name("software-update-available-symbolic").size(14))
                    .push(widget::text::caption("Not Downloaded")),
            )
            .padding([2, 8])
            .class(cosmic::theme::Container::Card)
        };

        let action_btn = if is_downloaded {
            widget::button::standard("Re-download")
                .leading_icon(widget::icon::from_name("view-refresh-symbolic"))
                .on_press(Message::StartImageDownload(tag.to_string()))
        } else if download.is_some() {
            widget::button::destructive("Cancel")
                .leading_icon(widget::icon::from_name("process-stop-symbolic"))
                .on_press(Message::CancelImageDownload(tag.to_string()))
        } else {
            widget::button::suggested("Download Image")
                .leading_icon(widget::icon::from_name("folder-download-symbolic"))
                .on_press(Message::StartImageDownload(tag.to_string()))
        };

        let main_line = widget::Row::new()
            .spacing(12)
            .align_y(Alignment::Center)
            .push(widget::text::title4(version.display_label()))
            .push(widget::text::caption(tag))
            .push(status_badge)
            .push(action_btn);

        row_content = row_content.push(main_line);

        if let Some(prog) = download {
            let prog_row = widget::Column::new()
                .spacing(4)
                .push(
                    widget::Row::new()
                        .spacing(8)
                        .push(widget::icon::from_name("folder-download-symbolic").size(14))
                        .push(widget::text::caption(prog.formatted_progress())),
                )
                .push(widget::progress_bar::determinate_linear(prog.progress_fraction));
            row_content = row_content.push(prog_row);

            if let Some(ref err) = prog.error {
                let err_box = widget::container(
                    widget::Row::new()
                        .spacing(8)
                        .push(widget::icon::from_name("dialog-error-symbolic").size(14))
                        .push(widget::text::caption(err)),
                )
                .padding([4, 8])
                .class(cosmic::theme::Container::Card);
                row_content = row_content.push(err_box);
            }
        }

        let item_card = widget::container(row_content)
            .padding(12)
            .class(cosmic::theme::Container::Card)
            .width(cosmic::iced::Length::Fill);

        images_list = images_list.push(item_card);
    }

    let official_section = widget::container(
        widget::Column::new()
            .spacing(12)
            .push(widget::text::title3("Official ReDroid Base Images"))
            .push(images_list),
    )
    .padding(16)
    .class(cosmic::theme::Container::Card)
    .width(cosmic::iced::Length::Fill);

    content = content.push(official_section);

    // Section 2: Image Factory Custom Synthesis Options
    let ver_idx = match state.selected_version {
        AndroidVersion::Android11 => 0,
        AndroidVersion::Android13 => 1,
        AndroidVersion::Android14 => 2,
    };

    let options_card = widget::container(
        widget::Column::new()
            .spacing(12)
            .push(widget::text::title3("Custom Image Synthesis"))
            .push(widget::text::caption("Target Base Version"))
            .push(widget::dropdown(
                AndroidVersion::NAMES,
                Some(ver_idx),
                Message::ImageFactoryVersionSelected,
            ))
            .push(
                widget::toggler(state.include_libndk)
                    .label("Include libndk ARM64 Translation".to_string())
                    .on_toggle(Message::ImageFactoryToggleLibndk),
            )
            .push(
                widget::toggler(state.include_gapps)
                    .label("Include MindTheGapps (Google Play Store & Services)".to_string())
                    .on_toggle(Message::ImageFactoryToggleGapps),
            )
            .push(
                widget::toggler(state.include_magisk)
                    .label("Include Kitsune Magisk (Systemless Root & Zygisk)".to_string())
                    .on_toggle(Message::ImageFactoryToggleMagisk),
            ),
    )
    .padding(16)
    .class(cosmic::theme::Container::Card)
    .width(cosmic::iced::Length::Fill);

    content = content.push(options_card);

    // Section 3: Dockerfile Preview Card
    let dockerfile_text = state.generate_dockerfile();
    let preview_card = widget::container(
        widget::Column::new()
            .spacing(8)
            .push(widget::text::title4("Generated Dockerfile Preview"))
            .push(
                widget::container(widget::text::body(dockerfile_text))
                    .padding(12)
                    .class(cosmic::theme::Container::Card)
                    .width(cosmic::iced::Length::Fill),
            ),
    )
    .padding(16)
    .class(cosmic::theme::Container::Card)
    .width(cosmic::iced::Length::Fill);

    content = content.push(preview_card);

    // Section 4: Action and Build Streamer Card
    let mut build_card_col = widget::Column::new().spacing(12);

    let action_row = if state.is_building {
        widget::Row::new()
            .spacing(12)
            .align_y(Alignment::Center)
            .push(
                widget::button::suggested("Building Custom Image...")
                    .leading_icon(widget::icon::from_name("system-run-symbolic")),
            )
            .push(widget::text::caption(
                "Executing docker build asynchronously. Check logs below.",
            ))
    } else {
        widget::Row::new()
            .spacing(12)
            .align_y(Alignment::Center)
            .push(
                widget::button::suggested("Build Custom Image")
                    .leading_icon(widget::icon::from_name("applications-engineering-symbolic"))
                    .on_press(Message::ImageFactoryStartBuild),
            )
            .push(widget::text::caption(
                "Compiles container image with selected features.",
            ))
    };

    build_card_col = build_card_col.push(action_row);

    let mut log_col = widget::Column::new().spacing(4);
    for line in &state.build_logs {
        log_col = log_col.push(widget::text::caption(line));
    }

    let logs_container = widget::container(widget::scrollable(log_col))
        .padding(12)
        .class(cosmic::theme::Container::Card)
        .width(cosmic::iced::Length::Fill)
        .height(180);

    build_card_col = build_card_col.push(logs_container);

    let build_card = widget::container(build_card_col)
        .padding(16)
        .class(cosmic::theme::Container::Card)
        .width(cosmic::iced::Length::Fill);

    content = content.push(build_card);

    widget::scrollable(content).into()
}
