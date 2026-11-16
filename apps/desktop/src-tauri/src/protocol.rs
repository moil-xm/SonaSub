use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const PROTOCOL_VERSION: u32 = 1;
pub const HANDSHAKE_PREFIX: &str = "SONASUB_HANDSHAKE ";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EngineInfo {
    pub engine_version: String,
    pub node_version: String,
    pub pid: u32,
    pub started_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Handshake {
    pub protocol_version: u32,
    pub port: u16,
    pub pid: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Request {
    Auth {
        #[serde(rename = "protocolVersion")]
        protocol_version: u32,
        token: String,
    },
    Request {
        #[serde(rename = "protocolVersion")]
        protocol_version: u32,
        id: String,
        method: String,
    },
}

#[derive(Debug, PartialEq, Eq)]
pub enum ProtocolError {
    InvalidJson,
    UnsupportedVersion,
    RequestMismatch,
    InvalidResponse,
    InvalidHandshake,
    Remote(String),
}

impl std::fmt::Display for ProtocolError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidJson => write!(formatter, "引擎消息不是有效 JSON。"),
            Self::UnsupportedVersion => write!(formatter, "引擎协议版本不受支持。"),
            Self::RequestMismatch => write!(formatter, "引擎响应与请求关联信息不一致。"),
            Self::InvalidResponse => write!(formatter, "引擎响应内容无效。"),
            Self::InvalidHandshake => write!(formatter, "引擎启动握手无效。"),
            Self::Remote(message) => write!(formatter, "{message}"),
        }
    }
}

pub fn parse_handshake(raw: &str) -> Result<Handshake, ProtocolError> {
    let payload = raw
        .strip_prefix(HANDSHAKE_PREFIX)
        .ok_or(ProtocolError::InvalidHandshake)?;
    let handshake: Handshake =
        serde_json::from_str(payload).map_err(|_| ProtocolError::InvalidHandshake)?;
    if handshake.protocol_version != PROTOCOL_VERSION || handshake.port == 0 || handshake.pid == 0 {
        return Err(ProtocolError::InvalidHandshake);
    }
    Ok(handshake)
}

pub fn parse_response(
    raw: &str,
    id: &str,
    method: &str,
) -> Result<Option<EngineInfo>, ProtocolError> {
    let value: Value = serde_json::from_str(raw).map_err(|_| ProtocolError::InvalidJson)?;
    if value["protocolVersion"] != PROTOCOL_VERSION {
        return Err(ProtocolError::UnsupportedVersion);
    }
    if value["type"] != "response" || value["id"] != id {
        return Err(ProtocolError::RequestMismatch);
    }
    if value["ok"] == false {
        let message = value["error"]["message"]
            .as_str()
            .ok_or(ProtocolError::InvalidResponse)?;
        return Err(ProtocolError::Remote(message.to_owned()));
    }
    if value["ok"] != true {
        return Err(ProtocolError::InvalidResponse);
    }
    if method == "engine.shutdown" {
        if !value["result"].is_null() {
            return Err(ProtocolError::InvalidResponse);
        }
        return Ok(None);
    }
    if method != "engine.info" {
        return Err(ProtocolError::InvalidResponse);
    }
    serde_json::from_value(value["result"].clone()).map_err(|_| ProtocolError::InvalidResponse)
}

pub fn auth_message(token: &str) -> String {
    serde_json::to_string(&Request::Auth {
        protocol_version: PROTOCOL_VERSION,
        token: token.to_owned(),
    })
    .expect("auth message is serializable")
}

pub fn request_message(id: &str, method: &str) -> String {
    serde_json::to_string(&Request::Request {
        protocol_version: PROTOCOL_VERSION,
        id: id.to_owned(),
        method: method.to_owned(),
    })
    .expect("control request is serializable")
}
