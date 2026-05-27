use endpoint_libs::libs::error_code::ErrorCode;
use endpoint_libs::libs::types::*;
use endpoint_libs::libs::ws::*;
use num_derive::FromPrimitive;
use psc_nanoid::{alphabet::Base62Alphabet, Nanoid};
use rkyv::Archive;
use serde::*;
use std::net::IpAddr;
use strum_macros::{Display, EnumString};
use uuid::Uuid;
use worktable::prelude::*;

#[derive(
    MemStat,
    Archive,
    Clone,
    Copy,
    Debug,
    Display,
    PartialEq,
    PartialOrd,
    Eq,
    Hash,
    Ord,
    EnumString,
    rkyv::Deserialize,
    rkyv::Serialize,
    serde::Serialize,
    serde::Deserialize,
)]
#[rkyv(compare(PartialEq), derive(Debug))]
#[repr(u8)]
pub enum LogLevel {
    /// Trace-level logging
    Trace = 0,
    /// Debug-level logging
    Debug = 1,
    /// Info-level logging
    Info = 2,
    /// Warn-level logging
    Warn = 3,
    /// Error-level logging
    Error = 4,
}

#[derive(
    MemStat,
    Archive,
    Clone,
    Copy,
    Debug,
    Display,
    PartialEq,
    PartialOrd,
    Eq,
    Hash,
    Ord,
    EnumString,
    rkyv::Deserialize,
    rkyv::Serialize,
    serde::Serialize,
    serde::Deserialize,
)]
#[rkyv(compare(PartialEq), derive(Debug))]
#[repr(u8)]
pub enum UserRole {
    /// Unauthenticated
    Public = 0,
    /// Platform admin
    Admin = 1,
    /// App frontend connection
    App = 2,
    /// User authenticated via honey.id token
    User = 3,
    /// honey.id callback endpoints
    HoneyAuth = 6,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    pub app_public_id: Nanoid<16, Base62Alphabet>,
    pub tg_bot_token: String,
    pub api_key: String,
    #[serde(default)]
    pub app_name: Option<String>,
    pub active: bool,
    pub created_at: i64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessage {
    pub incoming: bool,
    pub sent_at: i64,
    pub content: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ChatSession {
    pub session_id: Nanoid<16, Base62Alphabet>,
    pub app_public_id: Nanoid<16, Base62Alphabet>,
    pub user_pub_id: Nanoid<16, Base62Alphabet>,
    pub created_at: i64,
    #[serde(default)]
    pub closed_at: Option<i64>,
}

#[derive(
    Debug,
    Clone,
    Copy,
    Serialize,
    Deserialize,
    FromPrimitive,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    EnumString,
    Display,
    Hash,
)]
pub enum EnumEndpoint {
    ///
    Init = 1,
    ///
    AppConnect = 100,
    ///
    CreateSession = 110,
    ///
    SendMessage = 111,
    ///
    ListMessages = 112,
    ///
    SubscribeEvents = 113,
    ///
    CloseSession = 114,
    ///
    ListSessions = 115,
}

impl EnumEndpoint {
    pub fn schema(&self) -> endpoint_libs::model::EndpointSchema {
        let schema = match self {
            Self::Init => InitRequest::SCHEMA,
            Self::AppConnect => AppConnectRequest::SCHEMA,
            Self::CreateSession => CreateSessionRequest::SCHEMA,
            Self::SendMessage => SendMessageRequest::SCHEMA,
            Self::ListMessages => ListMessagesRequest::SCHEMA,
            Self::SubscribeEvents => SubscribeEventsRequest::SCHEMA,
            Self::CloseSession => CloseSessionRequest::SCHEMA,
            Self::ListSessions => ListSessionsRequest::SCHEMA,
        };
        serde_json::from_str(schema).unwrap()
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ErrorXxx {}

#[derive(
    Debug,
    Clone,
    Copy,
    Serialize,
    Deserialize,
    FromPrimitive,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    EnumString,
    Display,
    Hash,
)]
pub enum EnumErrorCode {
    /// None Please populate error_codes.json
    Xxx = 0,
}

impl From<EnumErrorCode> for ErrorCode {
    fn from(e: EnumErrorCode) -> Self {
        ErrorCode::new(e as _)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AppConnectRequest {
    pub app_public_id: Nanoid<16, Base62Alphabet>,
    pub user_public_id: Nanoid<16, Base62Alphabet>,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AppConnectResponse {
    pub app_public_id: Nanoid<16, Base62Alphabet>,
    #[serde(default)]
    pub app_name: Option<String>,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CloseSessionRequest {
    pub session_id: Nanoid<16, Base62Alphabet>,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CloseSessionResponse {}
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CreateSessionRequest {}
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CreateSessionResponse {
    pub session_id: Nanoid<16, Base62Alphabet>,
    pub created_at: i64,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct InitRequest {
    pub access_token: String,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct InitResponse {
    pub user_id: Nanoid<16, Base62Alphabet>,
    pub role: UserRole,
    pub version: String,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ListMessagesRequest {
    pub session_id: Nanoid<16, Base62Alphabet>,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ListMessagesResponse {
    pub data: Vec<ChatMessage>,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ListSessionsRequest {}
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ListSessionsResponse {
    pub data: Vec<ChatSession>,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SendMessageRequest {
    pub session_id: Nanoid<16, Base62Alphabet>,
    pub content: String,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SendMessageResponse {
    pub sent_at: i64,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SubscribeEventsRequest {
    pub session_id: Nanoid<16, Base62Alphabet>,
    #[serde(default)]
    pub unsub: Option<bool>,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SubscribeEventsResponse {
    pub data: Vec<ChatMessage>,
}

impl WsRequest for InitRequest {
    type Response = InitResponse;
    const METHOD_ID: u32 = 1;
    const ROLES: &[u32] = &[3];
    const SCHEMA: &'static str = r#"{
  "name": "Init",
  "code": 1,
  "parameters": [
    {
      "name": "access_token",
      "ty": "String"
    }
  ],
  "returns": [
    {
      "name": "user_id",
      "ty": {
        "NanoId": {
          "len": 16
        }
      }
    },
    {
      "name": "role",
      "ty": {
        "EnumRef": {
          "name": "UserRole"
        }
      }
    },
    {
      "name": "version",
      "ty": "String"
    }
  ],
  "stream_response": null,
  "description": "",
  "json_schema": null,
  "roles": [
    "UserRole::User"
  ]
}"#;
}
impl WsResponse for InitResponse {
    type Request = InitRequest;
}

impl WsRequest for AppConnectRequest {
    type Response = AppConnectResponse;
    const METHOD_ID: u32 = 100;
    const ROLES: &[u32] = &[0];
    const SCHEMA: &'static str = r#"{
  "name": "AppConnect",
  "code": 100,
  "parameters": [
    {
      "name": "app_public_id",
      "ty": {
        "NanoId": {
          "len": 16
        }
      }
    },
    {
      "name": "user_public_id",
      "ty": {
        "NanoId": {
          "len": 16
        }
      }
    }
  ],
  "returns": [
    {
      "name": "app_public_id",
      "ty": {
        "NanoId": {
          "len": 16
        }
      }
    },
    {
      "name": "app_name",
      "ty": {
        "Optional": "String"
      }
    }
  ],
  "stream_response": null,
  "description": "",
  "json_schema": null,
  "roles": [
    "UserRole::Public"
  ]
}"#;
}
impl WsResponse for AppConnectResponse {
    type Request = AppConnectRequest;
}

impl WsRequest for CreateSessionRequest {
    type Response = CreateSessionResponse;
    const METHOD_ID: u32 = 110;
    const ROLES: &[u32] = &[2];
    const SCHEMA: &'static str = r#"{
  "name": "CreateSession",
  "code": 110,
  "parameters": [],
  "returns": [
    {
      "name": "session_id",
      "ty": {
        "NanoId": {
          "len": 16
        }
      }
    },
    {
      "name": "created_at",
      "ty": "TimeStampMs"
    }
  ],
  "stream_response": null,
  "description": "",
  "json_schema": null,
  "roles": [
    "UserRole::App"
  ]
}"#;
}
impl WsResponse for CreateSessionResponse {
    type Request = CreateSessionRequest;
}

impl WsRequest for SendMessageRequest {
    type Response = SendMessageResponse;
    const METHOD_ID: u32 = 111;
    const ROLES: &[u32] = &[2];
    const SCHEMA: &'static str = r#"{
  "name": "SendMessage",
  "code": 111,
  "parameters": [
    {
      "name": "session_id",
      "ty": {
        "NanoId": {
          "len": 16
        }
      }
    },
    {
      "name": "content",
      "ty": "String"
    }
  ],
  "returns": [
    {
      "name": "sent_at",
      "ty": "TimeStampMs"
    }
  ],
  "stream_response": null,
  "description": "",
  "json_schema": null,
  "roles": [
    "UserRole::App"
  ]
}"#;
}
impl WsResponse for SendMessageResponse {
    type Request = SendMessageRequest;
}

impl WsRequest for ListMessagesRequest {
    type Response = ListMessagesResponse;
    const METHOD_ID: u32 = 112;
    const ROLES: &[u32] = &[2];
    const SCHEMA: &'static str = r#"{
  "name": "ListMessages",
  "code": 112,
  "parameters": [
    {
      "name": "session_id",
      "ty": {
        "NanoId": {
          "len": 16
        }
      }
    }
  ],
  "returns": [
    {
      "name": "data",
      "ty": {
        "StructTable": {
          "struct_ref": "ChatMessage"
        }
      }
    }
  ],
  "stream_response": null,
  "description": "",
  "json_schema": null,
  "roles": [
    "UserRole::App"
  ]
}"#;
}
impl WsResponse for ListMessagesResponse {
    type Request = ListMessagesRequest;
}

impl WsRequest for SubscribeEventsRequest {
    type Response = SubscribeEventsResponse;
    const METHOD_ID: u32 = 113;
    const ROLES: &[u32] = &[2];
    const SCHEMA: &'static str = r#"{
  "name": "SubscribeEvents",
  "code": 113,
  "parameters": [
    {
      "name": "session_id",
      "ty": {
        "NanoId": {
          "len": 16
        }
      }
    },
    {
      "name": "unsub",
      "ty": {
        "Optional": "Boolean"
      }
    }
  ],
  "returns": [
    {
      "name": "data",
      "ty": {
        "StructTable": {
          "struct_ref": "ChatMessage"
        }
      }
    }
  ],
  "stream_response": {
    "StructTable": {
      "struct_ref": "ChatMessage"
    }
  },
  "description": "",
  "json_schema": null,
  "roles": [
    "UserRole::App"
  ]
}"#;
}
impl WsResponse for SubscribeEventsResponse {
    type Request = SubscribeEventsRequest;
}

impl WsRequest for CloseSessionRequest {
    type Response = CloseSessionResponse;
    const METHOD_ID: u32 = 114;
    const ROLES: &[u32] = &[2];
    const SCHEMA: &'static str = r#"{
  "name": "CloseSession",
  "code": 114,
  "parameters": [
    {
      "name": "session_id",
      "ty": {
        "NanoId": {
          "len": 16
        }
      }
    }
  ],
  "returns": [],
  "stream_response": null,
  "description": "",
  "json_schema": null,
  "roles": [
    "UserRole::App"
  ]
}"#;
}
impl WsResponse for CloseSessionResponse {
    type Request = CloseSessionRequest;
}

impl WsRequest for ListSessionsRequest {
    type Response = ListSessionsResponse;
    const METHOD_ID: u32 = 115;
    const ROLES: &[u32] = &[2];
    const SCHEMA: &'static str = r#"{
  "name": "ListSessions",
  "code": 115,
  "parameters": [],
  "returns": [
    {
      "name": "data",
      "ty": {
        "StructTable": {
          "struct_ref": "ChatSession"
        }
      }
    }
  ],
  "stream_response": null,
  "description": "",
  "json_schema": null,
  "roles": [
    "UserRole::App"
  ]
}"#;
}
impl WsResponse for ListSessionsResponse {
    type Request = ListSessionsRequest;
}
