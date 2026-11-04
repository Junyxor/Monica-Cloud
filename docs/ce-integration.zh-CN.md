# Monica for Android（CE）接入 Monica Cloud

实现位于 Monica-CE 的 `feat/monica-cloud` 分支，包 `takagi.ru.monica.cloud`。
协议见 `protocol.zh-CN.md`，加密实现通过 `test-vectors.json`。

## 1. 同步什么

Monica Cloud 是 **Monica 本地库** 的云端：登录后，所有“Monica 本地”归属的条目
（不属于 KeePass / Bitwarden / MDBX 的条目）自动加密上传，并在多台设备之间双向同步。
KeePass、Bitwarden、MDBX 的条目继续走各自的同步，不会上传，避免重复。
同一时间只登录一个 Monica Cloud 账户。

| 协议 type | CE 来源 |
| --- | --- |
| 1 login / 7 wifi / 8 ssh-key | `PasswordEntry`（按 `loginType`、`sshKeyData` 区分），自定义字段来自 `custom_fields` |
| 2 / 3 / 4 / 5 / 9 / 10 | `SecureItem` NOTE / BANK_CARD / DOCUMENT / TOTP / BILLING_ADDRESS / PAYMENT_ACCOUNT |
| 6 passkey | `PasskeyEntry`，私钥经 `PasskeyPrivateKeyStore` 取出 PKCS#8 写入 `keyValue` |
| 文件夹 | 本地 `Category`（不属于 Bitwarden / MDBX 的分类） |

暂不同步：附件与图片（`imagePaths`、`attachments`）、Send、密码历史表、`SecureItem` 中遗留的 PASSWORD 类型。

## 2. 模块结构

```text
cloud/
├── crypto/MonicaCloudCrypto.kt        协议 §1，纯 JVM，可在单测中运行
├── api/                               Retrofit 接口、数据模型、自动刷新 token 的客户端
├── sync/MonicaCloudSyncEngine.kt      双向同步引擎（纯 Kotlin，存储通过接口注入）
├── mapper/MonicaCloudEntityMapper.kt  Room 实体 ⇄ 协议 payload
├── data/                              Room 表、DAO、LocalStore / LinkStore 的 Room 实现
├── MonicaCloudManager.kt              登录、同步调度（WorkManager）、本地改动监听
├── MonicaCloudAccountStore.kt         账户状态（EncryptedSharedPreferences）
└── ui/MonicaCloudScreen.kt            设置 → 同步与备份 → Monica Cloud
```

## 3. 数据层

不改动现有实体，只新增两张表（数据库 77 → 78，`MIGRATION_77_78`）：

- `monica_cloud_links`：云端条目/文件夹 id ⇄ 本地条目/分类，记录上次同步到的服务器 revision、
  本地内容哈希、回收站状态和条目密钥密文。
- `monica_cloud_payload_extras`：其它客户端写入、CE 不认识的 payload 字段，以及表里没有对应列的
  标志位（通行密钥的收藏/回收站、安全条目的归档），保证往返不丢。

账户（服务器地址、会话、账户密钥、同步游标）保存在 `SecurityManager` 的 EncryptedSharedPreferences，
与 Bitwarden 模块保存密钥的方式一致，这样后台同步可以在 App 未打开时运行。

## 4. payload 映射

payload 分两层：

- 标准字段（`login`、`card`、`totp`、`wifi` …）：WebUI 显示和编辑的部分；
- `monica` 段：CE 的完整原生数据（`itemData`、`wifiMetadata`、`sshKeyData`、排序、图标、应用绑定等）。

应用远端 payload 时以 `monica` 为底；只有标准字段与原生数据推导出的值不同（说明被其它客户端改过）
时才覆盖并重新编码，否则原生数据逐字节保留。这保证映射是幂等的，未修改的条目不会被反复上传。

本地加密：`PasswordEntry.password` 读取时 `decryptDataIfMonicaCiphertext`、写入时 `encryptData`，
与编辑器一致；其它字段兼容旧的 Monica 密文。

不参与同步的易变字段：`updatedAt`、通行密钥的 `lastUsedAt` / `useCount` / `signCount`。

## 5. 同步流程

1. 拉取 `GET /sync?since=游标`，按 `profile.id` 校验账户；
2. 文件夹：按链接重命名或新建本地分类，同名未关联的分类会被复用；
3. 条目：
   - 未关联的远端条目：先把它“预演”成本地会存成的样子，再只比较标准字段（忽略 `monica` 段）；本地有相同且尚未关联的条目就直接关联（首次登录、退出后重新登录都不会产生重复），否则新建；
   - 远端变化、本地未变：覆盖本地；
   - 两边都变：本地版本另存为“冲突副本”，再采用云端版本；
   - 远端删除：本地未改则删除；本地改过则保留并作为新条目重新上传（编辑优先于删除）；
   - 全量同步时云端突然为空而本地已同步 ≥ 5 条：中止，界面让用户确认。
4. 推送：新条目用 `POST /items/import` 批量上传（每批 500）；修改用 `PUT` 带 `lastKnownRevisionDate`，
   409/404 留到下次同步处理；回收站状态单独调用 trash / restore；本地删除调用 `DELETE`。

触发：App 启动、本地条目/分类表变化（Room InvalidationTracker，5 秒防抖，忽略同步自身的写入）、
WorkManager 每 15 分钟，以及设置页“立即同步”。

## 6. 测试

| 测试 | 内容 |
| --- | --- |
| `MonicaCloudCryptoTest` | 共享测试向量、RFC 7914、篡改/截断检测 |
| `MonicaCloudEntityMapperTest` | 各类型无损往返、映射幂等、WebUI 编辑覆盖、设备绑定通行密钥 |
| `MonicaCloudInteropTest` | 对真实服务器的完整客户端流程；解密 WebUI 写入的保险库 |
| `MonicaCloudSyncEngineTest` | 多设备收敛、冲突副本、删除传播、首次登录去重、空保险库保护 |

后两个需要运行中的服务器：

```bash
MONICA_CLOUD_URL=http://127.0.0.1:8000 ./gradlew :app:testDebugUnitTest --tests 'takagi.ru.monica.cloud.*'
```

## 7. 真机验证（Android 14 x86_64 模拟器）

| 场景 | 结果 |
| --- | --- |
| 旧版（数据库 77）有数据时覆盖安装新版 | 迁移到 78，原有条目保留，Room 校验通过 |
| 设置 → 数据库与备份 → Monica Cloud 登录 | 拉取 WebUI 写入的 13 个条目和 3 个文件夹，本机条目上传 |
| WebUI 修改名称、用户名、银行卡有效期 | 点“立即同步”后本机更新 |
| 在 App 列表里收藏条目 | 约 5 秒后自动同步到服务器（表变更监听 + WorkManager） |
| 强制停止 App、服务器端修改、重新打开 | 启动后自动拉取，会话与密钥在重启后保留 |
| 退出登录再登录 | 不产生重复条目 |
| 本机通行密钥上传 / 服务器通行密钥下载 | `keyValue` 与本地 PKCS#8 一致；下载的私钥进入 `PasskeyPrivateKeyStore` 受保护存储 |
| 无变化时再次同步 | 服务器上所有条目 revision 不变（映射幂等） |

## 8. 后续

- 附件与图片同步（协议 §4.5 已就绪）
- Send 列表与创建
- 字段级三方合并（替代冲突副本）
- 设置页显示冲突副本列表
