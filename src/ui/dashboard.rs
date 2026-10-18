// src/ui/dashboard.rs
// Instance cards dashboard for CosmicReDroid.

use crate::app::Message;
use crate::models::{AndroidInstance, ContainerMetrics, ImageDownloadProgress, InstanceStatus};
use cosmic::iced::Alignment;
use cosmic::widget;
use cosmic::Element;
use std::collections::HashMap;
use uuid::Uuid;

pub fn view<'a>(
    instances: &'a [AndroidInstance],
    metrics: &'a HashMap<Uuid, ContainerMetrics>,
    active_downloads: &'a HashMap<String, ImageDownloadProgress>,
) -> Element<'a, Message> {
    if instances.is_empty() {
        return view_empty_state();
    }

    let mut content = widget::Column::new()
        .spacing(16)
        .padding(16)
        .width(cosmic::iced::Length::Fill);

    let header_row = widget::Row::new()
        .spacing(12)
        .align_y(Alignment::Center)
        .push(widget::text::title2("Managed Instances"))
        .push(
            widget::container(widget::text::caption(format!("{} Total", instances.len())))
                .padding([2, 8])
                .class(cosmic::theme::Container::Card),
        );

    content = content.push(header_row);

    for instance in instances {
        let card = view_instance_card(
            instance,
            metrics.get(&instance.id),
            active_downloads.get(&instance.image_tag),
        );
        content = content.push(card);
    }

    widget::scrollable(content).into()
}

fn view_empty_state<'a>() -> Element<'a, Message> {
    let empty_box = widget::container(
        widget::Column::new()
            .spacing(16)
            .align_x(Alignment::Center)
            .push(widget::icon::from_name("view-grid-symbolic").size(48))
            .push(widget::text::title3("No Android Instances Configured"))
            .push(widget::text::body(
                "Create your first ReDroid container instance to get started.",
            ))
            .push(
                widget::button::suggested("New Instance")
                    .leading_icon(widget::icon::from_name("list-add-symbolic"))
                    .on_press(Message::NewInstanceClicked),
            ),
    )
    .padding(32)
    .class(cosmic::theme::Container::Card)
    .align_x(Alignment::Center)
    .align_y(Alignment::Center)
    .width(cosmic::iced::Length::Fill)
    .height(cosmic::iced::Length::Fill);

    empty_box.into()
}

fn view_instance_card<'a>(
    instance: &'a AndroidInstance,
    metrics: Option<&'a ContainerMetrics>,
    download: Option<&'a ImageDownloadProgress>,
) -> Element<'a, Message> {
    let mut card_content = widget::Column::new().spacing(12).width(cosmic::iced::Length::Fill);

    // Row 1: Instance Header
    let status_badge = widget::container(
        widget::Row::new()
            .spacing(6)
            .align_y(Alignment::Center)
            .push(widget::icon::from_name(instance.status.status_icon_name()).size(14))
            .push(widget::text::body(instance.status.display_label())),
    )
    .padding([2, 8])
    .class(cosmic::theme::Container::Card);

    let top_row = widget::Row::new()
        .spacing(12)
        .align_y(Alignment::Center)
        .push(widget::text::title3(&instance.name))
        .push(
            widget::container(widget::text::caption(instance.preset.display_name()))
                .padding([2, 6])
                .class(cosmic::theme::Container::Card),
        )
        .push(
            widget::container(widget::text::caption(instance.android_version.short_name()))
                .padding([2, 6])
                .class(cosmic::theme::Container::Card),
        )
        .push(
            widget::container(widget::text::caption(format!(
                "127.0.0.1:{}",
                instance.adb_port
            )))
            .padding([2, 6])
            .class(cosmic::theme::Container::Card),
        )
        .push(status_badge);

    card_content = card_content.push(top_row);

    // Row 2: Hardware Summary
    let (b1, b2, b3) = instance.binder_indices();
    let summary_row = widget::Row::new()
        .spacing(16)
        .align_y(Alignment::Center)
        .push(widget::text::caption(format!("Model: {}", instance.identity.model)))
        .push(widget::text::caption(format!("IMEI: {}", instance.identity.imei)))
        .push(widget::text::caption(format!("Android ID: {}", instance.identity.android_id)))
        .push(widget::text::caption(format!(
            "Display: {}x{} @ {} FPS",
            instance.identity.width, instance.identity.height, instance.identity.fps
        )))
        .push(widget::text::caption(format!("Binder: /dev/binder{},{},{}", b1, b2, b3)))
        .push(widget::text::caption(format!("Proxy: {}", instance.proxy.summary_display())));

    card_content = card_content.push(summary_row);

    // Row 3: Active Image Download Progress Bar
    if let Some(prog) = download {
        let prog_header = widget::Row::new()
            .spacing(8)
            .align_y(Alignment::Center)
            .push(widget::icon::from_name("folder-download-symbolic").size(14))
            .push(widget::text::body(format!(
                "Downloading Base Image ({}) - {}",
                instance.image_tag,
                prog.formatted_progress()
            )));

        let download_box = widget::container(
            widget::Column::new()
                .spacing(6)
                .push(prog_header)
                .push(widget::progress_bar::determinate_linear(prog.progress_fraction)),
        )
        .padding(10)
        .class(cosmic::theme::Container::Card)
        .width(cosmic::iced::Length::Fill);

        card_content = card_content.push(download_box);
    } else if let InstanceStatus::DownloadingImage(ref desc) = instance.status {
        let download_box = widget::container(
            widget::Row::new()
                .spacing(8)
                .align_y(Alignment::Center)
                .push(widget::icon::from_name("folder-download-symbolic").size(14))
                .push(widget::text::body(desc)),
        )
        .padding(10)
        .class(cosmic::theme::Container::Card)
        .width(cosmic::iced::Length::Fill);

        card_content = card_content.push(download_box);
    }

    // Row 4: Error Banner (if in error state)
    if let InstanceStatus::Error(ref err) = instance.status {
        let err_box = widget::container(
            widget::Row::new()
                .spacing(8)
                .align_y(Alignment::Center)
                .push(widget::icon::from_name("dialog-error-symbolic").size(16))
                .push(widget::text::caption(format!("Error: {err}"))),
        )
        .padding([8, 12])
        .class(cosmic::theme::Container::Card)
        .width(cosmic::iced::Length::Fill);

        card_content = card_content.push(err_box);
    }

    // Row 5: Live Telemetry (if running)
    if instance.status.is_running() {
        if let Some(m) = metrics {
            let cpu_frac = (m.cpu_percent / 100.0).clamp(0.0, 1.0);
            let mem_frac = if m.memory_limit_mb > 0 {
                (m.memory_used_mb as f32 / m.memory_limit_mb as f32).clamp(0.0, 1.0)
            } else {
                0.0
            };

            let cpu_col = widget::Column::new()
                .spacing(4)
                .width(cosmic::iced::Length::FillPortion(1))
                .push(widget::text::caption(format!("CPU: {:.1}%", m.cpu_percent)))
                .push(widget::progress_bar::determinate_linear(cpu_frac));

            let ram_col = widget::Column::new()
                .spacing(4)
                .width(cosmic::iced::Length::FillPortion(1))
                .push(widget::text::caption(format!(
                    "RAM: {} / {} MB ({:.1}%)",
                    m.memory_used_mb,
                    m.memory_limit_mb,
                    mem_frac * 100.0
                )))
                .push(widget::progress_bar::determinate_linear(mem_frac));

            let telemetry_row = widget::Row::new().spacing(24).push(cpu_col).push(ram_col);

            card_content = card_content.push(telemetry_row);
        }
    }

    // Row 6: Action Toolbar
    let mut actions_row = widget::Row::new().spacing(8).align_y(Alignment::Center);

    if matches!(instance.status, InstanceStatus::DownloadingImage(_)) || download.is_some() {
        actions_row = actions_row
            .push(
                widget::button::standard("Downloading...")
                    .leading_icon(widget::icon::from_name("folder-download-symbolic")),
            )
            .push(
                widget::button::destructive("Cancel")
                    .leading_icon(widget::icon::from_name("process-stop-symbolic"))
                    .on_press(Message::CancelImageDownload(instance.image_tag.clone())),
            );
    } else if instance.status.can_stop() {
        actions_row = actions_row.push(
            widget::button::destructive("Stop")
                .leading_icon(widget::icon::from_name("media-playback-stop-symbolic"))
                .on_press(Message::StopInstance(instance.id)),
        );
    } else if let InstanceStatus::Error(_) = instance.status {
        actions_row = actions_row.push(
            widget::button::suggested("Retry Start")
                .leading_icon(widget::icon::from_name("view-refresh-symbolic"))
                .on_press(Message::StartInstance(instance.id)),
        );
    } else if instance.status.can_start() {
        actions_row = actions_row.push(
            widget::button::suggested("Start")
                .leading_icon(widget::icon::from_name("media-playback-start-symbolic"))
                .on_press(Message::StartInstance(instance.id)),
        );
    } else {
        actions_row = actions_row.push(
            widget::button::standard("Processing...")
                .leading_icon(widget::icon::from_name("system-run-symbolic")),
        );
    }

    if instance.status.is_running() {
        actions_row = actions_row.push(
            widget::button::standard("Open Screen")
                .leading_icon(widget::icon::from_name("video-display-symbolic"))
                .on_press(Message::OpenScreen(instance.id)),
        );
    } else {
        actions_row = actions_row.push(
            widget::button::standard("Open Screen")
                .leading_icon(widget::icon::from_name("video-display-symbolic")),
        );
    }

    actions_row = actions_row
        .push(
            widget::button::standard("Clone")
                .leading_icon(widget::icon::from_name("edit-copy-symbolic"))
                .on_press(Message::CloneInstance(instance.id)),
        )
        .push(
            widget::button::standard("Snapshot")
                .leading_icon(widget::icon::from_name("camera-photo-symbolic"))
                .on_press(Message::SnapshotInstance(instance.id)),
        )
        .push(
            widget::button::standard("Configure")
                .leading_icon(widget::icon::from_name("preferences-system-symbolic"))
                .on_press(Message::ConfigureInstance(instance.id)),
        )
        .push(
            widget::button::destructive("Delete")
                .leading_icon(widget::icon::from_name("user-trash-symbolic"))
                .on_press(Message::DeleteInstance(instance.id)),
        );

    card_content = card_content.push(actions_row);

    widget::container(card_content)
        .padding(16)
        .class(cosmic::theme::Container::Card)
        .width(cosmic::iced::Length::Fill)
        .into()
}
