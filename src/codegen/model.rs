use endpoint_libs::libs::error_code::ErrorCode;
use endpoint_libs::libs::types::*;
use endpoint_libs::libs::ws::toolbox::CustomError;
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
    pub username: String,
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
    ///
    GetMyInfo = 60000,
}

impl EnumEndpoint {
    pub fn schema(&self) -> endpoint_libs::model::EndpointSchema {
        let schema = match self {
            Self::Init => InitRequest::SCHEMA,
            Self::CreateChatSession => CreateChatSessionRequest::SCHEMA,
            Self::SendMessage => SendMessageRequest::SCHEMA,
            Self::ListMessages => ListMessagesRequest::SCHEMA,
            Self::SubscribeEvents => SubscribeEventsRequest::SCHEMA,
            Self::CloseChatSession => CloseChatSessionRequest::SCHEMA,
            Self::ListChatSessions => ListChatSessionsRequest::SCHEMA,
            Self::AppConnect => AppConnectRequest::SCHEMA,
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
            Self::GetMyInfo => GetMyInfoRequest::SCHEMA,
        };
        serde_json::from_str(schema).unwrap()
    }
}

/// JSON-serialized shared struct/enum definitions referenced by endpoint schemas.
pub const TYPE_DEFINITIONS: &'static str = r#"[
  {
    "Struct": {
      "name": "AppConfig",
      "fields": [
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
          "ty": "String"
        },
        {
          "name": "app_name",
          "ty": {
            "Optional": "String"
          }
        },
        {
          "name": "active",
          "ty": "Boolean"
        },
        {
          "name": "message_persistence_enabled",
          "ty": "Boolean"
        },
        {
          "name": "created_at",
          "ty": "TimeStampMs"
        }
      ]
    }
  },
  {
    "Struct": {
      "name": "AppInfo",
      "fields": [
        {
          "name": "public_id",
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
        },
        {
          "name": "active",
          "ty": "Boolean"
        },
        {
          "name": "created_at",
          "ty": "TimeStampMs"
        }
      ]
    }
  },
  {
    "Struct": {
      "name": "AppMember",
      "fields": [
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
          "name": "username",
          "ty": "String"
        },
        {
          "name": "role",
          "ty": {
            "EnumRef": {
              "name": "AppMemberRole"
            }
          }
        },
        {
          "name": "created_at",
          "ty": "TimeStampMs"
        },
        {
          "name": "is_support_enabled",
          "ty": "Boolean"
        },
        {
          "name": "tg_handle",
          "ty": {
            "Optional": "String"
          }
        }
      ]
    }
  },
  {
    "Struct": {
      "name": "ChatMessage",
      "fields": [
        {
          "name": "session_id",
          "ty": {
            "NanoId": {
              "len": 16
            }
          }
        },
        {
          "name": "incoming",
          "ty": "Boolean"
        },
        {
          "name": "sent_by",
          "ty": "String"
        },
        {
          "name": "sent_at",
          "ty": "TimeStampMs"
        },
        {
          "name": "content",
          "ty": "String"
        }
      ]
    }
  },
  {
    "Struct": {
      "name": "ChatSession",
      "fields": [
        {
          "name": "session_id",
          "ty": {
            "NanoId": {
              "len": 16
            }
          }
        },
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
          "name": "created_at",
          "ty": "TimeStampMs"
        },
        {
          "name": "closed_at",
          "ty": {
            "Optional": "TimeStampMs"
          }
        }
      ]
    }
  },
  {
    "Struct": {
      "name": "SupportInfo",
      "fields": [
        {
          "name": "user_pub_id",
          "ty": {
            "NanoId": {
              "len": 16
            }
          }
        },
        {
          "name": "tg_handle",
          "ty": "String"
        },
        {
          "name": "chat_id",
          "ty": {
            "Optional": "Int64"
          }
        }
      ]
    }
  },
  {
    "Struct": {
      "name": "UserInfo",
      "fields": [
        {
          "name": "id",
          "ty": "Int64"
        },
        {
          "name": "pub_id",
          "ty": {
            "NanoId": {
              "len": 16
            }
          }
        },
        {
          "name": "username",
          "ty": "String"
        },
        {
          "name": "role",
          "ty": {
            "EnumRef": {
              "name": "UserRole"
            }
          }
        }
      ]
    }
  },
  {
    "Enum": {
      "name": "AppMemberRole",
      "variants": [
        {
          "name": "Owner",
          "description": "App owner",
          "value": 0
        },
        {
          "name": "Admin",
          "description": "App admin",
          "value": 1
        },
        {
          "name": "Support",
          "description": "App support member",
          "value": 2
        }
      ]
    }
  },
  {
    "Enum": {
      "name": "LogLevel",
      "variants": [
        {
          "name": "Trace",
          "description": "Trace-level logging",
          "value": 0
        },
        {
          "name": "Debug",
          "description": "Debug-level logging",
          "value": 1
        },
        {
          "name": "Info",
          "description": "Info-level logging",
          "value": 2
        },
        {
          "name": "Warn",
          "description": "Warn-level logging",
          "value": 3
        },
        {
          "name": "Error",
          "description": "Error-level logging",
          "value": 4
        }
      ]
    }
  },
  {
    "Enum": {
      "name": "UserRole",
      "variants": [
        {
          "name": "Public",
          "description": "Unauthenticated",
          "value": 0
        },
        {
          "name": "Admin",
          "description": "Platform admin",
          "value": 1
        },
        {
          "name": "App",
          "description": "App frontend connection",
          "value": 2
        },
        {
          "name": "User",
          "description": "User authenticated via honey.id token",
          "value": 3
        },
        {
          "name": "AppAdmin",
          "description": "App admin authenticated via honey.id Init",
          "value": 4
        },
        {
          "name": "Support",
          "description": "Support user authenticated via honey.id Init",
          "value": 5
        },
        {
          "name": "HoneyAuth",
          "description": "honey.id callback endpoints",
          "value": 6
        }
      ]
    }
  },
  {
    "Enum": {
      "name": "ErrorCode",
      "variants": [
        {
          "name": "BadRequest",
          "description": "Bad request",
          "value": 100400
        },
        {
          "name": "Unauthorized",
          "description": "Authentication is required",
          "value": 100401
        },
        {
          "name": "PaymentRequired",
          "description": "Payment is required",
          "value": 100402
        },
        {
          "name": "Forbidden",
          "description": "Access is forbidden",
          "value": 100403
        },
        {
          "name": "NotFound",
          "description": "Resource was not found",
          "value": 100404
        },
        {
          "name": "MethodNotAllowed",
          "description": "Method is not allowed",
          "value": 100405
        },
        {
          "name": "NotAcceptable",
          "description": "Response format is not acceptable",
          "value": 100406
        },
        {
          "name": "ProxyAuthenticationRequired",
          "description": "Proxy authentication is required",
          "value": 100407
        },
        {
          "name": "RequestTimeout",
          "description": "Request timed out",
          "value": 100408
        },
        {
          "name": "Conflict",
          "description": "Request conflicts with current state",
          "value": 100409
        },
        {
          "name": "Gone",
          "description": "Resource is gone",
          "value": 100410
        },
        {
          "name": "LengthRequired",
          "description": "Content length is required",
          "value": 100411
        },
        {
          "name": "PreconditionFailed",
          "description": "Precondition failed",
          "value": 100412
        },
        {
          "name": "PayloadTooLarge",
          "description": "Payload is too large",
          "value": 100413
        },
        {
          "name": "UriTooLong",
          "description": "URI is too long",
          "value": 100414
        },
        {
          "name": "UnsupportedMediaType",
          "description": "Media type is unsupported",
          "value": 100415
        },
        {
          "name": "RangeNotSatisfiable",
          "description": "Requested range cannot be satisfied",
          "value": 100416
        },
        {
          "name": "ExpectationFailed",
          "description": "Expectation failed",
          "value": 100417
        },
        {
          "name": "ImATeapot",
          "description": "I'm a teapot",
          "value": 100418
        },
        {
          "name": "MisdirectedRequest",
          "description": "Request was misdirected",
          "value": 100421
        },
        {
          "name": "UnprocessableEntity",
          "description": "Entity could not be processed",
          "value": 100422
        },
        {
          "name": "Locked",
          "description": "Resource is locked",
          "value": 100423
        },
        {
          "name": "FailedDependency",
          "description": "Dependency failed",
          "value": 100424
        },
        {
          "name": "UpgradeRequired",
          "description": "Request must be upgraded",
          "value": 100426
        },
        {
          "name": "PreconditionRequired",
          "description": "Precondition is required",
          "value": 100428
        },
        {
          "name": "TooManyRequests",
          "description": "Too many requests",
          "value": 100429
        },
        {
          "name": "RequestHeaderFieldsTooLarge",
          "description": "Request header fields are too large",
          "value": 100431
        },
        {
          "name": "UnavailableForLegalReasons",
          "description": "Unavailable for legal reasons",
          "value": 100451
        },
        {
          "name": "InternalError",
          "description": "Internal server error",
          "value": 100500
        },
        {
          "name": "NotImplemented",
          "description": "Endpoint is not implemented",
          "value": 100501
        },
        {
          "name": "BadGateway",
          "description": "Bad gateway",
          "value": 100502
        },
        {
          "name": "ServiceUnavailable",
          "description": "Service is unavailable",
          "value": 100503
        },
        {
          "name": "GatewayTimeout",
          "description": "Gateway timed out",
          "value": 100504
        },
        {
          "name": "HttpVersionNotSupported",
          "description": "HTTP version is not supported",
          "value": 100505
        },
        {
          "name": "VariantAlsoNegotiates",
          "description": "Content negotiation variant problem",
          "value": 100506
        },
        {
          "name": "InsufficientStorage",
          "description": "Insufficient storage",
          "value": 100507
        },
        {
          "name": "LoopDetected",
          "description": "Loop was detected",
          "value": 100508
        },
        {
          "name": "NotExtended",
          "description": "Request must be extended",
          "value": 100510
        },
        {
          "name": "NetworkAuthenticationRequired",
          "description": "Network authentication is required",
          "value": 100511
        }
      ]
    }
  }
]"#;

/// Builds the type registry over all shared definitions, for use with
/// `WebsocketServer::enable_mcp()`.
pub fn type_registry() -> endpoint_libs::model::TypeRegistry {
    let types: Vec<endpoint_libs::model::Type> =
        serde_json::from_str(TYPE_DEFINITIONS).expect("Invalid embedded type definitions");
    let mut registry = endpoint_libs::model::TypeRegistry::new();
    registry.add_all(types.iter());
    registry
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
pub enum EnumErrorCode {
    /// Bad request
    BadRequest = 100400,
    /// Authentication is required
    Unauthorized = 100401,
    /// Payment is required
    PaymentRequired = 100402,
    /// Access is forbidden
    Forbidden = 100403,
    /// Resource was not found
    NotFound = 100404,
    /// Method is not allowed
    MethodNotAllowed = 100405,
    /// Response format is not acceptable
    NotAcceptable = 100406,
    /// Proxy authentication is required
    ProxyAuthenticationRequired = 100407,
    /// Request timed out
    RequestTimeout = 100408,
    /// Request conflicts with current state
    Conflict = 100409,
    /// Resource is gone
    Gone = 100410,
    /// Content length is required
    LengthRequired = 100411,
    /// Precondition failed
    PreconditionFailed = 100412,
    /// Payload is too large
    PayloadTooLarge = 100413,
    /// URI is too long
    UriTooLong = 100414,
    /// Media type is unsupported
    UnsupportedMediaType = 100415,
    /// Requested range cannot be satisfied
    RangeNotSatisfiable = 100416,
    /// Expectation failed
    ExpectationFailed = 100417,
    /// I'm a teapot
    ImATeapot = 100418,
    /// Request was misdirected
    MisdirectedRequest = 100421,
    /// Entity could not be processed
    UnprocessableEntity = 100422,
    /// Resource is locked
    Locked = 100423,
    /// Dependency failed
    FailedDependency = 100424,
    /// Request must be upgraded
    UpgradeRequired = 100426,
    /// Precondition is required
    PreconditionRequired = 100428,
    /// Too many requests
    TooManyRequests = 100429,
    /// Request header fields are too large
    RequestHeaderFieldsTooLarge = 100431,
    /// Unavailable for legal reasons
    UnavailableForLegalReasons = 100451,
    /// Internal server error
    InternalError = 100500,
    /// Endpoint is not implemented
    NotImplemented = 100501,
    /// Bad gateway
    BadGateway = 100502,
    /// Service is unavailable
    ServiceUnavailable = 100503,
    /// Gateway timed out
    GatewayTimeout = 100504,
    /// HTTP version is not supported
    HttpVersionNotSupported = 100505,
    /// Content negotiation variant problem
    VariantAlsoNegotiates = 100506,
    /// Insufficient storage
    InsufficientStorage = 100507,
    /// Loop was detected
    LoopDetected = 100508,
    /// Request must be extended
    NotExtended = 100510,
    /// Network authentication is required
    NetworkAuthenticationRequired = 100511,
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
pub struct GetMyInfoRequest {}
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GetMyInfoResponse {
    pub pub_id: Nanoid<16, Base62Alphabet>,
    pub username: String,
    pub role: UserRole,
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
  "description": "Authenticate this connection with a honey.id access token. Runs at WebSocket handshake time via the Sec-WebSocket-Protocol header; not callable as a tool.",
  "json_schema": null,
  "roles": [
    "UserRole::Public"
  ],
  "errors": []
}"#;
}
impl WsResponse for InitResponse {
    type Request = InitRequest;
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
  "description": "Create a new support chat session for the given end-user of this app. Returns the 16-character session_id used by all subsequent message operations. Caller must be an App connection.",
  "json_schema": null,
  "roles": [
    "UserRole::App"
  ],
  "errors": []
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
  "description": "Send a message into an existing chat session. The message is stored and relayed to the app's support staff via Telegram. Support staff reply from Telegram, not via this endpoint.",
  "json_schema": null,
  "roles": [
    "UserRole::App",
    "UserRole::User",
    "UserRole::AppAdmin"
  ],
  "errors": []
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
  "description": "List all messages of a chat session, oldest first.",
  "json_schema": null,
  "roles": [
    "UserRole::App",
    "UserRole::User",
    "UserRole::AppAdmin"
  ],
  "errors": []
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
  "description": "Subscribe to live chat events (new messages) for a session; pass unsub: true to unsubscribe. Events are delivered as stream frames over the legacy protocol only.",
  "json_schema": null,
  "roles": [
    "UserRole::App",
    "UserRole::User",
    "UserRole::AppAdmin"
  ],
  "errors": []
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
  "description": "Close a chat session; no further messages can be sent to it.",
  "json_schema": null,
  "roles": [
    "UserRole::App",
    "UserRole::User",
    "UserRole::AppAdmin"
  ],
  "errors": []
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
  "description": "List chat sessions visible to the caller (the app's sessions for App connections, the user's own sessions otherwise).",
  "json_schema": null,
  "roles": [
    "UserRole::App",
    "UserRole::User",
    "UserRole::AppAdmin"
  ],
  "errors": []
}"#;
}
impl WsResponse for ListChatSessionsResponse {
    type Request = ListChatSessionsRequest;
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
  "description": "Connect as an app widget on behalf of an end-user, declaring the app and user public ids. Runs at WebSocket handshake time via the Sec-WebSocket-Protocol header; not callable as a tool.",
  "json_schema": null,
  "roles": [
    "UserRole::Public"
  ],
  "errors": []
}"#;
}
impl WsResponse for AppConnectResponse {
    type Request = AppConnectRequest;
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
  "description": "Register a new tenant app with its Telegram bot token. The caller becomes the app Owner and the bot is registered and started.",
  "json_schema": null,
  "roles": [
    "UserRole::Admin",
    "UserRole::AppAdmin"
  ],
  "errors": []
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
  "description": "Update an app's name, Telegram bot token, active flag, or message persistence. Changing the token or active flag restarts or stops the app's bot.",
  "json_schema": null,
  "roles": [
    "UserRole::Admin",
    "UserRole::AppAdmin"
  ],
  "errors": []
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
  "description": "List apps the caller is a member of. The response includes each app's Telegram bot token — treat it as a secret.",
  "json_schema": null,
  "roles": [
    "UserRole::Admin",
    "UserRole::AppAdmin"
  ],
  "errors": []
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
  "description": "Enable an app member to receive support messages in Telegram.",
  "json_schema": null,
  "roles": [
    "UserRole::Admin",
    "UserRole::AppAdmin"
  ],
  "errors": []
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
  "description": "Stop an app member from receiving support messages in Telegram.",
  "json_schema": null,
  "roles": [
    "UserRole::Admin",
    "UserRole::AppAdmin"
  ],
  "errors": []
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
  "description": "Add a user to an app as a Support member. Use SetAppMemberRole to change their role afterwards.",
  "json_schema": null,
  "roles": [
    "UserRole::Admin",
    "UserRole::AppAdmin"
  ],
  "errors": []
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
  "description": "Change an app member's role (Owner, Admin, or Support). Only the app Owner may call this.",
  "json_schema": null,
  "roles": [
    "UserRole::Admin",
    "UserRole::AppAdmin"
  ],
  "errors": []
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
  "description": "List members of an app with their roles and support-enabled status.",
  "json_schema": null,
  "roles": [
    "UserRole::Admin",
    "UserRole::AppAdmin",
    "UserRole::Support"
  ],
  "errors": []
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
  "description": "Enable disk persistence for the app's chat messages; existing in-memory messages are migrated to disk.",
  "json_schema": null,
  "roles": [
    "UserRole::Admin",
    "UserRole::AppAdmin"
  ],
  "errors": []
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
  "description": "Disable disk persistence for the app's chat messages; existing messages are migrated to the in-memory store (purged after 24h).",
  "json_schema": null,
  "roles": [
    "UserRole::Admin",
    "UserRole::AppAdmin"
  ],
  "errors": []
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
  "description": "Delete an app, its memberships, and stop its Telegram bot. Only the app Owner may call this. Existing sessions and messages are not cascaded.",
  "json_schema": null,
  "roles": [
    "UserRole::Admin",
    "UserRole::AppAdmin"
  ],
  "errors": []
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
  "description": "Change the server's log level at runtime (platform admin only).",
  "json_schema": null,
  "roles": [
    "UserRole::Admin"
  ],
  "errors": []
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
  "description": "List all registered users with their platform roles (platform admin only).",
  "json_schema": null,
  "roles": [
    "UserRole::Admin"
  ],
  "errors": []
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
  "description": "Override a user's platform role (platform admin only).",
  "json_schema": null,
  "roles": [
    "UserRole::Admin"
  ],
  "errors": []
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
  "description": "List all apps on the platform (platform admin only).",
  "json_schema": null,
  "roles": [
    "UserRole::Admin"
  ],
  "errors": []
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
  "description": "Set the caller's Telegram handle, used to route support replies. The handle is bound to a Telegram chat when the member sends /start to the app's bot.",
  "json_schema": null,
  "roles": [
    "UserRole::Admin",
    "UserRole::AppAdmin",
    "UserRole::Support"
  ],
  "errors": []
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
  "description": "Get the caller's currently configured Telegram handle, if any.",
  "json_schema": null,
  "roles": [
    "UserRole::Admin",
    "UserRole::AppAdmin",
    "UserRole::Support"
  ],
  "errors": []
}"#;
}
impl WsResponse for GetMyTgHandleResponse {
    type Request = GetMyTgHandleRequest;
}

impl WsRequest for GetMyInfoRequest {
    type Response = GetMyInfoResponse;
    const METHOD_ID: u32 = 60000;
    const ROLES: &[u32] = &[1, 3, 4, 5];
    const SCHEMA: &'static str = r#"{
  "name": "GetMyInfo",
  "code": 60000,
  "parameters": [],
  "returns": [
    {
      "name": "pub_id",
      "ty": {
        "NanoId": {
          "len": 16
        }
      }
    },
    {
      "name": "username",
      "ty": "String"
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
  "stream_response": null,
  "description": "Return the caller's public id, username, and platform role.",
  "json_schema": null,
  "roles": [
    "UserRole::User",
    "UserRole::Support",
    "UserRole::AppAdmin",
    "UserRole::Admin"
  ],
  "errors": []
}"#;
}
impl WsResponse for GetMyInfoResponse {
    type Request = GetMyInfoRequest;
}
