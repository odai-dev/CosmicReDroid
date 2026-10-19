//! Image Factory view for CosmicReDroid with Dockerfile synthesis and build execution.

use crate::app::Message;
use crate::models::{AndroidVersion, ImageDownloadProgress};
use cosmic::iced::Alignment;
use cosmic::widget;
use cosmic::Element;
use std::collections::{HashMap, HashSet};

/// Sub-tabs for the Image Management view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageHubTab {
    BaseImages,
    CustomStudio,
}

impl ImageHubTab {
    pub const ALL: [ImageHubTab; 2] = [ImageHubTab::BaseImages, ImageHubTab::CustomStudio];

    pub fn display_label(&self) -> &'static str {
        match self {
            ImageHubTab::BaseImages => "Official Base Images",
            ImageHubTab::CustomStudio => "Custom Image Studio",
        }
    }
}

/// 1-Click quick presets for custom ReDroid images.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageFactoryPreset {
    GamingGapps,
    GappsOnly,
    Rooted,
    Clean,
    Custom,
}

impl ImageFactoryPreset {
    pub fn display_label(&self) -> &'static str {
        match self {
            ImageFactoryPreset::GamingGapps => "Gaming & Google Play (Recommended)",
            ImageFactoryPreset::GappsOnly => "Google Play Only",
            ImageFactoryPreset::Rooted => "Power User (Rooted)",
            ImageFactoryPreset::Clean => "Clean AOSP (Minimal)",
            ImageFactoryPreset::Custom => "Custom Configuration",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ImageFactoryState {
    pub current_tab: ImageHubTab,
    pub selected_version: AndroidVersion,
    pub include_libndk: bool,
    pub include_gapps: bool,
    pub include_magisk: bool,
    pub active_preset: ImageFactoryPreset,
    pub is_dockerfile_expanded: bool,
    pub is_building: bool,
    pub build_logs: Vec<String>,
}

impl Default for ImageFactoryState {
    fn default() -> Self {
        Self {
            current_tab: ImageHubTab::BaseImages,
            selected_version: AndroidVersion::Android13,
            include_libndk: true,
            include_gapps: true,
            include_magisk: false,
            active_preset: ImageFactoryPreset::GamingGapps,
            is_dockerfile_expanded: false,
            is_building: false,
            build_logs: vec!["Ready to generate and build custom ReDroid images.".to_string()],
        }
    }
}

impl ImageFactoryState {
    /// Generates a deterministic Docker image repository tag based on selected version and features.
    pub fn target_image_tag(&self) -> String {
        let mut features = Vec::new();
        if self.include_libndk {
            features.push("ndk");
        }
        if self.include_gapps {
            features.push("gapps");
        }
        if self.include_magisk {
            features.push("magisk");
        }

        let ver = match self.selected_version {
            AndroidVersion::Android11 => "11.0",
            AndroidVersion::Android13 => "13.0",
            AndroidVersion::Android14 => "14.0",
        };

        if features.is_empty() {
            format!("redroid-custom:{ver}-vanilla")
        } else {
            format!("redroid-custom:{ver}-{}", features.join("-"))
        }
    }

    /// User-friendly label for the custom image configuration.
    pub fn display_name(&self) -> String {
        let mut parts = Vec::new();
        if self.include_libndk {
            parts.push("ARM64");
        }
        if self.include_gapps {
            parts.push("Google Play");
        }
        if self.include_magisk {
            parts.push("Magisk Root");
        }

        let ver = self.selected_version.display_label();
        if parts.is_empty() {
            format!("{ver} (Clean Custom)")
        } else {
            format!("{ver} ({})", parts.join(" + "))
        }
    }

    /// Checks if a custom image with this exact configuration already exists in the local Docker cache.
    pub fn is_already_built(&self, available: &HashSet<String>) -> bool {
        let tag = self.target_image_tag();
        available.contains(&tag)
    }

    /// Applies a 1-click preset.
    pub fn apply_preset(&mut self, preset: ImageFactoryPreset) {
        self.active_preset = preset;
        match preset {
            ImageFactoryPreset::GamingGapps => {
                self.include_libndk = true;
                self.include_gapps = true;
                self.include_magisk = false;
            }
            ImageFactoryPreset::GappsOnly => {
                self.include_libndk = false;
                self.include_gapps = true;
                self.include_magisk = false;
            }
            ImageFactoryPreset::Rooted => {
                self.include_libndk = true;
                self.include_gapps = true;
                self.include_magisk = true;
            }
            ImageFactoryPreset::Clean => {
                self.include_libndk = false;
                self.include_gapps = false;
                self.include_magisk = false;
            }
            ImageFactoryPreset::Custom => {}
        }
    }

    pub fn generate_dockerfile(&self) -> String {
        let tag = self.selected_version.image_tag();
        let mut df = format!("FROM {}\n\n", tag);

        if self.include_libndk {
            df.push_str("# Add libndk ARM translation layer (for mobile games and apps)\n");
            df.push_str("ADD https://github.com/remote-android/redroid-modules/releases/download/v1.0.0/libndk.tar.xz /\n");
            df.push_str("RUN tar -xvf /libndk.tar.xz -C / && rm /libndk.tar.xz\n\n");
        }

        if self.include_gapps {
            df.push_str("# Add MindTheGapps Google Play Services and Play Store\n");
            df.push_str("ADD https://github.com/remote-android/redroid-modules/releases/download/v1.0.0/gapps.tar.xz /\n");
            df.push_str("RUN tar -xvf /gapps.tar.xz -C / && rm /gapps.tar.xz\n\n");
        }

        if self.include_magisk {
            df.push_str("# Add Kitsune Magisk Systemless Root & Zygisk\n");
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

    // Sub-tab Navigation
    let mut tab_row = widget::Row::new().spacing(8);

    for tab in ImageHubTab::ALL {
        let is_selected = state.current_tab == tab;
        let icon_name = match tab {
            ImageHubTab::BaseImages => "folder-download-symbolic",
            ImageHubTab::CustomStudio => "applications-engineering-symbolic",
        };

        let btn = if is_selected {
            widget::button::suggested(tab.display_label())
                .leading_icon(widget::icon::from_name(icon_name))
                .on_press(Message::ImageHubTabSelected(tab))
        } else {
            widget::button::standard(tab.display_label())
                .leading_icon(widget::icon::from_name(icon_name))
                .on_press(Message::ImageHubTabSelected(tab))
        };
        tab_row = tab_row.push(btn);
    }

    content = content.push(tab_row);

    // Tab Content Routing
    match state.current_tab {
        ImageHubTab::BaseImages => {
            content = content.push(view_base_images_tab(available_images, active_downloads));
        }
        ImageHubTab::CustomStudio => {
            content = content.push(view_custom_studio_tab(state, available_images));
        }
    }

    widget::scrollable(content).into()
}

/// Renders Tab 1: Official ReDroid Base Images & Local Custom Images
fn view_base_images_tab<'a>(
    available_images: &'a HashSet<String>,
    active_downloads: &'a HashMap<String, ImageDownloadProgress>,
) -> Element<'a, Message> {
    let mut col = widget::Column::new().spacing(16);

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
            .push(
                widget::Column::new()
                    .spacing(2)
                    .width(cosmic::iced::Length::Fill)
                    .push(widget::text::title4(version.display_label()))
                    .push(widget::text::caption(tag)),
            )
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
            .push(widget::text::caption(
                "Official upstream container images. Read-only templates used to run instances or build custom variants.",
            ))
            .push(images_list),
    )
    .padding(16)
    .class(cosmic::theme::Container::Card)
    .width(cosmic::iced::Length::Fill);

    col = col.push(official_section);

    let mut custom_list = widget::Column::new().spacing(12);

    let custom_tags: Vec<&String> = available_images
        .iter()
        .filter(|t| t.starts_with("redroid-custom:") || t.starts_with("cosmic-redroid-custom:"))
        .collect();

    if custom_tags.is_empty() {
        let empty_card = widget::container(
            widget::Row::new()
                .spacing(12)
                .align_y(Alignment::Center)
                .push(widget::icon::from_name("software-update-available-symbolic").size(24))
                .push(
                    widget::Column::new()
                        .spacing(2)
                        .push(widget::text::body("No Custom Images Built Yet"))
                        .push(widget::text::caption(
                            "Switch to the 'Custom Image Studio' tab above to build a customized Android image with Google Play or ARM translation.",
                        )),
                ),
        )
        .padding(16)
        .class(cosmic::theme::Container::Card)
        .width(cosmic::iced::Length::Fill);

        custom_list = custom_list.push(empty_card);
    } else {
        for tag in custom_tags {
            let tag_clone = (*tag).clone();
            let row = widget::Row::new()
                .spacing(12)
                .align_y(Alignment::Center)
                .push(widget::icon::from_name("applications-system-symbolic").size(20))
                .push(
                    widget::Column::new()
                        .spacing(2)
                        .width(cosmic::iced::Length::Fill)
                        .push(widget::text::body(tag.clone()))
                        .push(widget::text::caption("Available for creating new instances")),
                )
                .push(
                    widget::button::destructive("Delete Image")
                        .leading_icon(widget::icon::from_name("user-trash-symbolic"))
                        .on_press(Message::DeleteCustomImage(tag_clone)),
                );

            let card = widget::container(row)
                .padding(12)
                .class(cosmic::theme::Container::Card)
                .width(cosmic::iced::Length::Fill);

            custom_list = custom_list.push(card);
        }
    }

    let custom_section = widget::container(
        widget::Column::new()
            .spacing(12)
            .push(widget::text::title3("Custom Images in Library"))
            .push(widget::text::caption(
                "Locally synthesized images containing Google Play, ARM translation, or root access.",
            ))
            .push(custom_list),
    )
    .padding(16)
    .class(cosmic::theme::Container::Card)
    .width(cosmic::iced::Length::Fill);

    col = col.push(custom_section);
    col.into()
}

/// Renders Tab 2: Custom Image Studio
fn view_custom_studio_tab<'a>(
    state: &'a ImageFactoryState,
    available_images: &'a HashSet<String>,
) -> Element<'a, Message> {
    let mut col = widget::Column::new().spacing(16);

    // Top Explanatory Info Card
    let notice_card = widget::container(
        widget::Row::new()
            .spacing(12)
            .align_y(Alignment::Center)
            .push(widget::icon::from_name("security-high-symbolic").size(24))
            .push(
                widget::Column::new()
                    .spacing(2)
                    .push(widget::text::body("Safe, Independent Image Synthesis"))
                    .push(widget::text::caption(
                        "Building a custom image compiles a separate new image variant. Base images and your existing instances are never modified.",
                    )),
            ),
    )
    .padding(16)
    .class(cosmic::theme::Container::Card)
    .width(cosmic::iced::Length::Fill);

    col = col.push(notice_card);

    // 1-Click Quick Presets
    let mut preset_row = widget::Row::new().spacing(8);

    let presets = [
        (
            ImageFactoryPreset::GamingGapps,
            "Gaming & Google Play",
            "applications-games-symbolic",
        ),
        (
            ImageFactoryPreset::GappsOnly,
            "Google Play Only",
            "software-properties-symbolic",
        ),
        (
            ImageFactoryPreset::Rooted,
            "Power User (Rooted)",
            "security-high-symbolic",
        ),
        (
            ImageFactoryPreset::Clean,
            "Clean AOSP (Minimal)",
            "edit-clear-symbolic",
        ),
    ];

    for (preset, label, icon) in presets {
        let is_selected = state.active_preset == preset;
        let btn = if is_selected {
            widget::button::suggested(label)
                .leading_icon(widget::icon::from_name(icon))
                .on_press(Message::ImageFactoryPresetSelected(preset))
        } else {
            widget::button::standard(label)
                .leading_icon(widget::icon::from_name(icon))
                .on_press(Message::ImageFactoryPresetSelected(preset))
        };
        preset_row = preset_row.push(btn);
    }

    let presets_card = widget::container(
        widget::Column::new()
            .spacing(10)
            .push(widget::text::title3("1-Click Configuration Presets"))
            .push(widget::text::caption(
                "Choose a pre-configured template or customize individual components below.",
            ))
            .push(preset_row),
    )
    .padding(16)
    .class(cosmic::theme::Container::Card)
    .width(cosmic::iced::Length::Fill);

    col = col.push(presets_card);

    // Version & Component Options
    let ver_idx = match state.selected_version {
        AndroidVersion::Android11 => 0,
        AndroidVersion::Android13 => 1,
        AndroidVersion::Android14 => 2,
    };

    let mut options_col = widget::Column::new()
        .spacing(16)
        .push(widget::text::title3("Components & Add-ons"))
        .push(widget::text::caption("Target Base Android Version"))
        .push(widget::dropdown(
            AndroidVersion::NAMES,
            Some(ver_idx),
            Message::ImageFactoryVersionSelected,
        ));

    // Feature Row 1: ARM64 Translation
    let arm_row = widget::Row::new()
        .spacing(12)
        .align_y(Alignment::Center)
        .push(widget::icon::from_name("applications-games-symbolic").size(24))
        .push(
            widget::Column::new()
                .spacing(2)
                .width(cosmic::iced::Length::Fill)
                .push(widget::text::body("ARM64 Mobile App Translation (libndk)"))
                .push(widget::text::caption(
                    "Translates mobile ARM code so games and social apps (TikTok, Genshin, Instagram) run on Intel/AMD PCs.",
                )),
        )
        .push(
            widget::toggler(state.include_libndk)
                .on_toggle(Message::ImageFactoryToggleLibndk),
        );

    let arm_card = widget::container(arm_row)
        .padding(12)
        .class(cosmic::theme::Container::Card)
        .width(cosmic::iced::Length::Fill);

    options_col = options_col.push(arm_card);

    // Feature Row 2: Google Play Store
    let gapps_row = widget::Row::new()
        .spacing(12)
        .align_y(Alignment::Center)
        .push(widget::icon::from_name("software-properties-symbolic").size(24))
        .push(
            widget::Column::new()
                .spacing(2)
                .width(cosmic::iced::Length::Fill)
                .push(widget::text::body("Google Play Store & Services (MindTheGapps)"))
                .push(widget::text::caption(
                    "Adds the Google Play Store, Google account sync, and services required for push notifications and sign-in.",
                )),
        )
        .push(
            widget::toggler(state.include_gapps)
                .on_toggle(Message::ImageFactoryToggleGapps),
        );

    let gapps_card = widget::container(gapps_row)
        .padding(12)
        .class(cosmic::theme::Container::Card)
        .width(cosmic::iced::Length::Fill);

    options_col = options_col.push(gapps_card);

    // Feature Row 3: Kitsune Magisk Root
    let magisk_row = widget::Row::new()
        .spacing(12)
        .align_y(Alignment::Center)
        .push(widget::icon::from_name("security-high-symbolic").size(24))
        .push(
            widget::Column::new()
                .spacing(2)
                .width(cosmic::iced::Length::Fill)
                .push(widget::text::body("Root Access & Zygisk (Kitsune Magisk)"))
                .push(widget::text::caption(
                    "Systemless superuser root access with built-in MagiskHide to prevent detection by banking or work apps.",
                )),
        )
        .push(
            widget::toggler(state.include_magisk)
                .on_toggle(Message::ImageFactoryToggleMagisk),
        );

    let magisk_card = widget::container(magisk_row)
        .padding(12)
        .class(cosmic::theme::Container::Card)
        .width(cosmic::iced::Length::Fill);

    options_col = options_col.push(magisk_card);

    let options_section = widget::container(options_col)
        .padding(16)
        .class(cosmic::theme::Container::Card)
        .width(cosmic::iced::Length::Fill);

    col = col.push(options_section);

    // Output Tag & Duplicate Prevention Action Card
    let target_tag = state.target_image_tag();
    let already_exists = state.is_already_built(available_images);

    let mut action_col = widget::Column::new().spacing(12);

    if already_exists {
        let badge_row = widget::Row::new()
            .spacing(8)
            .align_y(Alignment::Center)
            .push(widget::icon::from_name("object-select-symbolic").size(20))
            .push(widget::text::title4(format!("Already Built & Cached Locally: {target_tag}")));

        let desc = widget::text::caption(
            "This exact configuration already exists in your local Docker library. You can select it directly when creating a new instance.",
        );

        let btn_row = if state.is_building {
            widget::Row::new().push(
                widget::button::standard("Rebuilding Image...")
                    .leading_icon(widget::icon::from_name("system-run-symbolic")),
            )
        } else {
            widget::Row::new()
                .spacing(12)
                .align_y(Alignment::Center)
                .push(
                    widget::button::standard("Rebuild Image (Overwrite)")
                        .leading_icon(widget::icon::from_name("view-refresh-symbolic"))
                        .on_press(Message::ImageFactoryStartBuild),
                )
                .push(widget::text::caption("Recompiles image layers if an update is desired."))
        };

        action_col = action_col.push(badge_row).push(desc).push(btn_row);
    } else {
        let target_row = widget::Row::new()
            .spacing(8)
            .align_y(Alignment::Center)
            .push(widget::icon::from_name("software-update-available-symbolic").size(20))
            .push(widget::text::title4(format!("New Output Image: {target_tag}")));

        let desc = widget::text::caption(
            "Ready to compile. The resulting image will be added to your library without altering any base images or existing instances.",
        );

        let btn_row = if state.is_building {
            widget::Row::new().push(
                widget::button::suggested("Compiling Custom Image...")
                    .leading_icon(widget::icon::from_name("system-run-symbolic")),
            )
        } else {
            widget::Row::new()
                .spacing(12)
                .align_y(Alignment::Center)
                .push(
                    widget::button::suggested("Build Custom Image")
                        .leading_icon(widget::icon::from_name("applications-engineering-symbolic"))
                        .on_press(Message::ImageFactoryStartBuild),
                )
                .push(widget::text::caption("Asynchronously runs docker build in background."))
        };

        action_col = action_col.push(target_row).push(desc).push(btn_row);
    }

    let action_card = widget::container(action_col)
        .padding(16)
        .class(cosmic::theme::Container::Card)
        .width(cosmic::iced::Length::Fill);

    col = col.push(action_card);

    // Collapsible Dockerfile Preview
    let toggle_btn = widget::button::standard(if state.is_dockerfile_expanded {
        "Hide Generated Dockerfile"
    } else {
        "View Generated Dockerfile (Advanced)"
    })
    .leading_icon(widget::icon::from_name("utilities-terminal-symbolic"))
    .on_press(Message::ImageFactoryToggleDockerfilePreview);

    let mut preview_col = widget::Column::new().spacing(8).push(toggle_btn);

    if state.is_dockerfile_expanded {
        let dockerfile_text = state.generate_dockerfile();
        let code_container = widget::container(widget::text::body(dockerfile_text))
            .padding(12)
            .class(cosmic::theme::Container::Card)
            .width(cosmic::iced::Length::Fill);
        preview_col = preview_col.push(code_container);
    }

    let preview_card = widget::container(preview_col)
        .padding(16)
        .class(cosmic::theme::Container::Card)
        .width(cosmic::iced::Length::Fill);

    col = col.push(preview_card);

    // Build Logs Streamer
    if state.is_building || state.build_logs.len() > 1 {
        let mut log_col = widget::Column::new().spacing(4);
        for line in &state.build_logs {
            log_col = log_col.push(widget::text::caption(line));
        }

        let logs_container = widget::container(widget::scrollable(log_col))
            .padding(12)
            .class(cosmic::theme::Container::Card)
            .width(cosmic::iced::Length::Fill)
            .height(180);

        let logs_card = widget::container(
            widget::Column::new()
                .spacing(8)
                .push(widget::text::title4("Build Execution Logs"))
                .push(logs_container),
        )
        .padding(16)
        .class(cosmic::theme::Container::Card)
        .width(cosmic::iced::Length::Fill);

        col = col.push(logs_card);
    }

    col.into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_target_image_tag_deterministic_and_clean() {
        let mut state = ImageFactoryState::default();
        state.selected_version = AndroidVersion::Android13;
        state.include_libndk = true;
        state.include_gapps = true;
        state.include_magisk = false;
        assert_eq!(state.target_image_tag(), "redroid-custom:13.0-ndk-gapps");

        state.include_magisk = true;
        assert_eq!(
            state.target_image_tag(),
            "redroid-custom:13.0-ndk-gapps-magisk"
        );

        state.include_libndk = false;
        state.include_gapps = false;
        state.include_magisk = false;
        assert_eq!(state.target_image_tag(), "redroid-custom:13.0-vanilla");
    }

    #[test]
    fn test_duplicate_detection_against_available_set() {
        let mut state = ImageFactoryState::default();
        state.selected_version = AndroidVersion::Android13;
        state.include_libndk = true;
        state.include_gapps = true;
        state.include_magisk = false;

        let mut available = HashSet::new();
        assert!(!state.is_already_built(&available));

        available.insert("redroid-custom:13.0-ndk-gapps".to_string());
        assert!(state.is_already_built(&available));
    }

    #[test]
    fn test_preset_application() {
        let mut state = ImageFactoryState::default();

        state.apply_preset(ImageFactoryPreset::GamingGapps);
        assert!(state.include_libndk);
        assert!(state.include_gapps);
        assert!(!state.include_magisk);

        state.apply_preset(ImageFactoryPreset::Rooted);
        assert!(state.include_libndk);
        assert!(state.include_gapps);
        assert!(state.include_magisk);

        state.apply_preset(ImageFactoryPreset::Clean);
        assert!(!state.include_libndk);
        assert!(!state.include_gapps);
        assert!(!state.include_magisk);
    }
}
