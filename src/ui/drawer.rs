// src/ui/drawer.rs
// Right-side Context Drawer for CosmicReDroid instance configuration and hardware spoofing.

use crate::app::Message;
use crate::models::{AndroidInstance, DevicePreset};
use cosmic::app::context_drawer::{context_drawer, ContextDrawer};
use cosmic::iced::Alignment;
use cosmic::widget;

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
}

pub fn view<'a>(
    drawer_instance: &'a AndroidInstance,
    drawer_state: &'a DrawerState,
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

    // Section 1: Device Preset
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

    // Section 2: Hardware Spoofing
    let id_header = widget::Row::new()
        .spacing(12)
        .align_y(Alignment::Center)
        .push(widget::text::title4("Hardware Identity"))
        .push(
            widget::button::standard("Randomize All")
                .leading_icon(widget::icon::from_name("media-playlist-shuffle-symbolic"))
                .on_press(Message::DrawerRandomizeIdentity),
        );

    let identity_section = widget::Column::new()
        .spacing(8)
        .push(id_header)
        .push(widget::text::caption("Instance Display Name"))
        .push(
            widget::text_input("Name", &drawer_instance.name)
                .on_input(Message::DrawerNameChanged),
        )
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
        )
        .push(widget::text::caption("IMEI (Luhn 15-digit)"))
        .push(
            widget::text_input("IMEI", &drawer_instance.identity.imei)
                .on_input(Message::DrawerImeiChanged),
        )
        .push(widget::text::caption("Android ID (16-char hex)"))
        .push(
            widget::text_input("Android ID", &drawer_instance.identity.android_id)
                .on_input(Message::DrawerAndroidIdChanged),
        )
        .push(widget::text::caption("Serial Number"))
        .push(
            widget::text_input("Serial", &drawer_instance.identity.serial_no)
                .on_input(Message::DrawerSerialChanged),
        )
        .push(widget::text::caption("MAC Address"))
        .push(
            widget::text_input("MAC Address", &drawer_instance.identity.mac_address)
                .on_input(Message::DrawerMacChanged),
        );

    body = body.push(identity_section);

    // Section 3: Display & Performance
    let display_row1 = widget::Row::new()
        .spacing(12)
        .push(
            widget::Column::new()
                .spacing(4)
                .push(widget::text::caption("Width (px)"))
                .push(
                    widget::text_input("Width", &drawer_state.width)
                        .on_input(Message::DrawerWidthChanged),
                ),
        )
        .push(
            widget::Column::new()
                .spacing(4)
                .push(widget::text::caption("Height (px)"))
                .push(
                    widget::text_input("Height", &drawer_state.height)
                        .on_input(Message::DrawerHeightChanged),
                ),
        );

    let display_row2 = widget::Row::new()
        .spacing(12)
        .push(
            widget::Column::new()
                .spacing(4)
                .push(widget::text::caption("DPI"))
                .push(
                    widget::text_input("DPI", &drawer_state.dpi)
                        .on_input(Message::DrawerDpiChanged),
                ),
        )
        .push(
            widget::Column::new()
                .spacing(4)
                .push(widget::text::caption("Target FPS"))
                .push(
                    widget::text_input("FPS", &drawer_state.fps)
                        .on_input(Message::DrawerFpsChanged),
                ),
        );

    let display_section = widget::Column::new()
        .spacing(8)
        .push(widget::text::title4("Display & Performance"))
        .push(display_row1)
        .push(display_row2);

    body = body.push(display_section);

    // Section 4: Proxy Isolation
    let mut proxy_section = widget::Column::new()
        .spacing(8)
        .push(widget::text::title4("Per-Instance Proxy Isolation"))
        .push(
            widget::toggler(drawer_instance.proxy.enabled)
                .label("Enable HTTP/SOCKS5 Proxy".to_string())
                .on_toggle(Message::DrawerProxyToggled),
        );

    if drawer_instance.proxy.enabled {
        let proto_idx = if drawer_instance.proxy.protocol == "socks5" {
            1
        } else {
            0
        };

        let proxy_inputs = widget::Column::new()
            .spacing(8)
            .push(widget::text::caption("Protocol"))
            .push(widget::dropdown(
                &["http", "socks5"],
                Some(proto_idx),
                Message::DrawerProxyProtocolSelected,
            ))
            .push(widget::text::caption("Proxy Host"))
            .push(
                widget::text_input("127.0.0.1 or hostname", &drawer_instance.proxy.host)
                    .on_input(Message::DrawerProxyHostChanged),
            )
            .push(widget::text::caption("Proxy Port"))
            .push(
                widget::text_input("8080", &drawer_state.proxy_port)
                    .on_input(Message::DrawerProxyPortChanged),
            );

        proxy_section = proxy_section.push(proxy_inputs);
    }

    body = body.push(proxy_section);

    // Footer actions
    let footer_row = widget::Row::new()
        .spacing(12)
        .align_y(Alignment::Center)
        .push(
            widget::button::suggested("Save and Apply")
                .leading_icon(widget::icon::from_name("document-save-symbolic"))
                .on_press(Message::SaveDrawerInstance),
        )
        .push(widget::button::standard("Cancel").on_press(Message::CloseDrawer));

    context_drawer(widget::scrollable(body), Message::CloseDrawer)
        .title(title)
        .footer(footer_row)
}
