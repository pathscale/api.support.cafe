
# API Reference

## Structs/Datamodels

```rust
struct AppConfig{ appPublicId: Nanoid<16, Base62Alphabet>, tgBotToken: String, apiKey: String, appName: Option<String>, active: bool, createdAt: i64 }


struct ChatMessage{ incoming: bool, sentAt: i64, content: String }


struct ChatSession{ sessionId: Nanoid<16, Base62Alphabet>, appPublicId: Nanoid<16, Base62Alphabet>, userPubId: Nanoid<16, Base62Alphabet>, createdAt: i64, closedAt: Option<i64> }

```
---

## Enums

```rust
enum LogLevel { Trace, Debug, Info, Warn, Error }


enum UserRole { Public, Admin, App, User, HoneyAuth }

```
---

        

## authApi Server
ID: 0
### Endpoints
|Code|Name|Parameters|Response|Description|FE Facing|
|-----------|-----------|----------|--------|-----------|-----------|
|1|Init|`accessToken: String`|`userId: Nanoid<16, Base62Alphabet>`, `role: UserRole`, `version: String`||true|

## appConnect Server
ID: 1
### Endpoints
|Code|Name|Parameters|Response|Description|FE Facing|
|-----------|-----------|----------|--------|-----------|-----------|
|100|AppConnect|`appPublicId: Nanoid<16, Base62Alphabet>`, `userPublicId: Nanoid<16, Base62Alphabet>`|`appPublicId: Nanoid<16, Base62Alphabet>`, `appName: Option<String>`||true|

## appApi Server
ID: 2
### Endpoints
|Code|Name|Parameters|Response|Description|FE Facing|
|-----------|-----------|----------|--------|-----------|-----------|
|110|CreateSession||`sessionId: Nanoid<16, Base62Alphabet>`, `createdAt: i64`||true|
|111|SendMessage|`sessionId: Nanoid<16, Base62Alphabet>`, `content: String`|`sentAt: i64`||true|
|112|ListMessages|`sessionId: Nanoid<16, Base62Alphabet>`|`data: Vec<ChatMessage>`||true|
|113|SubscribeEvents|`sessionId: Nanoid<16, Base62Alphabet>`, `unsub: Option<bool>`|`data: Vec<ChatMessage>`||true|
|114|CloseSession|`sessionId: Nanoid<16, Base62Alphabet>`|||true|
|115|ListSessions||`data: Vec<ChatSession>`||true|
