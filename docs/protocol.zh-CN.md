# Monica Cloud 同步协议 v1

本文是 Monica Cloud Server、WebUI 和 Monica CE（Android）三方共同遵守的契约。
任何一方改动加密格式或接口语义，必须先改本文。

设计目标：

- **零知识**：服务器只保存密文和必要的元数据，永远拿不到主密码、账户密钥或条目明文。
- **自定义端点**：客户端像 Bitwarden 自托管那样填写服务器地址（例如 `https://cloud.example.com`）后登录。
- **条目级同步**：每个条目独立加密、独立版本号，按 revision 增量同步，乐观并发检测冲突。
- **跨端可实现**：所有算法都能用浏览器 WebCrypto 和 Android `javax.crypto` 直接实现，不依赖原生库。

---

## 1. 密钥体系

```text
主密码 ──PBKDF2──► masterKey ──HKDF──► wrapKey ──解开──► userKey ──解开──► itemKey ──► 条目 data
                       │                                     │
                       └─PBKDF2(1 次)─► authHash（发给服务器） └──解开──► sendKey / folder name
```

### 1.1 KDF

| 字段 | 值 |
| --- | --- |
| `kdf` | `0` = PBKDF2-HMAC-SHA256（v1 唯一支持的值；`1` 预留给 Argon2id） |
| `kdfIterations` | 默认 `600000`，服务器允许范围 `100000`–`2000000` |

```text
salt       = UTF-8( lowercase( trim(email) ) )
masterKey  = PBKDF2-HMAC-SHA256(password = UTF-8(masterPassword), salt, kdfIterations, 32 bytes)
authHash   = Base64( PBKDF2-HMAC-SHA256(password = masterKey, salt = UTF-8(masterPassword), 1, 32 bytes) )
wrapKey    = HKDF-SHA256(ikm = masterKey, salt = 空, info = "monica-cloud/v1/wrap", 32 bytes)
```

- 主密码不做 Unicode 规范化以外的处理；客户端统一使用 NFC 规范化后的字符串。
- `authHash` 是唯一发送到服务器的口令材料，服务器再用 Argon2id 哈希后存储。
- HKDF 的 salt 为空时，按 RFC 5869 等价于 32 个 0 字节。

### 1.2 userKey

注册时客户端随机生成 32 字节 `userKey`，用 `wrapKey` 加密成 `protectedUserKey` 上传。
改主密码只需要重新包装 `userKey`，所有条目无需重新加密。

### 1.3 itemKey

每个条目有独立的 32 字节 `itemKey`：

```text
item.key  = EncString(userKey, itemKey)
item.data = EncString(itemKey, UTF-8(JSON(payload)))
```

以后做组织共享时，只需要用组织密钥重新包装 `itemKey`。

### 1.4 EncString

```text
"1." + Base64(iv) + "." + Base64(ciphertext ‖ tag)
```

- 算法：AES-256-GCM，`iv` 12 字节随机，`tag` 16 字节，无 AAD。
- `Base64` 是标准字母表、带 padding（RFC 4648 §4）。
- 版本前缀不是 `1.` 的字符串，客户端必须拒绝解密并提示升级。

### 1.5 二进制分块加密（附件、Send 文件）

文件可能很大，所以按块加密，方便流式处理，也能防止重排和截断：

```text
header  = "MCB1" (4 bytes) ‖ chunkSize (u32 BE, 默认 65536)
chunk_i = iv_i (12) ‖ AES-256-GCM(key, iv_i, plaintext_i, aad = u64_BE(i) ‖ final_flag) ‖ tag (16)
file    = header ‖ chunk_0 ‖ chunk_1 ‖ … ‖ chunk_n
```

- 除最后一块外，每块明文长度都恰好是 `chunkSize`；最后一块的 `final_flag = 0x01`，其它块为 `0x00`。
- 空文件也必须产生一个明文长度为 0 的最终块。
- 密文总长度 = `8 + Σ(28 + len(plaintext_i))`，客户端上传前把它填进 `size`，服务器会校验。

### 1.6 Send 密钥

```text
sendKey    = 16 字节随机数，以 Base64URL（无 padding）放在分享链接的 # 片段里
contentKey = HKDF-SHA256(ikm = sendKey, salt = 空, info = "monica-cloud/v1/send", 32 bytes)
send.key   = EncString(userKey, sendKey)            // 所有者自己能再次打开
send.name / notes / text = EncString(contentKey, …)
passwordHash = Base64( PBKDF2-HMAC-SHA256(UTF-8(sendPassword), sendKey, 100000, 32) )
```

分享链接格式：`{serverUrl}/#/send/{accessId}/{sendKeyBase64Url}`。

---

## 2. 条目类型与 payload

服务器能看到 `type`（用于计数和筛选），看不到 payload。

| type | 名称 | CE 对应 |
| --- | --- | --- |
| 1 | login | `PasswordEntry`（loginType = PASSWORD / SSO） |
| 2 | note | `SecureItem` NOTE |
| 3 | card | `SecureItem` BANK_CARD |
| 4 | document | `SecureItem` DOCUMENT（证件） |
| 5 | totp | `SecureItem` TOTP（独立验证器） |
| 6 | passkey | `PasskeyEntry` |
| 7 | wifi | `PasswordEntry`（loginType = WIFI） |
| 8 | ssh-key | 含 `sshKeyData` 的条目 |
| 9 | billing-address | `SecureItem` BILLING_ADDRESS |
| 10 | payment-account | `SecureItem` PAYMENT_ACCOUNT |

payload 是一个 JSON 对象，公共字段如下；未知字段必须原样保留（前向兼容）：

```jsonc
{
  "v": 1,
  "name": "GitHub",
  "notes": "",
  "fields": [{ "name": "恢复码", "value": "…", "hidden": true }],
  "uris": ["https://github.com"],
  "login":    { "username": "", "password": "", "totp": "", "loginType": "PASSWORD", "ssoProvider": "" },
  "card":     { "holder": "", "number": "", "expiry": "MM/YY", "cvv": "", "brand": "" },
  "document": { "kind": "", "number": "", "holder": "", "issuedBy": "", "expiry": "" },
  "totp":     { "secret": "", "issuer": "", "account": "", "digits": 6, "period": 30, "algorithm": "SHA1" },
  "passkey":  {                       // 与 Bitwarden fido2Credentials 同形，便于互相导入
    "credentialId": "", "keyType": "public-key", "keyAlgorithm": "ECDSA", "keyCurve": "P-256",
    "keyValue": "",                     // PKCS#8 私钥 (Base64)，随条目一起端到端加密
    "rpId": "", "rpName": "", "userHandle": "", "userName": "", "userDisplayName": "",
    "counter": 0, "discoverable": true, "creationDate": "2026-11-04T00:00:00Z"
  },
  "wifi":     { "ssid": "", "password": "", "security": "WPA2", "hidden": false },
  "sshKey":   { "privateKey": "", "publicKey": "", "fingerprint": "" },
  "address":  { "fullName": "", "line1": "", "line2": "", "city": "", "state": "", "zip": "", "country": "", "phone": "", "email": "" },
  "payment":  { "provider": "", "account": "", "holder": "", "secret": "" },
  "passwordHistory": [{ "password": "", "lastUsedAt": "2026-11-04T00:00:00Z" }],
  "monica": {}   // CE 专有扩展（排序、图标、绑定关系、完整的原生数据等），其它客户端原样保留
}
```

---

枚举取值与 CE 保持一致：

- `wifi.security`：`WPA2_WPA3`、`WPA3`、`WPA_WPA2`、`WEP`、`NONE`、`WPA2_ENTERPRISE`、`WPA3_ENTERPRISE`
- `document.kind`：`ID_CARD`、`PASSPORT`、`DRIVER_LICENSE`、`SOCIAL_SECURITY`、`OTHER`
- `card.expiry`：`MM/YY`
- `login.loginType`：`PASSWORD`、`SSO`（Wi-Fi 用 type 7）

CE 写入的条目会在 `monica` 里带上完整原生数据（如 `itemData`、`wifiMetadata`、`sshKeyData`）。
其它客户端只改标准字段时，CE 以 `monica` 为底、把改动过的标准字段覆盖上去；标准字段没变时原生数据原样保留。

### 2.1 通行密钥

采用主流密码管理器（Bitwarden、1Password、iCloud 钥匙串、Google 密码管理器）的“可同步通行密钥”做法：

- 私钥是软件生成的 P-256 密钥，以 PKCS#8 形式放在 payload 的 `passkey.keyValue`，和密码一样只以密文离开设备。
- 创建和断言时 authenticator data 的 `BE`（可备份）和 `BS`（已备份）标志都置 1；这两个标志在凭据生命周期内不能改变。
- `counter` 始终为 0。多台设备无法维护同一个单调计数器，WebAuthn §6.1.1 允许 authenticator 恒返回 0，依赖方遇到 0 会跳过克隆检测。
- 只存在于 Android Keystore、无法导出私钥的旧凭据（设备绑定通行密钥）不上传 `keyValue`，并在 `monica.passkeyDeviceBound = true` 中标明；其他设备只显示、不能使用。
- 一个条目可以同时有登录密码和通行密钥，写在同一个 type 1 条目里（`login` + `passkey`）；独立的通行密钥使用 type 6。

## 3. 通用约定

- 所有接口都在 `{serverUrl}/api/v1` 下，请求和响应都是 JSON（`camelCase`），文件上传下载除外。
- 鉴权：`Authorization: Bearer <accessToken>`。
- 时间：RFC 3339 UTC，精确到微秒，例如 `2026-11-04T08:30:00.123456Z`。
  `revisionDate` 必须原样回传，不能被客户端重新格式化。
- 错误统一返回：

```json
{ "error": "conflict", "message": "item was modified on another device" }
```

| HTTP | error | 含义 |
| --- | --- | --- |
| 400 | `bad_request` | 参数错误 |
| 401 | `unauthorized` | token 无效、过期或已被吊销 |
| 403 | `forbidden` | 无权限或账户被禁用 |
| 404 | `not_found` | 资源不存在 |
| 409 | `conflict` | 乐观并发冲突，响应体额外带 `current` 字段 |
| 413 | `payload_too_large` | 超过单文件或配额限制 |
| 429 | `rate_limited` | 登录尝试过多 |

---

## 4. 接口

### 4.1 发现与账户

```http
GET  /api/v1/config                     # 公开：版本、是否开放注册、功能开关
POST /api/v1/accounts/prelogin          # { email } → { kdf, kdfIterations }
POST /api/v1/accounts/register          # { email, name, authHash, kdf, kdfIterations, protectedUserKey }
POST /api/v1/auth/token                 # 登录 / 刷新，见下
POST /api/v1/auth/logout                # 吊销当前设备的 refresh token
GET  /api/v1/accounts/profile
PUT  /api/v1/accounts/profile           # { name }
POST /api/v1/accounts/password          # { authHash, newAuthHash, newProtectedUserKey, kdfIterations? }
POST /api/v1/accounts/delete            # { authHash }
GET  /api/v1/accounts/revision-date     # { revisionDate }，客户端轮询它来决定是否需要同步
```

`prelogin` 对不存在的邮箱也返回默认参数，避免枚举账户。

登录：

```jsonc
// POST /api/v1/auth/token
{
  "grantType": "password",
  "email": "me@example.com",
  "authHash": "…",
  "device": { "identifier": "稳定的设备 UUID", "name": "Pixel 9", "type": "android", "appVersion": "1.0.302" }
}
// 或
{ "grantType": "refresh_token", "refreshToken": "…" }
```

```jsonc
// 200
{
  "accessToken": "…",            // JWT，默认 2 小时
  "expiresIn": 7200,
  "tokenType": "Bearer",
  "refreshToken": "…",           // 每次刷新都会轮换，旧的立即失效
  "protectedUserKey": "1.…",
  "kdf": 0,
  "kdfIterations": 600000,
  "account": { "id": "…", "email": "…", "name": "…", "role": "owner" }
}
```

改主密码或管理员强制下线时，服务器轮换 `securityStamp`，所有旧 token 立即失效。

### 4.2 同步

```http
GET /api/v1/sync                    # 全量
GET /api/v1/sync?since=<revision>   # 增量：只返回 revision > since 的对象和墓碑
```

```jsonc
{
  "full": false,
  "revisionDate": "2026-11-04T08:30:00.123456Z",   // 下次增量用它作为 since
  "profile": { … },
  "folders": [ Folder ],
  "items":   [ Item ],
  "sends":   [ Send ],
  "deleted": [ { "type": "item", "id": "…", "deletedAt": "…" } ]   // 永久删除的墓碑
}
```

墓碑保留 `TOMBSTONE_RETENTION_DAYS`（默认 180 天）。客户端如果拿着更早的 `since` 来请求，
服务器返回 `full: true` 的全量结果，客户端必须用全量结果整体替换本地缓存。

同一用户的 revision 严格递增，同一个写事务里所有对象共享同一个 revision。

### 4.3 条目

```jsonc
// Item
{
  "id": "…",
  "type": 1,
  "folderId": null,
  "favorite": false,
  "key": "1.…",              // EncString(userKey, itemKey)
  "data": "1.…",             // EncString(itemKey, payload JSON)
  "attachments": [ Attachment ],
  "creationDate": "…",
  "revisionDate": "…",
  "deletedDate": null,        // 回收站
  "archivedDate": null
}
```

```http
POST   /api/v1/items                        # { type, folderId, favorite, key, data, archived }
POST   /api/v1/items/import                 # { folders: [{ref, name}], items: [{… , folderRef}] } 首次上传批量导入
GET    /api/v1/items/{id}
PUT    /api/v1/items/{id}                   # { type, folderId, favorite, key, data, archived, lastKnownRevisionDate }
POST   /api/v1/items/{id}/trash             # 移入回收站
POST   /api/v1/items/{id}/restore           # 从回收站恢复
DELETE /api/v1/items/{id}                   # 永久删除（产生墓碑，删除附件）
POST   /api/v1/items/bulk                   # { action: "trash"|"restore"|"delete"|"move", ids, folderId? }
```

`PUT` 时如果 `lastKnownRevisionDate` 与服务器当前值不一致，返回 `409`，响应体的 `current`
里带上服务器版本，由客户端合并或让用户选择。省略 `lastKnownRevisionDate` 表示强制覆盖，
只允许用户在冲突界面上明确选择“用本地版本覆盖”时使用。

### 4.4 文件夹

```http
GET    /api/v1/folders
POST   /api/v1/folders            # { name: EncString(userKey, …) }
PUT    /api/v1/folders/{id}       # { name }
DELETE /api/v1/folders/{id}       # 其中的条目 folderId 置空并提升 revision
```

### 4.5 附件

```http
POST   /api/v1/items/{id}/attachments          # { fileName: EncString(itemKey), key: EncString(itemKey, attachmentKey), size }
PUT    /api/v1/items/{id}/attachments/{aid}    # 原始密文字节（application/octet-stream）
GET    /api/v1/items/{id}/attachments/{aid}    # 下载密文
DELETE /api/v1/items/{id}/attachments/{aid}
```

先登记再上传，服务器校验字节数与 `size` 一致；未上传完成的附件不会出现在同步结果里，24 小时后清理。

### 4.6 Send

```http
GET    /api/v1/sends
POST   /api/v1/sends                    # 文本：{ type: 0, key, name, notes, text, passwordHash?, maxAccessCount?, expirationDate?, deletionDate, hideEmail, disabled }
POST   /api/v1/sends                    # 文件：type: 1，额外带 fileName（EncString）和 size，之后：
PUT    /api/v1/sends/{id}/file          # 上传密文
PUT    /api/v1/sends/{id}               # 修改；passwordHash 为 null 保持不变，removePassword: true 删除密码
DELETE /api/v1/sends/{id}

POST   /api/v1/public/sends/{accessId}              # 公开访问：{ passwordHash? } → Send 内容，文本类型会计数
GET    /api/v1/public/sends/{accessId}/file/{token} # 下载文件密文并计数，token 5 分钟内有效
```

被禁用、已过期、超过访问次数或已到删除时间的 Send 一律返回 404。

### 4.7 设备、事件、管理

```http
GET    /api/v1/devices
DELETE /api/v1/devices/{id}         # 吊销该设备的 refresh token
GET    /api/v1/events?limit=&before=

# 以下需要 owner/admin 角色
GET    /api/v1/admin/users
PATCH  /api/v1/admin/users/{id}     # { enabled?, role?, quotaBytes? }
POST   /api/v1/admin/users/{id}/deauth
DELETE /api/v1/admin/users/{id}
GET    /api/v1/admin/diagnostics
GET    /api/v1/admin/settings
PUT    /api/v1/admin/settings       # { signupsAllowed, sendsAllowed, defaultQuotaBytes, eventRetentionDays }
GET    /api/v1/admin/events
```

---

## 5. 客户端同步流程（CE 参考实现要求）

1. 前台或定时任务调用 `GET /accounts/revision-date`；与本地记录相同就结束。
2. 有本地待上传修改时，先逐条 `PUT`（带 `lastKnownRevisionDate`）或 `POST`。
   - `409`：把服务器版本解密后与本地版本做字段级合并；无法自动合并的进入冲突队列让用户选择。
3. 调用 `GET /sync?since=…` 拉取远端变化，按 `revisionDate` 覆盖本地未修改的条目，按墓碑删除本地条目。
4. 保存新的 `revisionDate`。
5. 远端条目数从非零突然变成零时，必须暂停同步并提示用户（与现有 Bitwarden 模块的 `EmptyVaultProtection` 一致）。

---

## 6. 预留（v1 不实现）

- 两步验证（TOTP / WebAuthn）
- 组织、集合、成员与策略（需要用户公私钥对，用来包装 itemKey）
- WebSocket 实时推送（v1 用 revision-date 轮询）
- Argon2id KDF（需要浏览器端 WASM）
