use cosmic::app::{Core, Task};
use cosmic::widget;
use cosmic::{Application, Element};

pub struct AppModel {
    core: Core,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum Message {
    None,
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
        (Self { core }, Task::none())
    }

    fn update(&mut self, _message: Self::Message) -> Task<Self::Message> {
        Task::none()
    }

    fn view(&self) -> Element<'_, Self::Message> {
        widget::text(crate::fl!("app-title")).into()
    }
}
