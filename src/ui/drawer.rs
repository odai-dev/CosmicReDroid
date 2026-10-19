//! Right-side context drawer for instance configuration and hardware spoofing.

use crate::app::Message;
use crate::models::{AndroidInstance, AndroidVersion, DevicePreset};
use cosmic::app::context_drawer::{context_drawer, ContextDrawer};
use cosmic::iced::Alignment;
use cosmic::widget;
use std::collections::HashSet;

#[derive(Debug, Clone, Default)]
pub struct DrawerState {
    pub width: String,
    pub height: String,
    pub dpi: String,
    pub fps: String,
    pub proxy_port: String,
}

impl DrawerState {
    pub fn from_instance(inst: &AndroidInstance) -> Self {
        Self {
            width: inst.identity.width.to_string(),
            height: inst.identity.height.to_string(),
            dpi: inst.identity.dpi.to_string(),
            fps: inst.identity.fps.to_string(),
            proxy_port: inst.proxy.port.to_string(),
        }
    }

    pub fn sync_from_instance(&mut self, inst: &AndroidInstance) {
        self.width = inst.identity.width.to_string();
        self.height = inst.identity.height.to_string();
        self.dpi = inst.identity.dpi.to_string();
        self.fps = inst.identity.fps.to_string();
        self.proxy_port = inst.proxy.port.to_string();
    }

    pub fn validate_name(name: &str) -> Option<&'static str> {
        if name.trim().is_empty() {
            Some("Instance name cannot be empty")
        } else {
            None
        }
    }

    pub fn validate_imei(imei: &str) -> Option<&'static str> {
        if imei.len() != 15 || !imei.chars().all(|c| c.is_ascii_digit()) {
            Some("IMEI must be exactly 15 numeric digits")
        } else if !crate::backend::spoofing::verify_luhn(imei) {
            Some("Invalid IMEI: failed Luhn checksum")
        } else {
            None
        }
    }

    pub fn validate_android_id(id: &str) -> Option<&'static str> {
        if id.len() != 16 || !id.chars().all(|c| c.is_ascii_hexdigit()) {
            Some("Android ID must be 16 hexadecimal characters")
        } else {
            None
        }
    }

    pub fn validate_mac(mac: &str) -> Option<&'static str> {
        let parts: Vec<&str> = mac.split(':').collect();
        if parts.len() != 6 || !parts.iter().all(|p| p.len() == 2 && p.chars().all(|c| c.is_ascii_hexdigit())) {
            Some("MAC address must be in format XX:XX:XX:XX:XX:XX")
        } else {
            None
        }
    }

    pub fn validate_width(&self) -> Option<&'static str> {
        match self.width.parse::<u32>() {
            Ok(w) if (320..=7680).contains(&w) => None,
            _ => Some("Width must be between 320 and 7680 px"),
        }
    }

    pub fn validate_height(&self) -> Option<&'static str> {
        match self.height.parse::<u32>() {
            Ok(h) if (320..=7680).contains(&h) => None,
            _ => Some("Height must be between 320 and 7680 px"),
        }
    }

    pub fn validate_dpi(&self) -> Option<&'static str> {
        match self.dpi.parse::<u32>() {
            Ok(d) if (72..=1000).contains(&d) => None,
            _ => Some("DPI must be between 72 and 1000"),
        }
    }

    pub fn validate_fps(&self) -> Option<&'static str> {
        match self.fps.parse::<u32>() {
            Ok(f) if (10..=240).contains(&f) => None,
            _ => Some("FPS must be between 10 and 240"),
        }
    }

    pub fn validate_proxy_host(enabled: bool, host: &str) -> Option<&'static str> {
        if enabled && host.trim().is_empty() {
            Some("Proxy host cannot be empty when proxy is enabled")
        } else {
            None
        }
    }

    pub fn validate_proxy_port(&self, enabled: bool) -> Option<&'static str> {
        if !enabled {
            return None;
        }
        match self.proxy_port.parse::<u16>() {
            Ok(p) if p > 0 => None,
            _ => Some("Proxy port must be between 1 and 65535"),
        }
    }

    pub fn is_valid(&self, inst: &AndroidInstance) -> bool {
        Self::validate_name(&inst.name).is_none()
            && Self::validate_imei(&inst.identity.imei).is_none()
            && Self::validate_android_id(&inst.identity.android_id).is_none()
            && Self::validate_mac(&inst.identity.mac_address).is_none()
            && self.validate_width().is_none()
            && self.validate_height().is_none()
            && self.validate_dpi().is_none()
            && self.validate_fps().is_none()
            && Self::validate_proxy_host(inst.proxy.enabled, &inst.proxy.host).is_none()
            && self.validate_proxy_port(inst.proxy.enabled).is_none()
    }
}

pub fn view<'a>(
    drawer_instance: &'a AndroidInstance,
    drawer_state: &'a DrawerState,
    available_images: &'a HashSet<String>,
    is_new: bool,
) -> ContextDrawer<'a, Message> {
    let title = if is_new {
        "New Android Instance"
    } else {
        "Configure Instance"
    };

    let mut body = widget::Column::new()
        .spacing(16)
        .padding(16)
        .width(cosmic::iced::Length::Fill);

    let mut image_options: Vec<String> = AndroidVersion::ALL
        .iter()
        .map(|v| v.display_label().to_string())
        .collect();

    let mut custom_tags: Vec<String> = available_images
        .iter()
        .filter(|t| t.starts_with("cosmic-redroid-custom:") || !AndroidVersion::ALL.iter().any(|v| v.image_tag() == **t))
        .cloned()
        .collect();
    custom_tags.sort();

    for ct in &custom_tags {
        image_options.push(format!("Custom: {ct}"));
    }

    let current_img_idx = if let Some(pos) = AndroidVersion::ALL.iter().position(|v| v.image_tag() == drawer_instance.image_tag) {
        Some(pos)
    } else if let Some(pos) = custom_tags.iter().position(|t| *t == drawer_instance.image_tag) {
        Some(AndroidVersion::ALL.len() + pos)
    } else {
        Some(0)
    };

    let image_section = widget::Column::new()
        .spacing(8)
        .push(widget::text::title4("Android Version & Image"))
        .push(widget::dropdown(
            image_options,
            current_img_idx,
            Message::DrawerVersionSelected,
        ));

    body = body.push(image_section);

    let preset_idx = match drawer_instance.preset {
        DevicePreset::GalaxyS24Ultra => 0,
        DevicePreset::Pixel8Pro => 1,
        DevicePreset::AsusRogPhone8 => 2,
        DevicePreset::Custom => 3,
    };

    let preset_section = widget::Column::new()
        .spacing(8)
        .push(widget::text::title4("Device Hardware Preset"))
        .push(widget::dropdown(
            DevicePreset::NAMES,
            Some(preset_idx),
            Message::DrawerPresetSelected,
        ));

    body = body.push(preset_section);

    let id_header = widget::Row::new()
        .spacing(12)
        .align_y(Alignment::Center)
        .push(widget::text::title4("Hardware Identity"))
        .push(
            widget::button::standard("Randomize All")
                .leading_icon(widget::icon::from_name("media-playlist-shuffle-symbolic"))
                .on_press(Message::DrawerRandomizeIdentity),
        );

    let mut identity_section = widget::Column::new().spacing(8).push(id_header);

    // Instance Display Name
    identity_section = identity_section.push(widget::text::caption("Instance Display Name"));
    identity_section = identity_section.push(
        widget::text_input("Name", &drawer_instance.name)
            .on_input(Message::DrawerNameChanged),
    );
    if let Some(err) = DrawerState::validate_name(&drawer_instance.name) {
        identity_section = identity_section.push(widget::text::caption(err));
    }

    identity_section = identity_section
        .push(widget::text::caption("Manufacturer"))
        .push(
            widget::text_input("Manufacturer", &drawer_instance.identity.manufacturer)
                .on_input(Message::DrawerManufacturerChanged),
        )
        .push(widget::text::caption("Brand"))
        .push(
            widget::text_input("Brand", &drawer_instance.identity.brand)
                .on_input(Message::DrawerBrandChanged),
        )
        .push(widget::text::caption("Model"))
        .push(
            widget::text_input("Model", &drawer_instance.identity.model)
                .on_input(Message::DrawerModelChanged),
        )
        .push(widget::text::caption("Device Codename"))
        .push(
            widget::text_input("Device", &drawer_instance.identity.device)
                .on_input(Message::DrawerDeviceChanged),
        )
        .push(widget::text::caption("Board"))
        .push(
            widget::text_input("Board", &drawer_instance.identity.board)
                .on_input(Message::DrawerBoardChanged),
        )
        .push(widget::text::caption("Build Fingerprint"))
        .push(
            widget::text_input("Fingerprint", &drawer_instance.identity.fingerprint)
                .on_input(Message::DrawerFingerprintChanged),
        );

    // IMEI with validation
    identity_section = identity_section.push(widget::text::caption("IMEI (Luhn 15-digit)"));
    identity_section = identity_section.push(
        widget::text_input("IMEI", &drawer_instance.identity.imei)
            .on_input(Message::DrawerImeiChanged),
    );
    if let Some(err) = DrawerState::validate_imei(&drawer_instance.identity.imei) {
        identity_section = identity_section.push(widget::text::caption(err));
    }

    // Android ID with validation
    identity_section = identity_section.push(widget::text::caption("Android ID (16-char hex)"));
    identity_section = identity_section.push(
        widget::text_input("Android ID", &drawer_instance.identity.android_id)
            .on_input(Message::DrawerAndroidIdChanged),
    );
    if let Some(err) = DrawerState::validate_android_id(&drawer_instance.identity.android_id) {
        identity_section = identity_section.push(widget::text::caption(err));
    }

    identity_section = identity_section
        .push(widget::text::caption("Serial Number"))
        .push(
            widget::text_input("Serial", &drawer_instance.identity.serial_no)
                .on_input(Message::DrawerSerialChanged),
        );

    // MAC Address with validation
    identity_section = identity_section.push(widget::text::caption("MAC Address"));
    identity_section = identity_section.push(
        widget::text_input("MAC Address", &drawer_instance.identity.mac_address)
            .on_input(Message::DrawerMacChanged),
    );
    if let Some(err) = DrawerState::validate_mac(&drawer_instance.identity.mac_address) {
        identity_section = identity_section.push(widget::text::caption(err));
    }

    body = body.push(identity_section);

    let mut w_col = widget::Column::new()
        .spacing(4)
        .push(widget::text::caption("Width (px)"))
        .push(
            widget::text_input("Width", &drawer_state.width)
                .on_input(Message::DrawerWidthChanged),
        );
    if let Some(err) = drawer_state.validate_width() {
        w_col = w_col.push(widget::text::caption(err));
    }

    let mut h_col = widget::Column::new()
        .spacing(4)
        .push(widget::text::caption("Height (px)"))
        .push(
            widget::text_input("Height", &drawer_state.height)
                .on_input(Message::DrawerHeightChanged),
        );
    if let Some(err) = drawer_state.validate_height() {
        h_col = h_col.push(widget::text::caption(err));
    }

    let display_row1 = widget::Row::new()
        .spacing(12)
        .push(w_col)
        .push(h_col);

    let mut dpi_col = widget::Column::new()
        .spacing(4)
        .push(widget::text::caption("DPI"))
        .push(
            widget::text_input("DPI", &drawer_state.dpi)
                .on_input(Message::DrawerDpiChanged),
        );
    if let Some(err) = drawer_state.validate_dpi() {
        dpi_col = dpi_col.push(widget::text::caption(err));
    }

    let mut fps_col = widget::Column::new()
        .spacing(4)
        .push(widget::text::caption("Target FPS"))
        .push(
            widget::text_input("FPS", &drawer_state.fps)
                .on_input(Message::DrawerFpsChanged),
        );
    if let Some(err) = drawer_state.validate_fps() {
        fps_col = fps_col.push(widget::text::caption(err));
    }

    let display_row2 = widget::Row::new()
        .spacing(12)
        .push(dpi_col)
        .push(fps_col);

    let display_section = widget::Column::new()
        .spacing(8)
        .push(widget::text::title4("Display & Performance"))
        .push(display_row1)
        .push(display_row2);

    body = body.push(display_section);

    let mut proxy_section = widget::Column::new()
        .spacing(8)
        .push(widget::text::title4("Per-Instance Proxy Isolation"))
        .push(
            widget::toggler(drawer_instance.proxy.enabled)
                .label("Enable HTTP Proxy".to_string())
                .on_toggle(Message::DrawerProxyToggled),
        );

    if drawer_instance.proxy.enabled {
        let mut proxy_inputs = widget::Column::new()
            .spacing(8)
            .push(widget::text::caption("Protocol"))
            .push(widget::dropdown(
                &["HTTP (Android System Proxy)"],
                Some(0),
                Message::DrawerProxyProtocolSelected,
            ))
            .push(widget::text::caption("Proxy Host"))
            .push(
                widget::text_input("127.0.0.1 or hostname", &drawer_instance.proxy.host)
                    .on_input(Message::DrawerProxyHostChanged),
            );

        if let Some(err) = DrawerState::validate_proxy_host(drawer_instance.proxy.enabled, &drawer_instance.proxy.host) {
            proxy_inputs = proxy_inputs.push(widget::text::caption(err));
        }

        proxy_inputs = proxy_inputs
            .push(widget::text::caption("Proxy Port"))
            .push(
                widget::text_input("8080", &drawer_state.proxy_port)
                    .on_input(Message::DrawerProxyPortChanged),
            );

        if let Some(err) = drawer_state.validate_proxy_port(drawer_instance.proxy.enabled) {
            proxy_inputs = proxy_inputs.push(widget::text::caption(err));
        }

        proxy_section = proxy_section.push(proxy_inputs);
    }

    body = body.push(proxy_section);

    // Footer actions
    let save_label = if is_new {
        "Create Instance"
    } else if drawer_instance.status.is_running() {
        "Save (Restart Required)"
    } else {
        "Save"
    };

    let is_form_valid = drawer_state.is_valid(drawer_instance);

    let save_btn = if is_form_valid {
        widget::button::suggested(save_label)
            .leading_icon(widget::icon::from_name("document-save-symbolic"))
            .on_press(Message::SaveDrawerInstance)
    } else {
        widget::button::standard(save_label)
            .leading_icon(widget::icon::from_name("document-save-symbolic"))
    };

    let footer_row = widget::Row::new()
        .spacing(12)
        .align_y(Alignment::Center)
        .push(save_btn)
        .push(widget::button::standard("Cancel").on_press(Message::CloseDrawer));

    context_drawer(widget::scrollable(body), Message::CloseDrawer)
        .title(title)
        .footer(footer_row)
}
