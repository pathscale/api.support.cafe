
# API Reference

## Structs/Datamodels

```rust
struct AppConfig{ appPublicId: Nanoid<16, Base62Alphabet>, tgBotToken: String, appName: Option<String>, active: bool, createdAt: i64 }


struct AppInfo{ publicId: Nanoid<16, Base62Alphabet>, appName: Option<String>, active: bool, createdAt: i64 }


struct ChatMessage{ sessionId: Nanoid<16, Base62Alphabet>, incoming: bool, sentBy: String, sentAt: i64, content: String }


struct ChatSession{ sessionId: Nanoid<16, Base62Alphabet>, appPublicId: Nanoid<16, Base62Alphabet>, userPubId: Nanoid<16, Base62Alphabet>, createdAt: i64, closedAt: Option<i64> }


struct SupportUser{ id: i64, appPublicId: Nanoid<16, Base62Alphabet>, tgHandle: String, chatId: Option<i64>, isActive: bool }


struct UserInfo{ id: i64, pubId: Nanoid<16, Base62Alphabet>, username: String, role: UserRole }

```
---

## Enums

```rust
enum LogLevel { Trace, Debug, Info, Warn, Error }


enum UserRole { Public, Admin, App, User, AppAdmin, HoneyAuth }

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
|20001|CreateSession|`userPubId: Nanoid<16, Base62Alphabet>`|`sessionId: Nanoid<16, Base62Alphabet>`, `createdAt: i64`||true|
|20002|SendMessage|`sessionId: Nanoid<16, Base62Alphabet>`, `content: String`|`sentAt: i64`||true|
|20003|ListMessages|`sessionId: Nanoid<16, Base62Alphabet>`|`data: Vec<ChatMessage>`||true|
|20004|SubscribeEvents|`sessionId: Nanoid<16, Base62Alphabet>`, `unsub: Option<bool>`|`data: Vec<ChatMessage>`||true|
|20005|CloseSession|`sessionId: Nanoid<16, Base62Alphabet>`|||true|
|20006|ListSessions||`data: Vec<ChatSession>`||true|

## appAdminApi Server
ID: 3
### Endpoints
|Code|Name|Parameters|Response|Description|FE Facing|
|-----------|-----------|----------|--------|-----------|-----------|
|30000|CreateApp|`tgBotToken: String`, `appName: Option<String>`|`appPublicId: Nanoid<16, Base62Alphabet>`, `createdAt: i64`||true|
|30001|EditApp|`appPublicId: Nanoid<16, Base62Alphabet>`, `tgBotToken: Option<String>`, `appName: Option<String>`, `active: Option<bool>`|||true|
|30002|ListApps||`data: Vec<AppConfig>`||true|
|30003|AddSupportUser|`appPublicId: Nanoid<16, Base62Alphabet>`, `tgHandle: String`|||true|
|30004|ListSupportUsers|`appPublicId: Nanoid<16, Base62Alphabet>`|`data: Vec<SupportUser>`||true|
|30005|RemoveSupportUser|`appPublicId: Nanoid<16, Base62Alphabet>`, `tgHandle: String`|||true|

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
