//! Android TV: the video is played by Media3 (ExoPlayer) in `PlayerPlugin.kt`, in a surface under the transparent
//! webview. The page keeps talking to "mpv": these commands take the same arguments as on the desktop and hand them
//! to the plugin, which answers with the same `mpv` events (see `PlayerPlugin.kt`). They run off the main thread
//! (`async`) because a call into Kotlin waits for its answer.

use std::collections::HashMap;

use serde_json::{Value, json};
use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::plugin::{Builder, PluginHandle, TauriPlugin};
use tauri::{Emitter, Manager, State, Wry};

pub struct Player(PluginHandle<Wry>);

impl Player {
    fn call(&self, command: &str, payload: Value) -> Result<(), String> {
        self.0.run_mobile_plugin::<Value>(command, payload).map(|_| ()).map_err(|e| e.to_string())
    }
}

/// Registers the Kotlin plugin and wires its events to the page's `mpv` listener.
pub fn plugin() -> TauriPlugin<Wry> {
    Builder::new("player")
        .setup(|app, api| {
            let handle = api.register_android_plugin("com.marcdownhq.nightglass", "PlayerPlugin")?;
            let emitter = app.clone();
            let events: Channel<Value> = Channel::new(move |body| {
                if let InvokeResponseBody::Json(json) = body
                    && let Ok(payload) = serde_json::from_str::<Value>(&json)
                {
                    let _ = emitter.emit("mpv", payload);
                }
                Ok(())
            });
            handle.run_mobile_plugin::<Value>("setEvents", json!({ "channel": events }))?;
            app.manage(Player(handle));
            Ok(())
        })
        .build()
}

#[tauri::command(async)]
pub fn mpv_play(player: State<'_, Player>, url: String, headers: HashMap<String, String>, options: Option<String>) -> Result<(), String> {
    player.call("play", json!({ "url": url, "headers": headers, "options": options.unwrap_or_default() }))
}

#[tauri::command(async)]
pub fn mpv_command(player: State<'_, Player>, args: Vec<String>) -> Result<(), String> {
    player.call("command", json!({ "args": args }))
}

#[tauri::command(async)]
pub fn mpv_set(player: State<'_, Player>, name: String, value: String) -> Result<(), String> {
    player.call("set", json!({ "name": name, "value": value }))
}

/// Where the video goes in the window: x, y, width, height in physical pixels.
#[tauri::command(async)]
pub fn video_place(player: State<'_, Player>, rect: [i32; 4]) {
    let _ = player.call("place", json!({ "x": rect[0], "y": rect[1], "w": rect[2], "h": rect[3] }));
}

#[tauri::command(async)]
pub fn open_url(player: State<'_, Player>, url: String) -> Result<(), String> {
    player.call("openUrl", json!({ "url": url }))
}

#[tauri::command]
pub async fn pip_open() -> Result<(), String> {
    Err("Picture in picture is not available on Android TV.".into())
}

#[tauri::command]
pub async fn pip_close() -> Result<(), String> {
    Ok(())
}
