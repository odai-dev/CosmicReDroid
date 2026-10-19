//! Application model and state machine for CosmicReDroid.

use crate::models::{
    AndroidInstance, AndroidVersion, ContainerMetrics, DevicePreset, HostHealthReport,
    ImageDownloadProgress, InstanceStatus, NavPage, ProxyConfig, SnapshotInfo,
};
use crate::ui::drawer::DrawerState;
use crate::ui::image_hub::{ImageFactoryPreset, ImageFactoryState, ImageHubTab};
use cosmic::app::{context_drawer::ContextDrawer, Core, Task};
use cosmic::iced::Alignment;
use cosmic::widget::{self, nav_bar};
use cosmic::{Action, Application, ApplicationExt, Element};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

pub struct AppModel {
    pub core: Core,
    pub nav_model: nav_bar::Model,
    pub config: crate::config::AppConfig,
    pub active_page: NavPage,
    pub metrics: HashMap<Uuid, ContainerMetrics>,
    pub snapshots: Vec<SnapshotInfo>,
    pub host_health: HostHealthReport,
    pub drawer_instance: Option<AndroidInstance>,
    pub drawer_state: DrawerState,
    pub is_new_instance_drawer: bool,
    pub image_factory: ImageFactoryState,
    pub is_refreshing_health: bool,
    pub is_remediating: bool,
    pub active_downloads: HashMap<String, ImageDownloadProgress>,
    pub available_images: HashSet<String>,
    pub download_cancels: HashMap<String, tokio::sync::oneshot::Sender<()>>,
    pub pending_start_instances: HashMap<String, HashSet<Uuid>>,
    pub confirm_delete_id: Option<Uuid>,
    pub confirm_restore_id: Option<Uuid>,
    pub confirm_delete_snap_id: Option<Uuid>,
    pub mask_identifiers: bool,
    pub notification: Option<(String, bool)>,
}

#[derive(Debug, Clone)]
pub enum Message {
    NavSelect(nav_bar::Id),
    NewInstanceClicked,
    RefreshHealthClicked,

    // Instance Lifecycle
    StartInstance(Uuid),
    StopInstance(Uuid),
    OpenScreen(Uuid),
    CloneInstance(Uuid),
    InstanceCloned(Result<AndroidInstance, (Uuid, String)>),
    SnapshotInstance(Uuid),
    ConfigureInstance(Uuid),
    ConfirmDeleteInstance(Uuid),
    CancelDeleteInstance,
    DeleteInstance(Uuid),
    InstanceOperationFinished(Result<(Uuid, String), (Uuid, String)>),

    // Image Downloads & Caching
    StartImageDownload(String),
    ImageDownloadProgressUpdated(ImageDownloadProgress),
    CancelImageDownload(String),
    LocalImagesRefreshed(HashSet<String>),

    // Context Drawer Inputs
    CloseDrawer,
    DrawerVersionSelected(usize),
    DrawerNameChanged(String),
    DrawerPresetSelected(usize),
    DrawerManufacturerChanged(String),
    DrawerBrandChanged(String),
    DrawerModelChanged(String),
    DrawerDeviceChanged(String),
    DrawerBoardChanged(String),
    DrawerFingerprintChanged(String),
    DrawerImeiChanged(String),
    DrawerAndroidIdChanged(String),
    DrawerSerialChanged(String),
    DrawerMacChanged(String),
    DrawerRandomizeIdentity,
    DrawerWidthChanged(String),
    DrawerHeightChanged(String),
    DrawerDpiChanged(String),
    DrawerFpsChanged(String),
    DrawerProxyToggled(bool),
    DrawerProxyProtocolSelected(usize),
    DrawerProxyHostChanged(String),
    DrawerProxyPortChanged(String),
    SaveDrawerInstance,

    // Image Factory
    ImageHubTabSelected(ImageHubTab),
    ImageFactoryVersionSelected(usize),
    ImageFactoryPresetSelected(ImageFactoryPreset),
    ImageFactoryToggleLibndk(bool),
    ImageFactoryToggleGapps(bool),
    ImageFactoryToggleMagisk(bool),
    ImageFactoryToggleDockerfilePreview,
    ImageFactoryStartBuild,
    ImageFactoryBuildFinished(Result<String, String>),
    DeleteCustomImage(String),

    // Snapshots & Diagnostics
    ConfirmRestoreSnapshot(Uuid),
    CancelRestoreSnapshot,
    RestoreSnapshot(Uuid),
    ConfirmDeleteSnapshot(Uuid),
    CancelDeleteSnapshot,
    DeleteSnapshot(Uuid),
    SnapshotsLoaded(Vec<SnapshotInfo>),
    HostHealthLoaded(HostHealthReport),
    RemediationClicked,
    RemediationFinished(Result<(), String>),

    // UI & Privacy
    ToggleMaskIdentifiers,
    DismissNotification,

    // Periodic & Telemetry
    PeriodicTick,
    MetricsPolled(HashMap<Uuid, ContainerMetrics>),
}

impl Application for AppModel {
    type Executor = cosmic::executor::Default;
    type Flags = ();
    type Message = Message;
    const APP_ID: &'static str = "com.system76.CosmicRedroid";

    fn core(&self) -> &Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    fn init(core: Core, _flags: Self::Flags) -> (Self, Task<Self::Message>) {
        let (loaded_config, config_err) = match crate::config::load_config() {
            Ok(cfg) => (cfg, None),
            Err(err) => {
                let instances = crate::config::seed_default_instances();
                (
                    crate::config::AppConfig {
                        instances,
                        active_page: Some(NavPage::Instances),
                    },
                    Some(format!("Config warning: {err}. Defaults restored.")),
                )
            }
        };

        let mut nav_model = nav_bar::Model::default();
        for page in NavPage::ALL {
            nav_model
                .insert()
                .text(page.title())
                .icon(cosmic::widget::icon::from_name(page.icon_name()))
                .data(page);
        }

        let active_page = loaded_config.active_page.unwrap_or(NavPage::Instances);
        let active_pos = NavPage::ALL
            .iter()
            .position(|p| *p == active_page)
            .unwrap_or(0);
        nav_model.activate_position(active_pos as u16);

        let notification = config_err.map(|msg| (msg, true));

        let mut app = Self {
            core,
            nav_model,
            config: loaded_config,
            active_page,
            metrics: HashMap::new(),
            snapshots: Vec::new(),
            host_health: HostHealthReport::default(),
            drawer_instance: None,
            drawer_state: DrawerState::default(),
            is_new_instance_drawer: false,
            image_factory: ImageFactoryState::default(),
            is_refreshing_health: false,
            is_remediating: false,
            active_downloads: HashMap::new(),
            available_images: HashSet::new(),
            download_cancels: HashMap::new(),
            pending_start_instances: HashMap::new(),
            confirm_delete_id: None,
            confirm_restore_id: None,
            confirm_delete_snap_id: None,
            mask_identifiers: true,
            notification,
        };

        app.set_header_title(app.active_page.title().to_string());

        let initial_task = Task::batch(vec![
            Task::perform(
                async {
                    crate::backend::storage::list_all_snapshots()
                        .unwrap_or_default()
                },
                |snaps| Action::App(Message::SnapshotsLoaded(snaps)),
            ),
            Task::perform(
                async { crate::backend::host::probe_host_health().await },
                |report| Action::App(Message::HostHealthLoaded(report)),
            ),
            Task::perform(
                async {
                    if let Ok(engine) = crate::backend::docker::DockerEngine::new() {
                        engine.list_local_images().await.unwrap_or_default()
                    } else {
                        HashSet::new()
                    }
                },
                |images| Action::App(Message::LocalImagesRefreshed(images)),
            ),
        ]);

        (app, initial_task)
    }

    fn nav_model(&self) -> Option<&nav_bar::Model> {
        Some(&self.nav_model)
    }

    fn on_nav_select(&mut self, id: nav_bar::Id) -> Task<Self::Message> {
        self.nav_model.activate(id);
        if let Some(page) = self.nav_model.active_data::<NavPage>() {
            self.active_page = *page;
            self.set_header_title(self.active_page.title().to_string());
            self.config.active_page = Some(self.active_page);
            let _ = crate::config::save_config(&self.config);
        }
        Task::none()
    }

    fn header_start(&self) -> Vec<Element<'_, Self::Message>> {
        let running_count = self
            .config
            .instances
            .iter()
            .filter(|i| i.status.is_running())
            .count();

        let badge = widget::container(
            widget::Row::new()
                .spacing(6)
                .align_y(Alignment::Center)
                .push(widget::icon::from_name("media-playback-start-symbolic").size(14))
                .push(widget::text::body(format!("{} Active", running_count))),
        )
        .padding([2, 10])
        .class(cosmic::theme::Container::Card);

        vec![badge.into()]
    }

    fn header_end(&self) -> Vec<Element<'_, Self::Message>> {
        let refresh_btn = widget::button::standard("Refresh Health")
            .leading_icon(widget::icon::from_name("view-refresh-symbolic"))
            .on_press(Message::RefreshHealthClicked);

        let new_btn = widget::button::suggested("New Instance")
            .leading_icon(widget::icon::from_name("list-add-symbolic"))
            .on_press(Message::NewInstanceClicked);

        vec![refresh_btn.into(), new_btn.into()]
    }

    fn context_drawer(&self) -> Option<ContextDrawer<'_, Self::Message>> {
        if let Some(ref inst) = self.drawer_instance {
            Some(crate::ui::drawer::view(
                inst,
                &self.drawer_state,
                &self.available_images,
                self.is_new_instance_drawer,
            ))
        } else {
            None
        }
    }

    fn subscription(&self) -> cosmic::iced::Subscription<Self::Message> {
        cosmic::iced::time::every(std::time::Duration::from_secs(2))
            .map(|_| Message::PeriodicTick)
    }

    fn view(&self) -> Element<'_, Self::Message> {
        match self.active_page {
            NavPage::Instances => crate::ui::dashboard::view(
                &self.config.instances,
                &self.metrics,
                &self.active_downloads,
                self.confirm_delete_id,
                self.mask_identifiers,
                self.notification.as_ref(),
            ),
            NavPage::ImageFactory => {
                crate::ui::image_hub::view(&self.image_factory, &self.available_images, &self.active_downloads)
            }
            NavPage::Snapshots => crate::ui::host_health::view_snapshots(
                &self.snapshots,
                self.confirm_restore_id,
                self.confirm_delete_snap_id,
            ),
            NavPage::HostHealth => crate::ui::host_health::view_host_health(
                &self.host_health,
                self.is_refreshing_health,
                self.is_remediating,
            ),
        }
    }

    fn update(&mut self, message: Self::Message) -> Task<Self::Message> {
        match message {
            Message::NavSelect(id) => self.on_nav_select(id),

            Message::NewInstanceClicked => {
                let (next_port, next_binder) =
                    crate::backend::storage::allocate_next_port_and_binder(&self.config.instances);

                let id = Uuid::new_v4();
                let base_dir = crate::config::config_dir().unwrap_or_else(|_| ".".into());
                let data_path = base_dir.join("instances").join(id.to_string()).join("data");

                let mut new_inst = AndroidInstance {
                    id,
                    name: format!("Instance {}", self.config.instances.len() + 1),
                    container_name: format!("cosmic-redroid-{}", id.simple()),
                    android_version: AndroidVersion::Android13,
                    image_tag: AndroidVersion::Android13.image_tag().to_string(),
                    preset: DevicePreset::GalaxyS24Ultra,
                    identity: crate::models::HardwareIdentity::galaxy_s24_ultra(),
                    proxy: ProxyConfig::default(),
                    adb_port: next_port,
                    data_path,
                    binder_index: next_binder,
                    status: InstanceStatus::Stopped,
                };
                new_inst.identity = crate::backend::storage::regenerate_instance_identity(
                    &new_inst.identity,
                    new_inst.preset.tac_prefix(),
                );

                self.drawer_state = DrawerState::from_instance(&new_inst);
                self.drawer_instance = Some(new_inst);
                self.is_new_instance_drawer = true;
                self.set_show_context(true);
                Task::none()
            }

            Message::ConfigureInstance(id) => {
                if let Some(inst) = self.config.instances.iter().find(|i| i.id == id) {
                    if !inst.can_configure() {
                        return Task::none();
                    }
                    self.drawer_state = DrawerState::from_instance(inst);
                    self.drawer_instance = Some(inst.clone());
                    self.is_new_instance_drawer = false;
                    self.set_show_context(true);
                }
                Task::none()
            }

            Message::CloseDrawer => {
                self.drawer_instance = None;
                self.set_show_context(false);
                Task::none()
            }

            Message::RefreshHealthClicked => {
                self.is_refreshing_health = true;
                Task::perform(
                    async { crate::backend::host::probe_host_health().await },
                    |report| Action::App(Message::HostHealthLoaded(report)),
                )
            }

            Message::HostHealthLoaded(report) => {
                self.host_health = report;
                self.is_refreshing_health = false;
                Task::none()
            }

            Message::LocalImagesRefreshed(images) => {
                self.available_images = images;
                Task::none()
            }

            Message::StartImageDownload(image_tag) => {
                let engine = match crate::backend::docker::DockerEngine::new() {
                    Ok(e) => e,
                    Err(err) => {
                        self.notification = Some((format!("Cannot connect to Docker daemon: {err}"), true));
                        return Task::none();
                    }
                };

                let initial_prog = ImageDownloadProgress::new(&image_tag);
                self.active_downloads.insert(image_tag.clone(), initial_prog);

                let (cancel_tx, cancel_rx) = tokio::sync::oneshot::channel::<()>();
                self.download_cancels.insert(image_tag.clone(), cancel_tx);

                let stream = engine.pull_image_stream_cancellable(image_tag, cancel_rx);
                Task::run(stream, |prog| Action::App(Message::ImageDownloadProgressUpdated(prog)))
            }

            Message::ImageDownloadProgressUpdated(prog) => {
                if prog.is_complete {
                    self.download_cancels.remove(&prog.image_tag);
                    self.active_downloads.remove(&prog.image_tag);
                    self.available_images.insert(prog.image_tag.clone());

                    let mut restart_tasks = Vec::new();
                    if let Some(pending_ids) = self.pending_start_instances.remove(&prog.image_tag) {
                        for inst in self.config.instances.iter_mut() {
                            if pending_ids.contains(&inst.id) {
                                inst.status = InstanceStatus::Stopped;
                                restart_tasks.push(Task::done(Action::App(Message::StartInstance(inst.id))));
                            }
                        }
                    }
                    let _ = crate::config::save_config(&self.config);
                    if !restart_tasks.is_empty() {
                        return Task::batch(restart_tasks);
                    }
                } else if let Some(ref err) = prog.error {
                    self.download_cancels.remove(&prog.image_tag);
                    self.active_downloads.remove(&prog.image_tag);
                    if let Some(pending_ids) = self.pending_start_instances.remove(&prog.image_tag) {
                        for inst in self.config.instances.iter_mut() {
                            if pending_ids.contains(&inst.id) {
                                inst.status = InstanceStatus::Error(format!("Download failed: {err}"));
                            }
                        }
                    }
                    self.notification = Some((format!("Image download failed: {err}"), true));
                    let _ = crate::config::save_config(&self.config);
                } else {
                    let label = format!("Downloading: {}", prog.formatted_progress());
                    if let Some(pending_ids) = self.pending_start_instances.get(&prog.image_tag) {
                        for inst in self.config.instances.iter_mut() {
                            if pending_ids.contains(&inst.id) {
                                inst.status = InstanceStatus::DownloadingImage(label.clone());
                            }
                        }
                    }
                    self.active_downloads.insert(prog.image_tag.clone(), prog);
                }
                Task::none()
            }

            Message::CancelImageDownload(image_tag) => {
                if let Some(tx) = self.download_cancels.remove(&image_tag) {
                    let _ = tx.send(());
                }
                self.active_downloads.remove(&image_tag);
                if let Some(pending_ids) = self.pending_start_instances.remove(&image_tag) {
                    for inst in self.config.instances.iter_mut() {
                        if pending_ids.contains(&inst.id) {
                            inst.status = InstanceStatus::Stopped;
                        }
                    }
                }
                let _ = crate::config::save_config(&self.config);
                Task::none()
            }

            Message::StartInstance(id) => {
                let inst_opt = self.config.instances.iter().find(|i| i.id == id).cloned();
                let Some(target) = inst_opt else {
                    return Task::none();
                };

                if !target.status.can_start() {
                    return Task::none();
                }

                // Check if base container image is available locally
                let is_cached = self.available_images.contains(&target.image_tag);
                if !is_cached {
                    let tag = target.image_tag.clone();
                    let engine = match crate::backend::docker::DockerEngine::new() {
                        Ok(e) => e,
                        Err(err) => {
                            if let Some(inst) = self.config.instances.iter_mut().find(|i| i.id == id) {
                                inst.status = InstanceStatus::Error("Failed to connect to Docker engine".to_string());
                            }
                            let _ = crate::config::save_config(&self.config);
                            self.notification = Some((format!("Docker connection error: {err}"), true));
                            return Task::none();
                        }
                    };

                    if let Some(inst) = self.config.instances.iter_mut().find(|i| i.id == id) {
                        inst.status = InstanceStatus::DownloadingImage("Starting download...".to_string());
                    }
                    self.pending_start_instances
                        .entry(tag.clone())
                        .or_default()
                        .insert(id);
                    let _ = crate::config::save_config(&self.config);

                    if self.active_downloads.contains_key(&tag) {
                        return Task::none();
                    }

                    let initial_prog = ImageDownloadProgress::new(&tag);
                    self.active_downloads.insert(tag.clone(), initial_prog);

                    let (cancel_tx, cancel_rx) = tokio::sync::oneshot::channel::<()>();
                    self.download_cancels.insert(tag.clone(), cancel_tx);

                    let stream = engine.pull_image_stream_cancellable(tag, cancel_rx);
                    return Task::run(stream, |prog| Action::App(Message::ImageDownloadProgressUpdated(prog)));
                }

                if let Some(inst) = self.config.instances.iter_mut().find(|i| i.id == id) {
                    inst.status = InstanceStatus::Starting;
                }
                let _ = crate::config::save_config(&self.config);

                return Task::perform(
                    async move {
                        if let Err(err) =
                            crate::backend::spoofing::write_local_prop_async(
                                &target.data_path,
                                &target.identity,
                            )
                            .await
                        {
                            return Err((target.id, format!("Failed to write local.prop: {err:#}")));
                        }

                        let engine = match crate::backend::docker::DockerEngine::new() {
                            Ok(e) => e,
                            Err(err) => return Err((target.id, format!("Docker connection error: {err:#}"))),
                        };

                        if let Err(err) = engine.create_instance_container(&target).await {
                            return Err((target.id, format!("Failed to create container: {err:#}")));
                        }

                        if let Err(err) = engine.start_container(&target.container_name).await {
                            return Err((target.id, format!("Failed to start container: {err:#}")));
                        }

                        let _ = crate::backend::adb_scrcpy::adb_connect(target.adb_port).await;

                        if let Err(err) =
                            crate::backend::adb_scrcpy::wait_for_boot(target.adb_port, 45).await
                        {
                            let _ = engine.stop_container_with_timeout(&target.container_name, Some(5)).await;
                            let _ = crate::backend::adb_scrcpy::adb_disconnect(target.adb_port).await;
                            return Err((target.id, format!("Boot timeout: {err:#}")));
                        }

                        if let Err(err) = crate::backend::adb_scrcpy::inject_identity_and_proxy(
                            target.adb_port,
                            &target.identity,
                            &target.proxy,
                        )
                        .await
                        {
                            let _ = engine.stop_container_with_timeout(&target.container_name, Some(5)).await;
                            let _ = crate::backend::adb_scrcpy::adb_disconnect(target.adb_port).await;
                            return Err((target.id, format!("Identity injection failed: {err:#}")));
                        }

                        let _ = crate::backend::adb_scrcpy::spawn_scrcpy_for_instance(&target);

                        Ok((target.id, "Started successfully".to_string()))
                    },
                    |res| Action::App(Message::InstanceOperationFinished(res)),
                );
            }

            Message::StopInstance(id) => {
                if let Some(inst) = self.config.instances.iter_mut().find(|i| i.id == id) {
                    if !inst.status.is_running() {
                        return Task::none();
                    }
                    inst.status = InstanceStatus::Stopping;
                    let target = inst.clone();
                    let _ = crate::config::save_config(&self.config);

                    return Task::perform(
                        async move {
                            let engine = crate::backend::docker::DockerEngine::new()
                                .map_err(|e| (target.id, format!("Docker connection error: {e:#}")))?;
                            engine
                                .stop_container_with_timeout(&target.container_name, Some(10))
                                .await
                                .map_err(|e| (target.id, format!("Failed to stop container: {e:#}")))?;
                            crate::backend::adb_scrcpy::adb_disconnect(target.adb_port)
                                .await
                                .map_err(|e| (target.id, format!("Failed to disconnect adb: {e:#}")))?;
                            Ok((target.id, "Stopped".to_string()))
                        },
                        |res| Action::App(Message::InstanceOperationFinished(res)),
                    );
                }
                Task::none()
            }

            Message::OpenScreen(id) => {
                if let Some(inst) = self.config.instances.iter().find(|i| i.id == id).cloned() {
                    return Task::perform(
                        async move {
                            let _ = crate::backend::adb_scrcpy::adb_connect(inst.adb_port).await;
                            crate::backend::adb_scrcpy::spawn_scrcpy_for_instance(&inst)
                                .map(|_| ())
                                .map_err(|e| e.to_string())
                        },
                        |res| {
                            if let Err(e) = res {
                                tracing::error!("Failed to open scrcpy screen: {e}");
                            }
                            Action::App(Message::PeriodicTick)
                        },
                    );
                }
                Task::none()
            }

            Message::CloneInstance(id) => {
                if let Some(source) = self.config.instances.iter().find(|i| i.id == id).cloned() {
                    if !source.can_clone() {
                        return Task::none();
                    }
                    if let Some(inst) = self.config.instances.iter_mut().find(|i| i.id == id) {
                        inst.status = InstanceStatus::Cloning;
                    }
                    let _ = crate::config::save_config(&self.config);

                    let all_instances = self.config.instances.clone();
                    let source_id = source.id;
                    return Task::perform(
                        async move {
                            crate::backend::storage::clone_instance(&source, &all_instances, None)
                                .await
                                .map_err(|e| (source_id, e.to_string()))
                        },
                        |res| Action::App(Message::InstanceCloned(res)),
                    );
                }
                Task::none()
            }

            Message::InstanceCloned(res) => {
                match res {
                    Ok(cloned) => {
                        for inst in self.config.instances.iter_mut() {
                            if matches!(inst.status, InstanceStatus::Cloning) {
                                inst.status = InstanceStatus::Stopped;
                            }
                        }
                        let name = cloned.name.clone();
                        self.config.instances.push(cloned);
                        let _ = crate::config::save_config(&self.config);
                        self.notification = Some((format!("Cloned instance '{name}' successfully"), false));
                    }
                    Err((source_id, err)) => {
                        tracing::error!("Clone error for instance {source_id}: {err}");
                        if let Some(inst) = self.config.instances.iter_mut().find(|i| i.id == source_id) {
                            inst.status = InstanceStatus::Stopped;
                        }
                        let _ = crate::config::save_config(&self.config);
                        self.notification = Some((format!("Failed to clone instance: {err}"), true));
                    }
                }
                Task::none()
            }

            Message::SnapshotInstance(id) => {
                if let Some(inst) = self.config.instances.iter().find(|i| i.id == id).cloned() {
                    if !inst.can_snapshot() {
                        self.notification = Some((
                            "Cannot snapshot instance while it is running or transitional".to_string(),
                            true,
                        ));
                        return Task::none();
                    }
                    return Task::perform(
                        async move {
                            let snap_name = format!("snap-{}", chrono::Utc::now().format("%Y%m%d-%H%M%S"));
                            let _ = crate::backend::storage::create_snapshot(
                                &inst.id,
                                &inst.data_path,
                                &snap_name,
                            )
                            .await
                            .map_err(|e| e.to_string())?;
                            let snaps = crate::backend::storage::list_all_snapshots()
                                .unwrap_or_default();
                            Ok(snaps)
                        },
                        |res: Result<Vec<SnapshotInfo>, String>| match res {
                            Ok(snaps) => Action::App(Message::SnapshotsLoaded(snaps)),
                            Err(e) => {
                                tracing::error!("Snapshot error: {e}");
                                Action::App(Message::PeriodicTick)
                            }
                        },
                    );
                }
                Task::none()
            }

            Message::ConfirmDeleteInstance(id) => {
                self.confirm_delete_id = Some(id);
                Task::none()
            }

            Message::CancelDeleteInstance => {
                self.confirm_delete_id = None;
                Task::none()
            }

            Message::DeleteInstance(id) => {
                self.confirm_delete_id = None;
                if let Some(pos) = self.config.instances.iter().position(|i| i.id == id) {
                    let removed = self.config.instances.remove(pos);
                    let _ = crate::config::save_config(&self.config);

                    return Task::perform(
                        async move {
                            if let Ok(engine) = crate::backend::docker::DockerEngine::new() {
                                let _ = engine
                                    .stop_container_with_timeout(&removed.container_name, Some(5))
                                    .await;
                                let _ = engine
                                    .delete_container_opts(&removed.container_name, true)
                                    .await;
                            }
                            let _ = crate::backend::adb_scrcpy::adb_disconnect(removed.adb_port).await;

                            if let Some(parent) = removed.data_path.parent() {
                                let _ = tokio::fs::remove_dir_all(parent).await;
                            } else {
                                let _ = tokio::fs::remove_dir_all(&removed.data_path).await;
                            }

                            if let Ok(snaps_dir) = crate::config::snapshots_dir() {
                                let inst_snaps = snaps_dir.join(removed.id.to_string());
                                if inst_snaps.exists() {
                                    let _ = tokio::fs::remove_dir_all(&inst_snaps).await;
                                }
                            }

                            Ok((removed.id, "Deleted".to_string()))
                        },
                        |res| Action::App(Message::InstanceOperationFinished(res)),
                    );
                }
                Task::none()
            }

            Message::InstanceOperationFinished(res) => {
                match res {
                    Ok((id, status_desc)) => {
                        if let Some(inst) = self.config.instances.iter_mut().find(|i| i.id == id) {
                            if status_desc.contains("Started") {
                                inst.status = InstanceStatus::Running;
                            } else if status_desc.contains("Stopped") {
                                inst.status = InstanceStatus::Stopped;
                            }
                        }
                        if status_desc.contains("Deleted") {
                            return Task::perform(
                                async { crate::backend::storage::list_all_snapshots().unwrap_or_default() },
                                |snaps| Action::App(Message::SnapshotsLoaded(snaps)),
                            );
                        }
                    }
                    Err((id, err)) => {
                        tracing::error!("Instance operation error for instance {id}: {err}");
                        if let Some(inst) = self.config.instances.iter_mut().find(|i| i.id == id) {
                            inst.status = InstanceStatus::Error(err.clone());
                        }
                        self.notification = Some((err, true));
                    }
                }
                let _ = crate::config::save_config(&self.config);
                Task::none()
            }

            // Drawer Field Changes
            Message::DrawerVersionSelected(idx) => {
                if let Some(ref mut inst) = self.drawer_instance {
                    if let Some(ver) = AndroidVersion::ALL.get(idx) {
                        inst.android_version = *ver;
                        inst.image_tag = ver.image_tag().to_string();
                    } else {
                        let mut custom_tags: Vec<String> = self
                            .available_images
                            .iter()
                            .filter(|t| {
                                t.starts_with("cosmic-redroid-custom:")
                                    || !AndroidVersion::ALL.iter().any(|v| v.image_tag() == **t)
                            })
                            .cloned()
                            .collect();
                        custom_tags.sort();
                        let custom_idx = idx - AndroidVersion::ALL.len();
                        if let Some(tag) = custom_tags.get(custom_idx) {
                            inst.image_tag = tag.clone();
                        }
                    }
                }
                Task::none()
            }

            // Drawer Field Changes
            Message::DrawerNameChanged(name) => {
                if let Some(ref mut inst) = self.drawer_instance {
                    inst.name = name;
                }
                Task::none()
            }

            Message::DrawerPresetSelected(idx) => {
                if let Some(ref mut inst) = self.drawer_instance {
                    if let Some(preset) = DevicePreset::ALL.get(idx) {
                        inst.preset = *preset;
                        inst.identity = preset.default_identity();
                        self.drawer_state.sync_from_instance(inst);
                    }
                }
                Task::none()
            }

            Message::DrawerManufacturerChanged(val) => {
                if let Some(ref mut inst) = self.drawer_instance {
                    inst.identity.manufacturer = val;
                }
                Task::none()
            }

            Message::DrawerBrandChanged(val) => {
                if let Some(ref mut inst) = self.drawer_instance {
                    inst.identity.brand = val;
                }
                Task::none()
            }

            Message::DrawerModelChanged(val) => {
                if let Some(ref mut inst) = self.drawer_instance {
                    inst.identity.model = val;
                }
                Task::none()
            }

            Message::DrawerDeviceChanged(val) => {
                if let Some(ref mut inst) = self.drawer_instance {
                    inst.identity.device = val;
                }
                Task::none()
            }

            Message::DrawerBoardChanged(val) => {
                if let Some(ref mut inst) = self.drawer_instance {
                    inst.identity.board = val;
                }
                Task::none()
            }

            Message::DrawerFingerprintChanged(val) => {
                if let Some(ref mut inst) = self.drawer_instance {
                    inst.identity.fingerprint = val;
                }
                Task::none()
            }

            Message::DrawerImeiChanged(val) => {
                if let Some(ref mut inst) = self.drawer_instance {
                    inst.identity.imei = val;
                }
                Task::none()
            }

            Message::DrawerAndroidIdChanged(val) => {
                if let Some(ref mut inst) = self.drawer_instance {
                    inst.identity.android_id = val;
                }
                Task::none()
            }

            Message::DrawerSerialChanged(val) => {
                if let Some(ref mut inst) = self.drawer_instance {
                    inst.identity.serial_no = val;
                }
                Task::none()
            }

            Message::DrawerMacChanged(val) => {
                if let Some(ref mut inst) = self.drawer_instance {
                    inst.identity.mac_address = val;
                }
                Task::none()
            }

            Message::DrawerRandomizeIdentity => {
                if let Some(ref mut inst) = self.drawer_instance {
                    inst.identity = crate::backend::storage::regenerate_instance_identity(
                        &inst.identity,
                        inst.preset.tac_prefix(),
                    );
                    self.drawer_state.sync_from_instance(inst);
                }
                Task::none()
            }

            Message::DrawerWidthChanged(val) => {
                self.drawer_state.width = val.clone();
                if let Ok(num) = val.parse::<u32>() {
                    if let Some(ref mut inst) = self.drawer_instance {
                        inst.identity.width = num;
                    }
                }
                Task::none()
            }

            Message::DrawerHeightChanged(val) => {
                self.drawer_state.height = val.clone();
                if let Ok(num) = val.parse::<u32>() {
                    if let Some(ref mut inst) = self.drawer_instance {
                        inst.identity.height = num;
                    }
                }
                Task::none()
            }

            Message::DrawerDpiChanged(val) => {
                self.drawer_state.dpi = val.clone();
                if let Ok(num) = val.parse::<u32>() {
                    if let Some(ref mut inst) = self.drawer_instance {
                        inst.identity.dpi = num;
                    }
                }
                Task::none()
            }

            Message::DrawerFpsChanged(val) => {
                self.drawer_state.fps = val.clone();
                if let Ok(num) = val.parse::<u32>() {
                    if let Some(ref mut inst) = self.drawer_instance {
                        inst.identity.fps = num;
                    }
                }
                Task::none()
            }

            Message::DrawerProxyToggled(enabled) => {
                if let Some(ref mut inst) = self.drawer_instance {
                    inst.proxy.enabled = enabled;
                }
                Task::none()
            }

            Message::DrawerProxyProtocolSelected(_idx) => {
                if let Some(ref mut inst) = self.drawer_instance {
                    inst.proxy.protocol = "http".into();
                }
                Task::none()
            }

            Message::DrawerProxyHostChanged(val) => {
                if let Some(ref mut inst) = self.drawer_instance {
                    inst.proxy.host = val;
                }
                Task::none()
            }

            Message::DrawerProxyPortChanged(val) => {
                self.drawer_state.proxy_port = val.clone();
                if let Ok(num) = val.parse::<u16>() {
                    if let Some(ref mut inst) = self.drawer_instance {
                        inst.proxy.port = num;
                    }
                }
                Task::none()
            }

            Message::SaveDrawerInstance => {
                if let Some(mut drawer_inst) = self.drawer_instance.take() {
                    if !self.drawer_state.is_valid(&drawer_inst) {
                        self.drawer_instance = Some(drawer_inst);
                        return Task::none();
                    }

                    if let Ok(w) = self.drawer_state.width.parse::<u32>() {
                        drawer_inst.identity.width = w;
                    }
                    if let Ok(h) = self.drawer_state.height.parse::<u32>() {
                        drawer_inst.identity.height = h;
                    }
                    if let Ok(d) = self.drawer_state.dpi.parse::<u32>() {
                        drawer_inst.identity.dpi = d;
                    }
                    if let Ok(f) = self.drawer_state.fps.parse::<u32>() {
                        drawer_inst.identity.fps = f;
                    }
                    if let Ok(p) = self.drawer_state.proxy_port.parse::<u16>() {
                        drawer_inst.proxy.port = p;
                    }

                    if drawer_inst.data_path.exists() {
                        let _ = crate::backend::spoofing::write_local_prop(
                            &drawer_inst.data_path,
                            &drawer_inst.identity,
                        );
                    }

                    let was_running = if self.is_new_instance_drawer {
                        false
                    } else if let Some(existing) = self
                        .config
                        .instances
                        .iter_mut()
                        .find(|i| i.id == drawer_inst.id)
                    {
                        let running = existing.status.is_running();
                        drawer_inst.status = existing.status.clone();
                        *existing = drawer_inst.clone();
                        running
                    } else {
                        false
                    };

                    if self.is_new_instance_drawer {
                        self.config.instances.push(drawer_inst);
                        self.notification = Some(("New instance created successfully".to_string(), false));
                    } else if was_running {
                        self.notification = Some((
                            "Settings saved. Restart instance for changes to take effect.".to_string(),
                            false,
                        ));
                    } else {
                        self.notification = Some(("Settings saved successfully".to_string(), false));
                    }

                    let _ = crate::config::save_config(&self.config);
                }
                self.set_show_context(false);
                Task::none()
            }

            // Image Factory
            Message::ImageFactoryVersionSelected(idx) => {
                if let Some(ver) = AndroidVersion::ALL.get(idx) {
                    self.image_factory.selected_version = *ver;
                }
                Task::none()
            }

            Message::ImageHubTabSelected(tab) => {
                self.image_factory.current_tab = tab;
                Task::none()
            }

            Message::ImageFactoryPresetSelected(preset) => {
                self.image_factory.apply_preset(preset);
                Task::none()
            }

            Message::ImageFactoryToggleDockerfilePreview => {
                self.image_factory.is_dockerfile_expanded =
                    !self.image_factory.is_dockerfile_expanded;
                Task::none()
            }

            Message::ImageFactoryToggleLibndk(val) => {
                self.image_factory.include_libndk = val;
                self.image_factory.active_preset = ImageFactoryPreset::Custom;
                Task::none()
            }

            Message::ImageFactoryToggleGapps(val) => {
                self.image_factory.include_gapps = val;
                self.image_factory.active_preset = ImageFactoryPreset::Custom;
                Task::none()
            }

            Message::ImageFactoryToggleMagisk(val) => {
                self.image_factory.include_magisk = val;
                self.image_factory.active_preset = ImageFactoryPreset::Custom;
                Task::none()
            }

            Message::ImageFactoryStartBuild => {
                self.image_factory.is_building = true;
                let tag = self.image_factory.target_image_tag();
                self.image_factory.build_logs.push(format!(
                    "Starting asynchronous build for target tag: {tag}..."
                ));

                let dockerfile = self.image_factory.generate_dockerfile();

                Task::perform(
                    async move {
                        let temp_dir =
                            std::env::temp_dir().join(format!("build_{}", Uuid::new_v4()));
                        tokio::fs::create_dir_all(&temp_dir)
                            .await
                            .map_err(|e| e.to_string())?;
                        tokio::fs::write(temp_dir.join("Dockerfile"), dockerfile)
                            .await
                            .map_err(|e| e.to_string())?;

                        let mut cmd = tokio::process::Command::new("docker");
                        cmd.arg("build").arg("-t").arg(&tag).arg(&temp_dir);

                        let res = cmd.output().await;
                        let _ = tokio::fs::remove_dir_all(&temp_dir).await;

                        let output = res.map_err(|e| e.to_string())?;
                        if output.status.success() {
                            Ok(format!("Build successful: {tag}"))
                        } else {
                            Err(String::from_utf8_lossy(&output.stderr).to_string())
                        }
                    },
                    |res| Action::App(Message::ImageFactoryBuildFinished(res)),
                )
            }

            Message::ImageFactoryBuildFinished(res) => {
                self.image_factory.is_building = false;
                match res {
                    Ok(msg) => {
                        self.image_factory.build_logs.push(msg);
                        return Task::perform(
                            async {
                                if let Ok(engine) = crate::backend::docker::DockerEngine::new() {
                                    engine.list_local_images().await.unwrap_or_default()
                                } else {
                                    HashSet::new()
                                }
                            },
                            |images| Action::App(Message::LocalImagesRefreshed(images)),
                        );
                    }
                    Err(err) => {
                        self.image_factory
                            .build_logs
                            .push(format!("Build failed: {err}"));
                    }
                }
                Task::none()
            }

            Message::DeleteCustomImage(tag) => {
                self.available_images.remove(&tag);
                Task::perform(
                    async move {
                        let mut cmd = tokio::process::Command::new("docker");
                        cmd.arg("rmi").arg("-f").arg(&tag);
                        let _ = cmd.output().await;

                        if let Ok(engine) = crate::backend::docker::DockerEngine::new() {
                            engine.list_local_images().await.unwrap_or_default()
                        } else {
                            HashSet::new()
                        }
                    },
                    |images| Action::App(Message::LocalImagesRefreshed(images)),
                )
            }

            // Snapshots & Remediation
            Message::ConfirmRestoreSnapshot(id) => {
                self.confirm_restore_id = Some(id);
                Task::none()
            }

            Message::CancelRestoreSnapshot => {
                self.confirm_restore_id = None;
                Task::none()
            }

            Message::RestoreSnapshot(id) => {
                self.confirm_restore_id = None;
                if let Some(snap) = self.snapshots.iter().find(|s| s.id == id).cloned() {
                    if let Some(target) = self
                        .config
                        .instances
                        .iter()
                        .find(|i| i.id == snap.instance_id)
                        .cloned()
                    {
                        if !target.can_snapshot() {
                            self.notification = Some((
                                "Cannot restore snapshot while instance is running or transitional".to_string(),
                                true,
                            ));
                            return Task::none();
                        }
                        return Task::perform(
                            async move {
                                crate::backend::storage::restore_snapshot(&snap, &target.data_path)
                                    .await
                                    .map_err(|e| (target.id, e.to_string()))?;
                                Ok((target.id, "Restored snapshot".to_string()))
                            },
                            |res| Action::App(Message::InstanceOperationFinished(res)),
                        );
                    }
                }
                Task::none()
            }

            Message::ConfirmDeleteSnapshot(id) => {
                self.confirm_delete_snap_id = Some(id);
                Task::none()
            }

            Message::CancelDeleteSnapshot => {
                self.confirm_delete_snap_id = None;
                Task::none()
            }

            Message::DeleteSnapshot(id) => {
                self.confirm_delete_snap_id = None;
                if let Some(snap) = self.snapshots.iter().find(|s| s.id == id).cloned() {
                    self.snapshots.retain(|s| s.id != id);
                    return Task::perform(
                        async move {
                            let _ = crate::backend::storage::delete_snapshot_by_id(&snap.instance_id, &id);
                            crate::backend::storage::list_all_snapshots()
                                .unwrap_or_default()
                        },
                        |snaps| Action::App(Message::SnapshotsLoaded(snaps)),
                    );
                }
                Task::none()
            }

            Message::SnapshotsLoaded(snapshots) => {
                self.snapshots = snapshots;
                Task::none()
            }

            Message::RemediationClicked => {
                self.is_remediating = true;
                Task::perform(
                    async {
                        crate::backend::host::remediate_host_environment()
                            .await
                            .map_err(|e| e.to_string())
                    },
                    |res| Action::App(Message::RemediationFinished(res)),
                )
            }

            Message::RemediationFinished(res) => {
                self.is_remediating = false;
                if let Err(err) = res {
                    tracing::error!("Host remediation error: {err}");
                }
                Task::perform(
                    async { crate::backend::host::probe_host_health().await },
                    |report| Action::App(Message::HostHealthLoaded(report)),
                )
            }

            Message::ToggleMaskIdentifiers => {
                self.mask_identifiers = !self.mask_identifiers;
                Task::none()
            }

            Message::DismissNotification => {
                self.notification = None;
                Task::none()
            }

            Message::PeriodicTick => {
                let running_instances: Vec<AndroidInstance> = self
                    .config
                    .instances
                    .iter()
                    .filter(|i| i.status.is_running())
                    .cloned()
                    .collect();

                if running_instances.is_empty() {
                    return Task::none();
                }

                Task::perform(
                    async move {
                        let mut map = HashMap::new();
                        if let Ok(engine) = crate::backend::docker::DockerEngine::new() {
                            for inst in running_instances {
                                if let Ok(metrics) =
                                    engine.get_container_metrics(&inst.container_name).await
                                {
                                    map.insert(inst.id, metrics);
                                }
                            }
                        }
                        map
                    },
                    |metrics| Action::App(Message::MetricsPolled(metrics)),
                )
            }

            Message::MetricsPolled(metrics) => {
                self.metrics = metrics;
                Task::none()
            }
        }
    }
}
