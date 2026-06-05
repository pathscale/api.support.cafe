
# API Reference

## Structs/Datamodels

```rust
struct AppConfig{ appPublicId: Nanoid<16, Base62Alphabet>, tgBotToken: String, appName: Option<String>, active: bool, messagePersistenceEnabled: bool, createdAt: i64 }


struct AppInfo{ publicId: Nanoid<16, Base62Alphabet>, appName: Option<String>, active: bool, createdAt: i64 }


struct AppMember{ appPublicId: Nanoid<16, Base62Alphabet>, userPubId: Nanoid<16, Base62Alphabet>, role: AppMemberRole, createdAt: i64, isSupportEnabled: bool, tgHandle: Option<String> }


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

```
---

        

## authApi Server
ID: 1
### Endpoints
|Code|Name|Parameters|Response|Description|FE Facing|
|-----------|-----------|----------|--------|-----------|-----------|
|10000|Init|`accessToken: String`|`userId: Nanoid<16, Base62Alphabet>`, `role: UserRole`, `version: String`||true|

## appConnect Server
ID: 2
### Endpoints
|Code|Name|Parameters|Response|Description|FE Facing|
|-----------|-----------|----------|--------|-----------|-----------|
|20000|AppConnect|`appPublicId: Nanoid<16, Base62Alphabet>`, `userPublicId: Nanoid<16, Base62Alphabet>`|`appPublicId: Nanoid<16, Base62Alphabet>`, `appName: Option<String>`||true|

## appApi Server
ID: 2
### Endpoints
|Code|Name|Parameters|Response|Description|FE Facing|
|-----------|-----------|----------|--------|-----------|-----------|
|20001|CreateChatSession|`userPubId: Nanoid<16, Base62Alphabet>`|`sessionId: Nanoid<16, Base62Alphabet>`, `createdAt: i64`||true|
|20002|SendMessage|`sessionId: Nanoid<16, Base62Alphabet>`, `content: String`|`sentAt: i64`||true|
|20003|ListMessages|`sessionId: Nanoid<16, Base62Alphabet>`|`data: Vec<ChatMessage>`||true|
|20004|SubscribeEvents|`sessionId: Nanoid<16, Base62Alphabet>`, `unsub: Option<bool>`|`data: Vec<ChatMessage>`||true|
|20005|CloseChatSession|`sessionId: Nanoid<16, Base62Alphabet>`|||true|
|20006|ListChatSessions||`data: Vec<ChatSession>`||true|

## appAdminApi Server
ID: 3
### Endpoints
|Code|Name|Parameters|Response|Description|FE Facing|
|-----------|-----------|----------|--------|-----------|-----------|
|30000|CreateApp|`tgBotToken: String`, `appName: Option<String>`, `messagePersistenceEnabled: Option<bool>`|`appPublicId: Nanoid<16, Base62Alphabet>`, `createdAt: i64`||true|
|30001|EditApp|`appPublicId: Nanoid<16, Base62Alphabet>`, `tgBotToken: Option<String>`, `appName: Option<String>`, `active: Option<bool>`, `messagePersistenceEnabled: Option<bool>`|||true|
|30002|ListApps||`data: Vec<AppConfig>`||true|
|30003|EnableSupportUser|`appPublicId: Nanoid<16, Base62Alphabet>`, `userPubId: Nanoid<16, Base62Alphabet>`|||true|
|30005|DisableSupportUser|`appPublicId: Nanoid<16, Base62Alphabet>`, `userPubId: Nanoid<16, Base62Alphabet>`|||true|
|30006|AddAppMember|`appPublicId: Nanoid<16, Base62Alphabet>`, `userPubId: Nanoid<16, Base62Alphabet>`|||true|
|30007|SetAppMemberRole|`appPublicId: Nanoid<16, Base62Alphabet>`, `userPubId: Nanoid<16, Base62Alphabet>`, `role: AppMemberRole`|||true|
|30008|ListAppMembers|`appPublicId: Nanoid<16, Base62Alphabet>`|`data: Vec<AppMember>`||true|
|30009|EnableMessagePersistence|`appPublicId: Nanoid<16, Base62Alphabet>`|||true|
|30010|DisableMessagePersistence|`appPublicId: Nanoid<16, Base62Alphabet>`|||true|

## adminApi Server
ID: 4
### Endpoints
|Code|Name|Parameters|Response|Description|FE Facing|
|-----------|-----------|----------|--------|-----------|-----------|
|40000|DeleteApp|`appPublicId: Nanoid<16, Base62Alphabet>`|||true|
|40001|SetLogLevel|`level: LogLevel`|||true|
|40002|GetUsers||`data: Vec<UserInfo>`||true|
|40003|SetRole|`userPubId: Nanoid<16, Base62Alphabet>`, `role: UserRole`|||true|
|40004|GetAllApps||`data: Vec<AppInfo>`||true|

## supportApi Server
ID: 5
### Endpoints
|Code|Name|Parameters|Response|Description|FE Facing|
|-----------|-----------|----------|--------|-----------|-----------|
|20007|SetMyTgHandle|`tgHandle: String`|||true|
|20008|GetMyTgHandle||`tgHandle: Option<String>`||true|
