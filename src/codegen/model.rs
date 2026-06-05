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
pub enum AppMemberRole {
    /// App owner
    Owner = 0,
    /// App admin
    Admin = 1,
    /// App support member
    Support = 2,
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
    /// App admin authenticated via honey.id Init
    AppAdmin = 4,
    /// Support user authenticated via honey.id Init
    Support = 5,
    /// honey.id callback endpoints
    HoneyAuth = 6,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    pub app_public_id: Nanoid<16, Base62Alphabet>,
    pub tg_bot_token: String,
    #[serde(default)]
    pub app_name: Option<String>,
    pub active: bool,
    pub message_persistence_enabled: bool,
    pub created_at: i64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub public_id: Nanoid<16, Base62Alphabet>,
    #[serde(default)]
    pub app_name: Option<String>,
    pub active: bool,
    pub created_at: i64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AppMember {
    pub app_public_id: Nanoid<16, Base62Alphabet>,
    pub user_pub_id: Nanoid<16, Base62Alphabet>,
    pub role: AppMemberRole,
    pub created_at: i64,
    pub is_support_enabled: bool,
    #[serde(default)]
    pub tg_handle: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessage {
    pub session_id: Nanoid<16, Base62Alphabet>,
    pub incoming: bool,
    pub sent_by: String,
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

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SupportInfo {
    pub user_pub_id: Nanoid<16, Base62Alphabet>,
    pub tg_handle: String,
    #[serde(default)]
    pub chat_id: Option<i64>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UserInfo {
    pub id: i64,
    pub pub_id: Nanoid<16, Base62Alphabet>,
    pub username: String,
    pub role: UserRole,
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
    Init = 10000,
    ///
    AppConnect = 20000,
    ///
    CreateChatSession = 20001,
    ///
    SendMessage = 20002,
    ///
    ListMessages = 20003,
    ///
    SubscribeEvents = 20004,
    ///
    CloseChatSession = 20005,
    ///
    ListChatSessions = 20006,
    ///
    SetMyTgHandle = 20007,
    ///
    GetMyTgHandle = 20008,
    ///
    CreateApp = 30000,
    ///
    EditApp = 30001,
    ///
    ListApps = 30002,
    ///
    EnableSupportUser = 30003,
    ///
    DisableSupportUser = 30005,
    ///
    AddAppMember = 30006,
    ///
    SetAppMemberRole = 30007,
    ///
    ListAppMembers = 30008,
    ///
    EnableMessagePersistence = 30009,
    ///
    DisableMessagePersistence = 30010,
    ///
    DeleteApp = 40000,
    ///
    SetLogLevel = 40001,
    ///
    GetUsers = 40002,
    ///
    SetRole = 40003,
    ///
    GetAllApps = 40004,
}

impl EnumEndpoint {
    pub fn schema(&self) -> endpoint_libs::model::EndpointSchema {
        let schema = match self {
            Self::Init => InitRequest::SCHEMA,
            Self::AppConnect => AppConnectRequest::SCHEMA,
            Self::CreateChatSession => CreateChatSessionRequest::SCHEMA,
            Self::SendMessage => SendMessageRequest::SCHEMA,
            Self::ListMessages => ListMessagesRequest::SCHEMA,
            Self::SubscribeEvents => SubscribeEventsRequest::SCHEMA,
            Self::CloseChatSession => CloseChatSessionRequest::SCHEMA,
            Self::ListChatSessions => ListChatSessionsRequest::SCHEMA,
            Self::CreateApp => CreateAppRequest::SCHEMA,
            Self::EditApp => EditAppRequest::SCHEMA,
            Self::ListApps => ListAppsRequest::SCHEMA,
            Self::EnableSupportUser => EnableSupportUserRequest::SCHEMA,
            Self::DisableSupportUser => DisableSupportUserRequest::SCHEMA,
            Self::AddAppMember => AddAppMemberRequest::SCHEMA,
            Self::SetAppMemberRole => SetAppMemberRoleRequest::SCHEMA,
            Self::ListAppMembers => ListAppMembersRequest::SCHEMA,
            Self::EnableMessagePersistence => EnableMessagePersistenceRequest::SCHEMA,
            Self::DisableMessagePersistence => DisableMessagePersistenceRequest::SCHEMA,
            Self::DeleteApp => DeleteAppRequest::SCHEMA,
            Self::SetLogLevel => SetLogLevelRequest::SCHEMA,
            Self::GetUsers => GetUsersRequest::SCHEMA,
            Self::SetRole => SetRoleRequest::SCHEMA,
            Self::GetAllApps => GetAllAppsRequest::SCHEMA,
            Self::SetMyTgHandle => SetMyTgHandleRequest::SCHEMA,
            Self::GetMyTgHandle => GetMyTgHandleRequest::SCHEMA,
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
pub struct AddAppMemberRequest {
    pub app_public_id: Nanoid<16, Base62Alphabet>,
    pub user_pub_id: Nanoid<16, Base62Alphabet>,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AddAppMemberResponse {}
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
pub struct CloseChatSessionRequest {
    pub session_id: Nanoid<16, Base62Alphabet>,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CloseChatSessionResponse {}
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CreateAppRequest {
    pub tg_bot_token: String,
    #[serde(default)]
    pub app_name: Option<String>,
    #[serde(default)]
    pub message_persistence_enabled: Option<bool>,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CreateAppResponse {
    pub app_public_id: Nanoid<16, Base62Alphabet>,
    pub created_at: i64,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CreateChatSessionRequest {
    pub user_pub_id: Nanoid<16, Base62Alphabet>,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CreateChatSessionResponse {
    pub session_id: Nanoid<16, Base62Alphabet>,
    pub created_at: i64,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DeleteAppRequest {
    pub app_public_id: Nanoid<16, Base62Alphabet>,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DeleteAppResponse {}
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DisableMessagePersistenceRequest {
    pub app_public_id: Nanoid<16, Base62Alphabet>,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DisableMessagePersistenceResponse {}
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DisableSupportUserRequest {
    pub app_public_id: Nanoid<16, Base62Alphabet>,
    pub user_pub_id: Nanoid<16, Base62Alphabet>,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DisableSupportUserResponse {}
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct EditAppRequest {
    pub app_public_id: Nanoid<16, Base62Alphabet>,
    #[serde(default)]
    pub tg_bot_token: Option<String>,
    #[serde(default)]
    pub app_name: Option<String>,
    #[serde(default)]
    pub active: Option<bool>,
    #[serde(default)]
    pub message_persistence_enabled: Option<bool>,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct EditAppResponse {}
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct EnableMessagePersistenceRequest {
    pub app_public_id: Nanoid<16, Base62Alphabet>,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct EnableMessagePersistenceResponse {}
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct EnableSupportUserRequest {
    pub app_public_id: Nanoid<16, Base62Alphabet>,
    pub user_pub_id: Nanoid<16, Base62Alphabet>,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct EnableSupportUserResponse {}
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GetAllAppsRequest {}
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GetAllAppsResponse {
    pub data: Vec<AppInfo>,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GetMyTgHandleRequest {}
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GetMyTgHandleResponse {
    #[serde(default)]
    pub tg_handle: Option<String>,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GetUsersRequest {}
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GetUsersResponse {
    pub data: Vec<UserInfo>,
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
pub struct ListAppMembersRequest {
    pub app_public_id: Nanoid<16, Base62Alphabet>,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ListAppMembersResponse {
    pub data: Vec<AppMember>,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ListAppsRequest {}
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ListAppsResponse {
    pub data: Vec<AppConfig>,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ListChatSessionsRequest {}
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ListChatSessionsResponse {
    pub data: Vec<ChatSession>,
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
pub struct SetAppMemberRoleRequest {
    pub app_public_id: Nanoid<16, Base62Alphabet>,
    pub user_pub_id: Nanoid<16, Base62Alphabet>,
    pub role: AppMemberRole,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SetAppMemberRoleResponse {}
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SetLogLevelRequest {
    pub level: LogLevel,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SetLogLevelResponse {}
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SetMyTgHandleRequest {
    pub tg_handle: String,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SetMyTgHandleResponse {}
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SetRoleRequest {
    pub user_pub_id: Nanoid<16, Base62Alphabet>,
    pub role: UserRole,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SetRoleResponse {}
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
    const METHOD_ID: u32 = 10000;
    const ROLES: &[u32] = &[0];
    const SCHEMA: &'static str = r#"{
  "name": "Init",
  "code": 10000,
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
    "UserRole::Public"
  ]
}"#;
}
impl WsResponse for InitResponse {
    type Request = InitRequest;
}

impl WsRequest for AppConnectRequest {
    type Response = AppConnectResponse;
    const METHOD_ID: u32 = 20000;
    const ROLES: &[u32] = &[0];
    const SCHEMA: &'static str = r#"{
  "name": "AppConnect",
  "code": 20000,
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

impl WsRequest for CreateChatSessionRequest {
    type Response = CreateChatSessionResponse;
    const METHOD_ID: u32 = 20001;
    const ROLES: &[u32] = &[2];
    const SCHEMA: &'static str = r#"{
  "name": "CreateChatSession",
  "code": 20001,
  "parameters": [
    {
      "name": "user_pub_id",
      "ty": {
        "NanoId": {
          "len": 16
        }
      }
    }
  ],
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
impl WsResponse for CreateChatSessionResponse {
    type Request = CreateChatSessionRequest;
}

impl WsRequest for SendMessageRequest {
    type Response = SendMessageResponse;
    const METHOD_ID: u32 = 20002;
    const ROLES: &[u32] = &[2, 3, 4];
    const SCHEMA: &'static str = r#"{
  "name": "SendMessage",
  "code": 20002,
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
    "UserRole::App",
    "UserRole::User",
    "UserRole::AppAdmin"
  ]
}"#;
}
impl WsResponse for SendMessageResponse {
    type Request = SendMessageRequest;
}

impl WsRequest for ListMessagesRequest {
    type Response = ListMessagesResponse;
    const METHOD_ID: u32 = 20003;
    const ROLES: &[u32] = &[2, 3, 4];
    const SCHEMA: &'static str = r#"{
  "name": "ListMessages",
  "code": 20003,
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
    "UserRole::App",
    "UserRole::User",
    "UserRole::AppAdmin"
  ]
}"#;
}
impl WsResponse for ListMessagesResponse {
    type Request = ListMessagesRequest;
}

impl WsRequest for SubscribeEventsRequest {
    type Response = SubscribeEventsResponse;
    const METHOD_ID: u32 = 20004;
    const ROLES: &[u32] = &[2, 3, 4];
    const SCHEMA: &'static str = r#"{
  "name": "SubscribeEvents",
  "code": 20004,
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
    "UserRole::App",
    "UserRole::User",
    "UserRole::AppAdmin"
  ]
}"#;
}
impl WsResponse for SubscribeEventsResponse {
    type Request = SubscribeEventsRequest;
}

impl WsRequest for CloseChatSessionRequest {
    type Response = CloseChatSessionResponse;
    const METHOD_ID: u32 = 20005;
    const ROLES: &[u32] = &[2, 3, 4];
    const SCHEMA: &'static str = r#"{
  "name": "CloseChatSession",
  "code": 20005,
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
    "UserRole::App",
    "UserRole::User",
    "UserRole::AppAdmin"
  ]
}"#;
}
impl WsResponse for CloseChatSessionResponse {
    type Request = CloseChatSessionRequest;
}

impl WsRequest for ListChatSessionsRequest {
    type Response = ListChatSessionsResponse;
    const METHOD_ID: u32 = 20006;
    const ROLES: &[u32] = &[2, 3, 4];
    const SCHEMA: &'static str = r#"{
  "name": "ListChatSessions",
  "code": 20006,
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
    "UserRole::App",
    "UserRole::User",
    "UserRole::AppAdmin"
  ]
}"#;
}
impl WsResponse for ListChatSessionsResponse {
    type Request = ListChatSessionsRequest;
}

impl WsRequest for CreateAppRequest {
    type Response = CreateAppResponse;
    const METHOD_ID: u32 = 30000;
    const ROLES: &[u32] = &[1, 4];
    const SCHEMA: &'static str = r#"{
  "name": "CreateApp",
  "code": 30000,
  "parameters": [
    {
      "name": "tg_bot_token",
      "ty": "String"
    },
    {
      "name": "app_name",
      "ty": {
        "Optional": "String"
      }
    },
    {
      "name": "message_persistence_enabled",
      "ty": {
        "Optional": "Boolean"
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
      "name": "created_at",
      "ty": "TimeStampMs"
    }
  ],
  "stream_response": null,
  "description": "",
  "json_schema": null,
  "roles": [
    "UserRole::Admin",
    "UserRole::AppAdmin"
  ]
}"#;
}
impl WsResponse for CreateAppResponse {
    type Request = CreateAppRequest;
}

impl WsRequest for EditAppRequest {
    type Response = EditAppResponse;
    const METHOD_ID: u32 = 30001;
    const ROLES: &[u32] = &[1, 4];
    const SCHEMA: &'static str = r#"{
  "name": "EditApp",
  "code": 30001,
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
      "name": "tg_bot_token",
      "ty": {
        "Optional": "String"
      }
    },
    {
      "name": "app_name",
      "ty": {
        "Optional": "String"
      }
    },
    {
      "name": "active",
      "ty": {
        "Optional": "Boolean"
      }
    },
    {
      "name": "message_persistence_enabled",
      "ty": {
        "Optional": "Boolean"
      }
    }
  ],
  "returns": [],
  "stream_response": null,
  "description": "",
  "json_schema": null,
  "roles": [
    "UserRole::Admin",
    "UserRole::AppAdmin"
  ]
}"#;
}
impl WsResponse for EditAppResponse {
    type Request = EditAppRequest;
}

impl WsRequest for ListAppsRequest {
    type Response = ListAppsResponse;
    const METHOD_ID: u32 = 30002;
    const ROLES: &[u32] = &[1, 4];
    const SCHEMA: &'static str = r#"{
  "name": "ListApps",
  "code": 30002,
  "parameters": [],
  "returns": [
    {
      "name": "data",
      "ty": {
        "StructTable": {
          "struct_ref": "AppConfig"
        }
      }
    }
  ],
  "stream_response": null,
  "description": "",
  "json_schema": null,
  "roles": [
    "UserRole::Admin",
    "UserRole::AppAdmin"
  ]
}"#;
}
impl WsResponse for ListAppsResponse {
    type Request = ListAppsRequest;
}

impl WsRequest for EnableSupportUserRequest {
    type Response = EnableSupportUserResponse;
    const METHOD_ID: u32 = 30003;
    const ROLES: &[u32] = &[1, 4];
    const SCHEMA: &'static str = r#"{
  "name": "EnableSupportUser",
  "code": 30003,
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
      "name": "user_pub_id",
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
    "UserRole::Admin",
    "UserRole::AppAdmin"
  ]
}"#;
}
impl WsResponse for EnableSupportUserResponse {
    type Request = EnableSupportUserRequest;
}

impl WsRequest for DisableSupportUserRequest {
    type Response = DisableSupportUserResponse;
    const METHOD_ID: u32 = 30005;
    const ROLES: &[u32] = &[1, 4];
    const SCHEMA: &'static str = r#"{
  "name": "DisableSupportUser",
  "code": 30005,
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
      "name": "user_pub_id",
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
    "UserRole::Admin",
    "UserRole::AppAdmin"
  ]
}"#;
}
impl WsResponse for DisableSupportUserResponse {
    type Request = DisableSupportUserRequest;
}

impl WsRequest for AddAppMemberRequest {
    type Response = AddAppMemberResponse;
    const METHOD_ID: u32 = 30006;
    const ROLES: &[u32] = &[1, 4];
    const SCHEMA: &'static str = r#"{
  "name": "AddAppMember",
  "code": 30006,
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
      "name": "user_pub_id",
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
    "UserRole::Admin",
    "UserRole::AppAdmin"
  ]
}"#;
}
impl WsResponse for AddAppMemberResponse {
    type Request = AddAppMemberRequest;
}

impl WsRequest for SetAppMemberRoleRequest {
    type Response = SetAppMemberRoleResponse;
    const METHOD_ID: u32 = 30007;
    const ROLES: &[u32] = &[1, 4];
    const SCHEMA: &'static str = r#"{
  "name": "SetAppMemberRole",
  "code": 30007,
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
      "name": "user_pub_id",
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
          "name": "AppMemberRole"
        }
      }
    }
  ],
  "returns": [],
  "stream_response": null,
  "description": "",
  "json_schema": null,
  "roles": [
    "UserRole::Admin",
    "UserRole::AppAdmin"
  ]
}"#;
}
impl WsResponse for SetAppMemberRoleResponse {
    type Request = SetAppMemberRoleRequest;
}

impl WsRequest for ListAppMembersRequest {
    type Response = ListAppMembersResponse;
    const METHOD_ID: u32 = 30008;
    const ROLES: &[u32] = &[1, 4, 5];
    const SCHEMA: &'static str = r#"{
  "name": "ListAppMembers",
  "code": 30008,
  "parameters": [
    {
      "name": "app_public_id",
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
          "struct_ref": "AppMember"
        }
      }
    }
  ],
  "stream_response": null,
  "description": "",
  "json_schema": null,
  "roles": [
    "UserRole::Admin",
    "UserRole::AppAdmin",
    "UserRole::Support"
  ]
}"#;
}
impl WsResponse for ListAppMembersResponse {
    type Request = ListAppMembersRequest;
}

impl WsRequest for EnableMessagePersistenceRequest {
    type Response = EnableMessagePersistenceResponse;
    const METHOD_ID: u32 = 30009;
    const ROLES: &[u32] = &[1, 4];
    const SCHEMA: &'static str = r#"{
  "name": "EnableMessagePersistence",
  "code": 30009,
  "parameters": [
    {
      "name": "app_public_id",
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
    "UserRole::Admin",
    "UserRole::AppAdmin"
  ]
}"#;
}
impl WsResponse for EnableMessagePersistenceResponse {
    type Request = EnableMessagePersistenceRequest;
}

impl WsRequest for DisableMessagePersistenceRequest {
    type Response = DisableMessagePersistenceResponse;
    const METHOD_ID: u32 = 30010;
    const ROLES: &[u32] = &[1, 4];
    const SCHEMA: &'static str = r#"{
  "name": "DisableMessagePersistence",
  "code": 30010,
  "parameters": [
    {
      "name": "app_public_id",
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
    "UserRole::Admin",
    "UserRole::AppAdmin"
  ]
}"#;
}
impl WsResponse for DisableMessagePersistenceResponse {
    type Request = DisableMessagePersistenceRequest;
}

impl WsRequest for DeleteAppRequest {
    type Response = DeleteAppResponse;
    const METHOD_ID: u32 = 40000;
    const ROLES: &[u32] = &[1, 4];
    const SCHEMA: &'static str = r#"{
  "name": "DeleteApp",
  "code": 40000,
  "parameters": [
    {
      "name": "app_public_id",
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
    "UserRole::Admin",
    "UserRole::AppAdmin"
  ]
}"#;
}
impl WsResponse for DeleteAppResponse {
    type Request = DeleteAppRequest;
}

impl WsRequest for SetLogLevelRequest {
    type Response = SetLogLevelResponse;
    const METHOD_ID: u32 = 40001;
    const ROLES: &[u32] = &[1];
    const SCHEMA: &'static str = r#"{
  "name": "SetLogLevel",
  "code": 40001,
  "parameters": [
    {
      "name": "level",
      "ty": {
        "EnumRef": {
          "name": "LogLevel"
        }
      }
    }
  ],
  "returns": [],
  "stream_response": null,
  "description": "",
  "json_schema": null,
  "roles": [
    "UserRole::Admin"
  ]
}"#;
}
impl WsResponse for SetLogLevelResponse {
    type Request = SetLogLevelRequest;
}

impl WsRequest for GetUsersRequest {
    type Response = GetUsersResponse;
    const METHOD_ID: u32 = 40002;
    const ROLES: &[u32] = &[1];
    const SCHEMA: &'static str = r#"{
  "name": "GetUsers",
  "code": 40002,
  "parameters": [],
  "returns": [
    {
      "name": "data",
      "ty": {
        "StructTable": {
          "struct_ref": "UserInfo"
        }
      }
    }
  ],
  "stream_response": null,
  "description": "",
  "json_schema": null,
  "roles": [
    "UserRole::Admin"
  ]
}"#;
}
impl WsResponse for GetUsersResponse {
    type Request = GetUsersRequest;
}

impl WsRequest for SetRoleRequest {
    type Response = SetRoleResponse;
    const METHOD_ID: u32 = 40003;
    const ROLES: &[u32] = &[1];
    const SCHEMA: &'static str = r#"{
  "name": "SetRole",
  "code": 40003,
  "parameters": [
    {
      "name": "user_pub_id",
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
    }
  ],
  "returns": [],
  "stream_response": null,
  "description": "",
  "json_schema": null,
  "roles": [
    "UserRole::Admin"
  ]
}"#;
}
impl WsResponse for SetRoleResponse {
    type Request = SetRoleRequest;
}

impl WsRequest for GetAllAppsRequest {
    type Response = GetAllAppsResponse;
    const METHOD_ID: u32 = 40004;
    const ROLES: &[u32] = &[1];
    const SCHEMA: &'static str = r#"{
  "name": "GetAllApps",
  "code": 40004,
  "parameters": [],
  "returns": [
    {
      "name": "data",
      "ty": {
        "StructTable": {
          "struct_ref": "AppInfo"
        }
      }
    }
  ],
  "stream_response": null,
  "description": "",
  "json_schema": null,
  "roles": [
    "UserRole::Admin"
  ]
}"#;
}
impl WsResponse for GetAllAppsResponse {
    type Request = GetAllAppsRequest;
}

impl WsRequest for SetMyTgHandleRequest {
    type Response = SetMyTgHandleResponse;
    const METHOD_ID: u32 = 20007;
    const ROLES: &[u32] = &[1, 4, 5];
    const SCHEMA: &'static str = r#"{
  "name": "SetMyTgHandle",
  "code": 20007,
  "parameters": [
    {
      "name": "tg_handle",
      "ty": "String"
    }
  ],
  "returns": [],
  "stream_response": null,
  "description": "",
  "json_schema": null,
  "roles": [
    "UserRole::Admin",
    "UserRole::AppAdmin",
    "UserRole::Support"
  ]
}"#;
}
impl WsResponse for SetMyTgHandleResponse {
    type Request = SetMyTgHandleRequest;
}

impl WsRequest for GetMyTgHandleRequest {
    type Response = GetMyTgHandleResponse;
    const METHOD_ID: u32 = 20008;
    const ROLES: &[u32] = &[1, 4, 5];
    const SCHEMA: &'static str = r#"{
  "name": "GetMyTgHandle",
  "code": 20008,
  "parameters": [],
  "returns": [
    {
      "name": "tg_handle",
      "ty": {
        "Optional": "String"
      }
    }
  ],
  "stream_response": null,
  "description": "",
  "json_schema": null,
  "roles": [
    "UserRole::Admin",
    "UserRole::AppAdmin",
    "UserRole::Support"
  ]
}"#;
}
impl WsResponse for GetMyTgHandleResponse {
    type Request = GetMyTgHandleRequest;
}
