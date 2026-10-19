use super::{Adapter, Config, Event, Status};
use serde::Deserialize;

pub(super) struct Nsd {
    app: tauri::AppHandle,
    config: Config,
    started: bool,
}
#[derive(Deserialize)]
struct Response {
    events: Vec<Event>,
}
#[derive(Deserialize)]
struct StartResponse {
    state: Status,
}
impl Nsd {
    pub fn new(app: &tauri::AppHandle, config: &Config) -> Result<Self, Status> {
        Ok(Self {
            app: app.clone(),
            config: config.clone(),
            started: false,
        })
    }
    fn start(&mut self, browse: bool) -> Result<(), Status> {
        let mut args = serde_json::to_value(&self.config).map_err(|_| Status::Unavailable)?;
        args["browse"] = browse.into();
        let response: StartResponse =
            tauri_plugin_library_transfer::discovery_call(&self.app, "discoveryStart", args)
                .map_err(|_| Status::Unavailable)?;
        if response.state != Status::Starting {
            return Err(response.state);
        }
        self.started = true;
        Ok(())
    }
    fn stop(&mut self) -> Result<(), Status> {
        if self.started {
            let result: Result<(), _> = tauri_plugin_library_transfer::discovery_call(
                &self.app,
                "discoveryStop",
                serde_json::json!({"id":self.config.id}),
            );
            self.started = false;
            result.map_err(|_| Status::Unavailable)?;
        }
        Ok(())
    }
}
impl Adapter for Nsd {
    fn browse(&mut self) -> Result<(), Status> {
        self.start(true)
    }
    fn publish(&mut self, enabled: bool) -> Result<(), Status> {
        if enabled {
            self.start(false)
        } else {
            self.stop()
        }
    }
    fn poll(&mut self) -> Result<Vec<Event>, Status> {
        if !self.started {
            return Ok(vec![]);
        }
        let response: Response = tauri_plugin_library_transfer::discovery_call(
            &self.app,
            "discoveryPoll",
            serde_json::json!({"id":self.config.id}),
        )
        .map_err(|_| Status::Unavailable)?;
        Ok(response.events)
    }
}
impl Drop for Nsd {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}
