use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use serde::{Deserialize, Serialize};
use winsafe::HWND;
use windows::Win32::UI::WindowsAndMessaging::PostMessageW;
use windows::Win32::Foundation::{HWND as WinHWND, WPARAM, LPARAM};

const WM_GLAZE_UPDATE: u32 = 0x8000 + 1;

const GLAZEWM_PORT: &str = "6123";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlazeWorkspace {
    pub id: String,
    pub name: Option<String>,
    #[serde(rename = "displayName")]
    pub display_name: Option<String>,
    #[serde(rename = "hasFocus")]
    pub has_focus: bool,
    #[serde(rename = "isDisplayed")]
    pub is_displayed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlazeState {
    pub workspaces: Vec<GlazeWorkspace>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GlazeResponse {
    #[serde(rename = "messageType")]
    message_type: String,
    data: Option<serde_json::Value>,
    error: Option<String>,
}

pub fn read_state() -> anyhow::Result<GlazeState> {
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(async {
        query_workspaces().await
    })
}

async fn query_workspaces() -> anyhow::Result<GlazeState> {
    let (mut ws_stream, _) = connect_async(format!("ws://localhost:{}", GLAZEWM_PORT))
        .await
        .map_err(|e| anyhow::anyhow!("Failed to connect to GlazeWM: {}", e))?;

    log::info!("Connected to GlazeWM");

    ws_stream.send(Message::Text("query workspaces".into())).await?;

    while let Some(msg) = ws_stream.next().await {
        let msg = msg.map_err(|e| anyhow::anyhow!("WebSocket error: {}", e))?;
        
        if let Message::Text(text) = msg {
            let response: GlazeResponse = serde_json::from_str(&text)
                .map_err(|e| anyhow::anyhow!("Failed to parse response: {}", e))?;

            if let Some(data) = response.data {
                let state: GlazeState = serde_json::from_value(data)
                    .map_err(|e| anyhow::anyhow!("Failed to parse workspaces: {}", e))?;
                return Ok(state);
            }
        }
    }

    anyhow::bail!("No response from GlazeWM")
}

pub fn focus_workspace(workspace_name: &str) -> anyhow::Result<()> {
    log::info!("focus_workspace called with name={}", workspace_name);
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(async {
        let (mut ws_stream, _) = connect_async(format!("ws://localhost:{}", GLAZEWM_PORT))
            .await
            .map_err(|e| anyhow::anyhow!("Failed to connect to GlazeWM: {}", e))?;

        // use the actual workspace name, not idx+1
        let command = format!("command focus --workspace {}", workspace_name);
        log::info!("Sending GlazeWM command: {}", command);
        ws_stream.send(Message::Text(command.into())).await?;

        while let Some(msg) = ws_stream.next().await {
            let msg = msg.map_err(|e| anyhow::anyhow!("WebSocket error: {}", e))?;
            if let Message::Text(text) = msg {
                let response: GlazeResponse = serde_json::from_str(&text)
                    .map_err(|e| anyhow::anyhow!("Failed to parse response: {}", e))?;
                if let Some(error) = response.error {
                    anyhow::bail!("GlazeWM error: {}", error);
                }
                return Ok(());
            }
        }
        Ok(())
    })
}

pub fn start_listen_for_workspaces(hwnd: HWND) -> anyhow::Result<std::thread::JoinHandle<()>> {
    let handle = std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().expect("Failed to create runtime");
        rt.block_on(async {
            if let Err(e) = listen_loop(hwnd).await {
                log::error!("Error in listen loop: {}", e);
            }
        });
    });
    Ok(handle)
}

async fn listen_loop(hwnd: HWND) -> anyhow::Result<()> {
    let (mut ws_stream, _) = connect_async(format!("ws://localhost:{}", GLAZEWM_PORT))
        .await
        .map_err(|e| anyhow::anyhow!("Failed to connect to GlazeWM: {}", e))?;

    log::info!("Connected to GlazeWM for subscriptions");

    ws_stream.send(Message::Text("sub --events workspace_updated focus_changed".into())).await?;

    let mut subscription_id: Option<String> = None;

    while let Some(msg) = ws_stream.next().await {
        let msg = msg.map_err(|e| anyhow::anyhow!("WebSocket error: {}", e))?;
        
        if let Message::Text(text) = msg {
            if let Ok(response) = serde_json::from_str::<GlazeResponse>(&text) {
                if response.message_type == "client_response" {
                    if let Some(data) = response.data {
                        if let Ok(sid) = serde_json::from_value::<serde_json::Value>(data.clone()) {
                            if let Some(id) = sid.get("subscriptionId").and_then(|v| v.as_str()) {
                                subscription_id = Some(id.to_string());
                                log::info!("Subscribed to GlazeWM events");
                            }
                        }
                    }
                } else if response.message_type == "event_subscription" {
                    if let Some(data) = response.data {
                        log::debug!("Received event: {:?}", data);
                        
                        unsafe {
                            let _ = PostMessageW(
                                Some(WinHWND(hwnd.ptr() as *mut std::ffi::c_void)),
                                WM_GLAZE_UPDATE,
                                WPARAM(0),
                                LPARAM(0),
                            );
                        }
                    }
                }
            }
        }
    }

    Ok(())
}
