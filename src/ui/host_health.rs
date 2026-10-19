//! Host diagnostics and snapshots views for CosmicReDroid.

use crate::app::Message;
use crate::models::{HostHealthReport, SnapshotInfo};
use cosmic::iced::Alignment;
use cosmic::widget;
use cosmic::Element;

pub fn view_snapshots<'a>(
    snapshots: &'a [SnapshotInfo],
    confirm_restore_id: Option<uuid::Uuid>,
    confirm_delete_id: Option<uuid::Uuid>,
) -> Element<'a, Message> {
    let mut content = widget::Column::new()
        .spacing(16)
        .padding(16)
        .width(cosmic::iced::Length::Fill);

    let header = widget::Column::new()
        .spacing(4)
        .push(widget::text::title2("Storage & Snapshots"))
        .push(widget::text::body(
            "Point-in-time Copy-on-Write compressed archives of Android instance data volumes.",
        ));
    content = content.push(header);

    if snapshots.is_empty() {
        let empty_card = widget::container(
            widget::Column::new()
                .spacing(12)
                .align_x(Alignment::Center)
                .push(widget::icon::from_name("camera-photo-symbolic").size(48))
                .push(widget::text::title3("No Snapshots Created"))
                .push(widget::text::body(
                    "Click 'Snapshot' on an instance card in the dashboard to create compressed .tar.zst archives.",
                )),
        )
        .padding(32)
        .class(cosmic::theme::Container::Card)
        .align_x(Alignment::Center)
        .width(cosmic::iced::Length::Fill);

        content = content.push(empty_card);
    } else {
        for snap in snapshots {
            let mut actions_row = widget::Row::new().spacing(8).align_y(Alignment::Center);

            if confirm_restore_id == Some(snap.id) {
                actions_row = actions_row
                    .push(
                        widget::button::destructive("Confirm Restore")
                            .leading_icon(widget::icon::from_name("document-revert-symbolic"))
                            .on_press(Message::RestoreSnapshot(snap.id)),
                    )
                    .push(
                        widget::button::standard("Cancel")
                            .on_press(Message::CancelRestoreSnapshot),
                    );
            } else {
                actions_row = actions_row.push(
                    widget::button::suggested("Restore")
                        .leading_icon(widget::icon::from_name("document-revert-symbolic"))
                        .on_press(Message::ConfirmRestoreSnapshot(snap.id)),
                );
            }

            if confirm_delete_id == Some(snap.id) {
                actions_row = actions_row
                    .push(
                        widget::button::destructive("Confirm Delete")
                            .leading_icon(widget::icon::from_name("user-trash-symbolic"))
                            .on_press(Message::DeleteSnapshot(snap.id)),
                    )
                    .push(
                        widget::button::standard("Cancel")
                            .on_press(Message::CancelDeleteSnapshot),
                    );
            } else {
                actions_row = actions_row.push(
                    widget::button::destructive("Delete")
                        .leading_icon(widget::icon::from_name("user-trash-symbolic"))
                        .on_press(Message::ConfirmDeleteSnapshot(snap.id)),
                );
            }

            let snap_card = widget::container(
                widget::Row::new()
                    .spacing(16)
                    .align_y(Alignment::Center)
                    .push(widget::icon::from_name("camera-photo-symbolic").size(24))
                    .push(
                        widget::Column::new()
                            .spacing(4)
                            .width(cosmic::iced::Length::FillPortion(2))
                            .push(widget::text::title4(&snap.name))
                            .push(widget::text::caption(format!(
                                "Path: {}",
                                snap.file_path().display()
                            ))),
                    )
                    .push(
                        widget::Column::new()
                            .spacing(4)
                            .width(cosmic::iced::Length::FillPortion(1))
                            .push(widget::text::body(snap.size_display()))
                            .push(widget::text::caption(snap.date_display())),
                    )
                    .push(actions_row),
            )
            .padding(16)
            .class(cosmic::theme::Container::Card)
            .width(cosmic::iced::Length::Fill);

            content = content.push(snap_card);
        }
    }

    widget::scrollable(content).into()
}

pub fn view_host_health<'a>(
    report: &'a HostHealthReport,
    is_refreshing: bool,
    is_remediating: bool,
) -> Element<'a, Message> {
    let mut content = widget::Column::new()
        .spacing(16)
        .padding(16)
        .width(cosmic::iced::Length::Fill);

    let header_row = widget::Row::new()
        .spacing(16)
        .align_y(Alignment::Center)
        .push(
            widget::Column::new()
                .spacing(4)
                .push(widget::text::title2("Host System Diagnostics"))
                .push(widget::text::body(
                    "Validates host kernel modules, container engine, GPU render node, and execution tools.",
                )),
        )
        .push(
            if is_refreshing {
                widget::button::standard("Checking...")
                    .leading_icon(widget::icon::from_name("system-run-symbolic"))
            } else {
                widget::button::standard("Refresh Diagnostics")
                    .leading_icon(widget::icon::from_name("view-refresh-symbolic"))
                    .on_press(Message::RefreshHealthClicked)
            },
        );

    content = content.push(header_row);

    // Remediation Banner
    if report.needs_remediation() {
        let mut rem_card_col = widget::Column::new().spacing(8);

        rem_card_col = rem_card_col
            .push(
                widget::Row::new()
                    .spacing(8)
                    .align_y(Alignment::Center)
                    .push(widget::icon::from_name("dialog-warning-symbolic").size(20))
                    .push(widget::text::title4("Host Configuration Action Required")),
            )
            .push(widget::text::body(
                "One or more host requirements are not met (binder nodes, permissions, or container access). Click below to execute automated host configuration via pkexec.",
            ));

        let rem_btn = if is_remediating {
            widget::button::suggested("Executing Remediation...")
                .leading_icon(widget::icon::from_name("system-run-symbolic"))
        } else {
            widget::button::suggested("Initialize Binder Nodes and Fix Permissions (pkexec)")
                .leading_icon(widget::icon::from_name("dialog-password-symbolic"))
                .on_press(Message::RemediationClicked)
        };

        rem_card_col = rem_card_col.push(rem_btn);

        let rem_card = widget::container(rem_card_col)
            .padding(16)
            .class(cosmic::theme::Container::Card)
            .width(cosmic::iced::Length::Fill);

        content = content.push(rem_card);
    } else if report.all_ok() {
        let ok_card = widget::container(
            widget::Row::new()
                .spacing(12)
                .align_y(Alignment::Center)
                .push(widget::icon::from_name("emblem-ok-symbolic").size(24))
                .push(widget::text::body(
                    "All host prerequisites, tools, and kernel modules are operating normally.",
                )),
        )
        .padding(16)
        .class(cosmic::theme::Container::Card)
        .width(cosmic::iced::Length::Fill);

        content = content.push(ok_card);
    } else {
        let warn_card = widget::container(
            widget::Row::new()
                .spacing(12)
                .align_y(Alignment::Center)
                .push(widget::icon::from_name("dialog-warning-symbolic").size(24))
                .push(widget::text::body(
                    "Kernel modules and Docker are available, but user tools or storage features (adb, scrcpy, or CoW reflink) require attention.",
                )),
        )
        .padding(16)
        .class(cosmic::theme::Container::Card)
        .width(cosmic::iced::Length::Fill);

        content = content.push(warn_card);
    }

    // Diagnostic Checklist Cards
    let mut check_col = widget::Column::new().spacing(8);

    check_col = check_col
        .push(view_probe_item(
            "Docker Daemon Connectivity",
            "Ensures local Docker engine socket is accessible.",
            report.docker_available,
        ))
        .push(view_probe_item(
            "Android Binderfs Nodes",
            "Validates /dev/binder, /dev/hwbinder, /dev/vndbinder allocation.",
            report.binder_available,
        ))
        .push(view_probe_item(
            "DRI GPU Render Node (/dev/dri/renderD128)",
            "Validates hardware graphics acceleration render node.",
            report.gpu_available,
        ))
        .push(view_probe_item(
            "Android Debug Bridge (adb)",
            "Command-line bridge for guest Android interaction and telemetry.",
            report.adb_available,
        ))
        .push(view_probe_item(
            "Screen Mirroring Utility (scrcpy)",
            "Hardware-accelerated low-latency Wayland screen attachment.",
            report.scrcpy_available,
        ))
        .push(view_probe_item(
            "CoW Reflink Storage Support",
            "Validates filesystem support for instant zero-copy data cloning.",
            report.reflink_supported,
        ));

    let checks_card = widget::container(check_col)
        .padding(16)
        .class(cosmic::theme::Container::Card)
        .width(cosmic::iced::Length::Fill);

    content = content.push(checks_card);

    // Diagnostic Log Details
    if !report.details.is_empty() {
        let mut details_col = widget::Column::new().spacing(4);
        details_col = details_col.push(widget::text::title4("Diagnostic Probe Logs"));

        for detail in &report.details {
            details_col = details_col.push(widget::text::caption(detail));
        }

        let details_card = widget::container(details_col)
            .padding(16)
            .class(cosmic::theme::Container::Card)
            .width(cosmic::iced::Length::Fill);

        content = content.push(details_card);
    }

    widget::scrollable(content).into()
}

fn view_probe_item<'a>(
    title: &'static str,
    desc: &'static str,
    passed: bool,
) -> Element<'a, Message> {
    let icon_name = if passed {
        "emblem-ok-symbolic"
    } else {
        "dialog-warning-symbolic"
    };

    let status_label = if passed { "Passed" } else { "Missing / Issue" };

    widget::Row::new()
        .spacing(12)
        .align_y(Alignment::Center)
        .push(widget::icon::from_name(icon_name).size(18))
        .push(
            widget::Column::new()
                .spacing(2)
                .width(cosmic::iced::Length::Fill)
                .push(widget::text::body(title))
                .push(widget::text::caption(desc)),
        )
        .push(widget::text::caption(status_label))
        .into()
}
