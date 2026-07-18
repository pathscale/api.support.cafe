
# API Reference

## Structs/Datamodels

```rust
struct AppConfig{ appPublicId: Nanoid<16, Base62Alphabet>, tgBotToken: String, appName: Option<String>, active: bool, messagePersistenceEnabled: bool, createdAt: i64 }


struct AppInfo{ publicId: Nanoid<16, Base62Alphabet>, appName: Option<String>, active: bool, createdAt: i64 }


struct AppMember{ appPublicId: Nanoid<16, Base62Alphabet>, userPubId: Nanoid<16, Base62Alphabet>, username: String, role: AppMemberRole, createdAt: i64, isSupportEnabled: bool, tgHandle: Option<String> }


struct ChatMessage{ sessionId: Nanoid<16, Base62Alphabet>, incoming: bool, sentBy: String, sentAt: i64, content: String }


struct ChatSession{ sessionId: Nanoid<16, Base62Alphabet>, appPublicId: Nanoid<16, Base62Alphabet>, userPubId: Nanoid<16, Base62Alphabet>, createdAt: i64, closedAt: Option<i64> }


struct SupportInfo{ userPubId: Nanoid<16, Base62Alphabet>, tgHandle: String, chatId: Option<i64> }


struct UserInfo{ id: i64, pubId: Nanoid<16, Base62Alphabet>, username: String, role: UserRole }

```
---

## Enums

```rust
enum AppMemberRole { Owner, Admin, Support }


enum LogLevel { Trace, Debug, Info, Warn, Error }


enum UserRole { Public, Admin, App, User, AppAdmin, Support, HoneyAuth }


enum ErrorCode { BadRequest, Unauthorized, PaymentRequired, Forbidden, NotFound, MethodNotAllowed, NotAcceptable, ProxyAuthenticationRequired, RequestTimeout, Conflict, Gone, LengthRequired, PreconditionFailed, PayloadTooLarge, UriTooLong, UnsupportedMediaType, RangeNotSatisfiable, ExpectationFailed, ImATeapot, MisdirectedRequest, UnprocessableEntity, Locked, FailedDependency, UpgradeRequired, PreconditionRequired, TooManyRequests, RequestHeaderFieldsTooLarge, UnavailableForLegalReasons, InternalError, NotImplemented, BadGateway, ServiceUnavailable, GatewayTimeout, HttpVersionNotSupported, VariantAlsoNegotiates, InsufficientStorage, LoopDetected, NotExtended, NetworkAuthenticationRequired }

```
---

        

## authApi Server
ID: 1
### Endpoints
|Code|Name|Parameters|Response|Description|FE Facing|Errors|
|-----------|-----------|----------|--------|-----------|-----------|-----------|
|10000|Init|`accessToken: String`|`userId: Nanoid<16, Base62Alphabet>`, `role: UserRole`, `version: String`|Authenticate this connection with a honey.id access token. Runs at WebSocket handshake time via the Sec-WebSocket-Protocol header; not callable as a tool.|true||

## appApi Server
ID: 2
### Endpoints
|Code|Name|Parameters|Response|Description|FE Facing|Errors|
|-----------|-----------|----------|--------|-----------|-----------|-----------|
|20001|CreateChatSession|`userPubId: Nanoid<16, Base62Alphabet>`|`sessionId: Nanoid<16, Base62Alphabet>`, `createdAt: i64`|Create a new support chat session for the given end-user of this app. Returns the 16-character session_id used by all subsequent message operations. Caller must be an App connection.|true||
|20002|SendMessage|`sessionId: Nanoid<16, Base62Alphabet>`, `content: String`|`sentAt: i64`|Send a message into an existing chat session. The message is stored and relayed to the app's support staff via Telegram. Support staff reply from Telegram, not via this endpoint.|true||
|20003|ListMessages|`sessionId: Nanoid<16, Base62Alphabet>`|`data: Vec<ChatMessage>`|List all messages of a chat session, oldest first.|true||
|20004|SubscribeEvents|`sessionId: Nanoid<16, Base62Alphabet>`, `unsub: Option<bool>`|`data: Vec<ChatMessage>`|Subscribe to live chat events (new messages) for a session; pass unsub: true to unsubscribe. Events are delivered as stream frames over the legacy protocol only.|true||
|20005|CloseChatSession|`sessionId: Nanoid<16, Base62Alphabet>`||Close a chat session; no further messages can be sent to it.|true||
|20006|ListChatSessions||`data: Vec<ChatSession>`|List chat sessions visible to the caller (the app's sessions for App connections, the user's own sessions otherwise).|true||

## appConnect Server
ID: 2
### Endpoints
|Code|Name|Parameters|Response|Description|FE Facing|Errors|
|-----------|-----------|----------|--------|-----------|-----------|-----------|
|20000|AppConnect|`appPublicId: Nanoid<16, Base62Alphabet>`, `userPublicId: Nanoid<16, Base62Alphabet>`|`appPublicId: Nanoid<16, Base62Alphabet>`, `appName: Option<String>`|Connect as an app widget on behalf of an end-user, declaring the app and user public ids. Runs at WebSocket handshake time via the Sec-WebSocket-Protocol header; not callable as a tool.|true||

## appAdminApi Server
ID: 3
### Endpoints
|Code|Name|Parameters|Response|Description|FE Facing|Errors|
|-----------|-----------|----------|--------|-----------|-----------|-----------|
|30000|CreateApp|`tgBotToken: String`, `appName: Option<String>`, `messagePersistenceEnabled: Option<bool>`|`appPublicId: Nanoid<16, Base62Alphabet>`, `createdAt: i64`|Register a new tenant app with its Telegram bot token. The caller becomes the app Owner and the bot is registered and started.|true||
|30001|EditApp|`appPublicId: Nanoid<16, Base62Alphabet>`, `tgBotToken: Option<String>`, `appName: Option<String>`, `active: Option<bool>`, `messagePersistenceEnabled: Option<bool>`||Update an app's name, Telegram bot token, active flag, or message persistence. Changing the token or active flag restarts or stops the app's bot.|true||
|30002|ListApps||`data: Vec<AppConfig>`|List apps the caller is a member of. The response includes each app's Telegram bot token — treat it as a secret.|true||
|30003|EnableSupportUser|`appPublicId: Nanoid<16, Base62Alphabet>`, `userPubId: Nanoid<16, Base62Alphabet>`||Enable an app member to receive support messages in Telegram.|true||
|30005|DisableSupportUser|`appPublicId: Nanoid<16, Base62Alphabet>`, `userPubId: Nanoid<16, Base62Alphabet>`||Stop an app member from receiving support messages in Telegram.|true||
|30006|AddAppMember|`appPublicId: Nanoid<16, Base62Alphabet>`, `userPubId: Nanoid<16, Base62Alphabet>`||Add a user to an app as a Support member. Use SetAppMemberRole to change their role afterwards.|true||
|30007|SetAppMemberRole|`appPublicId: Nanoid<16, Base62Alphabet>`, `userPubId: Nanoid<16, Base62Alphabet>`, `role: AppMemberRole`||Change an app member's role (Owner, Admin, or Support). Only the app Owner may call this.|true||
|30008|ListAppMembers|`appPublicId: Nanoid<16, Base62Alphabet>`|`data: Vec<AppMember>`|List members of an app with their roles and support-enabled status.|true||
|30009|EnableMessagePersistence|`appPublicId: Nanoid<16, Base62Alphabet>`||Enable disk persistence for the app's chat messages; existing in-memory messages are migrated to disk.|true||
|30010|DisableMessagePersistence|`appPublicId: Nanoid<16, Base62Alphabet>`||Disable disk persistence for the app's chat messages; existing messages are migrated to the in-memory store (purged after 24h).|true||

## adminApi Server
ID: 4
### Endpoints
|Code|Name|Parameters|Response|Description|FE Facing|Errors|
|-----------|-----------|----------|--------|-----------|-----------|-----------|
|40000|DeleteApp|`appPublicId: Nanoid<16, Base62Alphabet>`||Delete an app, its memberships, and stop its Telegram bot. Only the app Owner may call this. Existing sessions and messages are not cascaded.|true||
|40001|SetLogLevel|`level: LogLevel`||Change the server's log level at runtime (platform admin only).|true||
|40002|GetUsers||`data: Vec<UserInfo>`|List all registered users with their platform roles (platform admin only).|true||
|40003|SetRole|`userPubId: Nanoid<16, Base62Alphabet>`, `role: UserRole`||Override a user's platform role (platform admin only).|true||
|40004|GetAllApps||`data: Vec<AppInfo>`|List all apps on the platform (platform admin only).|true||

## supportApi Server
ID: 5
### Endpoints
|Code|Name|Parameters|Response|Description|FE Facing|Errors|
|-----------|-----------|----------|--------|-----------|-----------|-----------|
|20007|SetMyTgHandle|`tgHandle: String`||Set the caller's Telegram handle, used to route support replies. The handle is bound to a Telegram chat when the member sends /start to the app's bot.|true||
|20008|GetMyTgHandle||`tgHandle: Option<String>`|Get the caller's currently configured Telegram handle, if any.|true||

## userApi Server
ID: 6
### Endpoints
|Code|Name|Parameters|Response|Description|FE Facing|Errors|
|-----------|-----------|----------|--------|-----------|-----------|-----------|
|60000|GetMyInfo||`pubId: Nanoid<16, Base62Alphabet>`, `username: String`, `role: UserRole`|Return the caller's public id, username, and platform role.|true||
