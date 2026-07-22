# Telegram-Drive 生产加固整合方案

> 2026-07-10 · 承接三轮侦察：4 个参考项目亮点抽取 + 当前项目 500 并发/7x24 缺口审查
> 交付：方案文档先行，用户审批后再实施

---

## 0. 摘要（先读这段）

当前项目（v4.0.0-beta）模块完整（59 个 Rust 文件、96.88% 覆盖率）、安全模块齐全，**但「生产就绪」是验收报告层面的，不是 500 并发真实负载层面的**。缺口审查暴露 6 个 CRITICAL/HIGH 级硬伤，会使 500 路并发实际上限跌到个位数/秒，且 7x24 部署会在 Redis 宕机/Bot 被封/User 断线时停服。

本方案以**纯服务端（Actix Headless API + 静态 Web UI）**为产品形态，分四层补齐：连接双轨与降级、背压重做、可靠性四件套、数据一致性自愈。每项都给出降级路径与落点（文件:行号）。

**关键纠偏**：用户明确「桌面程序无所谓」，桌面 Tauri GUI 退出交付范围。Web UI 面向**其他用户**上传（公网多用户网盘），这引入 4 个此前未考虑的盲点（见 §6）。

---

## 1. 产品形态定调

| 维度 | 决定 |
|------|------|
| 交付形态 | **纯服务端**：`telegram-drive-server` headless binary + 静态 Web UI（`deploy/web/`）|
| 桌面 Tauri | **不交付**（用户明确「无所谓」）。代码保留不删，但不在加固范围 |
| 连接模式 | **Bot 池 + User 模式双轨**，User 为主、Bot 为辅，互为降级 |
| Web UI 定位 | **公网多用户上传入口**（不限本机），非仅管理员控制台 |
| 并发目标 | 500 路同时上传可持续消化（排队而非拒绝） |
| 运行目标 | 7x24 无人值守，单点故障自动降级不中断 |

---

## 2. 当前项目缺口（缺口审查结论，按严重级）

### CRITICAL（合并前必须修）

| # | 缺口 | 现状 | 后果 |
|---|------|------|------|
| C1 | Bot 上传前固定 3.5s 串行 sleep | `BOT_RATE_LIMIT_MS=3500`，`telegram_transport.rs:429` 全局串行 | 8 槽 × 3.5s ≈ 2.3 upload/s，500 并发需 217s 消化一批 |
| C2 | 每次上传 `reqwest::Client::new()` | `telegram_transport.rs:453` 无连接池复用 | 500 并发 = 500 次 TLS 握手，开销显著 |
| C3 | chunk 全量缓冲到内存 | `legacy_routes.rs:94-104` `Vec<u8>` | 500 × 20MB = 10GB，Docker 限 2G → OOM kill |
| C4 | Redis 运行时宕机无回退 | `upload_gate.rs:91-114` acquire 返 false→全 503，release 静默失败→槽位永久泄漏 | Redis 一抖，上传服务全停且无法自愈 |
| C5 | SQLite 连接池 max_size=8 | `db.rs:44-48` | 500 并发下 SQLITE_BUSY，5s 超时返 500 |

### HIGH（合并前应修）

| # | 缺口 | 现状 | 后果 |
|---|------|------|------|
| H1 | `/upload_chunk` 用 `try_acquire`（非阻塞） | `legacy_routes.rs:76` | 8 槽瞬间占满，492 路直接 503 而非排队 |
| H2 | Bot 模式无重试 | `http_upload.rs:62-66` 返 Err 即终止 | 网络抖动→整文件失败，客户端重传 |
| H3 | FloodWait 不向客户端传 Retry-After | `legacy_routes.rs:190-193` 返 500 | 客户端无法退避，雪崩 |
| H4 | Bot 被封(401/403)不隔离 | `bot_pool.rs` 无 banned 状态 | 被封 bot 永远 `is_available=true`，轮询必失败 |
| H5 | User 模式无周期健康探测 | `server_maintenance.rs:89` 仅 Bot keepalive | User 断线后到下次请求才发现，不自愈 |
| H6 | 孤儿 chunk 不清理 | `cleanup_stale_uploads` 只删 DB 不删 TG 消息 | 频道堆垃圾，触发 TG 限制 |
| H7 | S3 降级后端未接入 | `ServerConfig` 有字段无逻辑 | TG 全挂时无替代存储，直接 503 |
| H8 | 全局限流不分类 | `RATE_LIMIT_RPM=600` 上传/下载/列表共享 | 500 上传 50s 内被限流拒 |

### MEDIUM（兜底项，列出让用户知情）

Runner 100ms shutdown 不保证退出（`auth.rs:49-53`）、MetadataCache 多副本不传播（`metadata_cache.rs:2`）、peer_cache/cancelled_transfers 无 LRU 无限增长、Actix `stop(true)` 无关闭超时、Docker unhealthy 不触发重启、无进程监督器(PID 1)。详见缺口审查报告。

---

## 3. 参考项目亮点抽取（按优先级）

| 优先级 | 来源 | 亮点 | 落入本项目 |
|--------|------|------|-----------|
| **P0** | Pentaract | DB 原子单事务限流 `HAVING COUNT<limit ORDER BY COUNT LIMIT 1` | 替换 H8 全局限流，公平且可水平扩展 |
| **P0** | K-Vault | 下载 Range 透传 → 206 Partial Content | 大文件分段下载，见 §4.4 |
| **P0** | tgDrive | 上传 3 次重试 + 指数退避 | 修 H2 Bot 模式重试 |
| **P1** | tgDrive | PipedStream 并发分片下载 + 流式合并 | Rust 用 `tokio::io::duplex` |
| **P1** | Pentaract | Actor 模型 worker 调度 | 可选，C1 修好后收益递减 |
| **P1** | K-Vault | 健康检查 + storage-regression 脚本 | 已有 `/health`，补回归脚本 |
| **P2** | tgDrive | 每日 Bot keepalive | H5 一并覆盖 User 模式 |
| **P2** | tgNetDisc | 多格式外链（HTML/MD/BBCode/URL） | 分享增强，非加固必需 |
| **避免** | tgNetDisc | `log.Panic` 杀进程、无限重试、`fileAll.txt` 清单 | 不照搬，本项目已避坑 |

---

## 4. 加固方案（四层闭环）

### 4.1 第一层：连接双轨与智能降级

```
                    ┌─ User 模式(MTProto, 无大小限制) ── 主路径
上传请求 ──路由决策──┤
                    └─ Bot 模式(≤20MB 分片, 多 bot 轮询) ── 降级路径
```

**路由规则**（新增 `transport_router.rs`）：
1. User session 健康 → 走 User，单文件不分片
2. User 断线且文件 ≤20MB → 降级 Bot 分片
3. User 断线且文件 >20MB → 入 S3 降级队列（§4.5），待 User 恢复回迁
4. User + Bot 全挂 → 503 + Retry-After，触发 S3 兜底

**Bot 健康隔离**（修 H4）：`BotPool` 增加 `BotStatus { Healthy, FloodWait(until), Banned(at) }`。401/403 连续 3 次 → `Banned`，30 分钟内不轮询，过期重试 1 次探活。落点：`bot_pool.rs:46-50` `is_available()`。

**User 周期探活**（修 H5）：`server_maintenance.rs` 新增 `spawn_user_keepalive`，每 60s `get_me` 探测，失败触发 `ensure_client_initialized_at()` 重连。复用现有重连逻辑（`auth.rs:36-175`）。

→ 验证：断开 User session，60s 内自动重连且期间 ≤20MB 上传走 Bot 不中断。

### 4.2 第二层：背压重做（修 C1/C2/C3/C4/H1/H3）

**C1+C2 修：Bot 上传提速**
- 移除全局 3.5s 串行 sleep，改为**每 bot 独立令牌桶**（每 bot 0.3 req/s，多 bot 并行）。落点：`telegram_transport.rs:429` `bot_rate_limit()`。
- `reqwest::Client` 改为**进程级单例**（lazy `OnceLock<reqwest::Client>`，配连接池 `pool_max_idle_per_host=64`）。落点：`telegram_transport.rs:453`。

**C3 修：流式上传防 OOM**
- `/upload_chunk` 改用 `actix_multipart::Field`（流式），写临时文件而非 `Vec<u8>`。上传 TG 时 `tokio::fs::File` 流式 `send_message`。
- 全局内存水位门：`Arc<AtomicU64>` 跟踪 in-flight 字节，超容器上限 60%（Docker 2G → 1.2G）时新请求 503+Retry-After。

**C4 修：Redis 运行时降级链**
```
Redis acquire 失败 → 健康探测 Redis
  ├ Redis 不可达 → 切 Memory 后端（运行时热切，非仅启动时）
  └ Redis 可达但满 → 503+Retry-After
Redis release 失败 → 记 tombstone，下次 acquire 时 reconcile（DECR 校正）
status() → Redis 异常时标记 degraded，监控可见
```
落点：`upload_gate.rs:91-114`，新增 `GateBackend::swap_to_memory()`。

**H1 修：排队而非拒绝**
- `/upload_chunk` 改 `acquire_chunk()`（5 分钟等待，已实现但未被调用，`upload_gate.rs:16`）而非 `try_acquire_chunk()`。前端 503→退避重试，202 Accepted→排队。
- 槽位上调：500 并发目标下 `CHUNK_CONCURRENT=32`、`FILES_CONCURRENT=16`（prod compose 调整）。

**H3 修：FloodWait 透传**
- Bot 返 `FLOOD_WAIT:{index}:{secs}` 时，handler 返 503 + `Retry-After: secs`，不返 500。落点：`legacy_routes.rs:190-193`。

**C5 修：DB 池扩容**
- r2d2 `max_size=8 → 32`，`connection_timeout=5s → 30s`。WAL + `busy_timeout` 已开。落点：`db.rs:44-48`。

→ 验证：500 路并发上传，平均排队 <30s，无 503 雪崩，内存峰值 <1.2G，无 OOM。

### 4.3 第三层：可靠性四件套（用户选定的四层）

| 层 | 机制 | 落点 |
|----|------|------|
| Bot 故障自动转移 | §4.1 路由 + Banned 隔离 + 多 bot 轮询跳过 FloodWait | `bot_pool.rs` `transport_router.rs` |
| 断线重连+进程守护 | User keepalive 60s 探活 + 重连；进程级加 tini/s6 作 PID 1，捕获 SIGTERM/SIGPIPE；`stop(true)` 加 30s shutdown_timeout | `server_maintenance.rs` `Dockerfile` `telegram-drive-server.rs:123` |
| Redis 降级链 | §4.2 C4：运行时 Redis→Memory 热切 + tombstone reconcile | `upload_gate.rs` |
| 数据一致性自愈 | §4.4：孤儿清理 + 事务包裹 + abort API | `db.rs` `legacy_routes.rs` |

**进程守护补充**（MEDIUM 级盲点）：Dockerfile 加 `tini` 作 PID 1（`apt-get install tini` + `ENTRYPOINT ["/usr/bin/tini","--"]`）；配置 `docker-compose.prod.yml` healthcheck unhealthy 配合外部 `autoheal` 容器重启（或文档说明需 swarm/k8s 才能按 unhealthy 重启）。

### 4.4 第四层：数据一致性自愈（修 H6 + 事务）

**孤儿 chunk 清理**（修 H6）：
- `cleanup_stale_uploads()`（`db.rs:997`）扩展：删 DB 记录前，先按 session_id 查 chunk 列表，调 Bot/User `delete_messages` 删 TG 频道消息，再删 DB。
- 新增 `POST /api/v1/uploads/{session_id}/abort`：客户端主动取消 → 立即触发孤儿清理（不等 7 天）。

**事务包裹**：
- `create_upload_session`（`db.rs:854-886`）：`BEGIN → INSERT session → INSERT chunks → COMMIT`，进程崩溃不留半 session。
- `merge_chunks` 成功后 `complete_upload_session` 包入同一事务，manifest 消息 ID 先写 DB 再确认，防重复合并。

→ 验证：上传中途 kill 进程，重启后无孤儿 chunk 残留 TG；abort API 调用后 5s 内 TG 消息删除。

### 4.5 S3 降级后端（修 H7，最后一道兜底）

`ServerConfig` 已有 s3_* 字段，补实现 `s3_backend.rs`：
- TG 全挂（User 断线 + 所有 Bot Banned/FloodWait）时，上传写 S3，DB 记 `storage=s3`。
- User 恢复后后台 job 将 S3 对象回迁 TG，回迁成功更新 DB 为 `storage=telegram` 并删 S3。
- 下载路由按 `storage` 字段分流：TG → 现有流；S3 → 直接 S3 GET 或预签名 URL 重定向。

依赖：`aws-sdk-s3`（或 `rust-s3`）。**此项可降级为 P1**——若用户暂无 S3，可先实现「全挂时 503+Retry-After + 告警」，S3 后置。

→ 验证：模拟 TG 全挂，上传不丢（落 S3），TG 恢复后自动回迁。

### 4.6 限流重做（修 H8，借 Pentaract）

替换全局 RPM 为**分类 DB 原子限流**：
- 按 `(tenant_id, action)` 分类：upload / download / list 各独立配额。
- 单事务 `INSERT INTO rate_log ...; SELECT COUNT(*) HAVING COUNT < limit ORDER BY ts LIMIT 1`（Pentaract 式，原子公平）。
- 500 并发上传配额独立，不被列表/下载挤占。

落点：`http_middleware.rs` 限流中间件 + 新增 `rate_limit_table`。

---

## 5. 降级矩阵（多方案多方位补齐）

| 故障 | 第一降级 | 第二降级 | 终态 |
|------|---------|---------|------|
| 单 Bot 被 FloodWait | 轮询跳过该 bot | 其他 bot 接管 | 无感知 |
| 单 Bot 被封 | 标 Banned 隔离 | 30min 后探活 | 隔离期间不轮询 |
| 所有 Bot 不可用 | 切 User 模式 | User 也不可用→S3 | S3 兜底 |
| User session 断线 | ≤20MB 走 Bot 分片 | >20MB 入 S3 队列 | 待 User 恢复回迁 |
| Redis 宕机 | 热切 Memory 后端 | tombstone reconcile 槽位 | 降级运行不中断 |
| SQLite BUSY | busy_timeout 30s 重试 | 池扩容到 32 | 极少触发 |
| 内存逼近上限 | 60% 水位门 503+Retry-After | 客户端退避 | 防 OOM |
| 客户端取消上传 | abort API 立即清理 | 7 天兜底清理 | 无孤儿 |
| 进程崩溃 | Docker restart | tini 处理信号 | 自启 |
| 大文件下载 | Range 透传 206 | PipedStream 并发分片 | 流式不 OOM |

---

## 6. 你没考虑到的盲点（公网多用户上传引入）

Web UI 面向其他用户上传 = 公网开放上传入口。这是产品形态的关键转变，引入 4 个必须正面解决的问题：

### 6.1 滥用防御（CRITICAL，开放上传 = 任何人可往你 TG 投递文件）
- **现状盲点**：当前 Web UI 用 `ACCESS_PWD` 单密码，公网暴露后密码泄露即被刷。
- **方案**：
  - 上传入口要求 token（管理员签发/或匿名 token 带配额）。
  - 文件类型白名单 + 魔数校验（不信扩展名）。
  - 单 token 上传配额（文件数/总字节/天），借 §4.6 分类限流。
  - 病毒扫描钩子（可选，ClamAV sidecar，大文件降级为「延迟扫描 + 隔离」）。
  - 文件名净化（已有路径净化，补 TG 元数据注入防护）。

### 6.2 用户身份模型（HIGH，决定 6.1/6.3/6.4 怎么做）
- **三种模型，需用户选**：
  1. **匿名 token**：管理员生成一次性/限时 token，访客凭 token 上传。最简单，适合「给朋友传文件」。
  2. **注册账号**：访客注册，自管理文件。重，需增 auth 表、注册流。
  3. **API Key 分租户**：复用现有 `tenant_auth.rs`，每个上传者一个 tenant。中等，已有基础设施。
- **建议**：模型 3（复用 tenant_auth），最小改动满足多用户隔离。

### 6.3 内容责任（HIGH，存储在你 TG 账号 = 你担责）
- **现状盲点**：他人上传的违法/侵权内容存在你的 Telegram 频道，法律风险在你。
- **方案**：
  - 上传即记录 `uploader_token + ip + ts + sha256`，可追溯（DB 字段已部分具备）。
  - 管理员审核队列：匿名/新 token 上传默认「未发布」，审核后才生成分享链接。
  - DMCA/举报下架 API：一键删除文件 + TG 消息 + 撤分享（复用现有 `PurgeIndexResult`）。
  - 服务条款 + 上传声明（Web UI 强制勾选）。

### 6.4 配额与公平（MEDIUM，防一人占满 Bot 槽）
- **现状盲点**：500 并发若被一个用户占满，其他人全 503。
- **方案**：
  - 每 tenant 独立并发上限（如 max 50 chunk 槽），剩余给他人。
  - 全局槽位中保留「保障池」（如 20% 给小租户），防大户独占。
  - 借 §4.6 分类限流实现，`UploadGate` 增加 per-tenant 子信号量。

---

## 7. 实施分期（建议顺序，每期可独立验证）

| 期 | 内容 | 解决 | 风险 |
|----|------|------|------|
| **P0-1** | Bot 提速（C1+C2）：令牌桶 + Client 单例 | 2.3→高 upload/s | 低，纯内部 |
| **P0-2** | 背压重做（H1+C3+C4）：流式上传 + 排队 + Redis 热切 | 500 并发不雪崩 | 中，改主上传路径 |
| **P0-3** | Bot 重试+FloodWait 透传（H2+H3） | 不再整文件失败 | 低 |
| **P0-4** | DB 池扩容+事务（C5+H6） | 无 BUSY/孤儿 | 低 |
| **P1-1** | 连接双轨路由+Banned 隔离+User keepalive（§4.1+H4+H5） | 自愈 | 中，新模块 |
| **P1-2** | 限流重做分类（H8） | 公平 | 低 |
| **P1-3** | 进程守护（tini+shutdown timeout） | 7x24 | 低 |
| **P2-1** | S3 降级后端（H7） | 全挂兜底 | 中，新依赖 |
| **P2-2** | 多用户盲点（§6.1-6.4） | 公网开放 | 高，需用户定身份模型 |
| **P2-3** | Range 下载 + PipedStream（借 K-Vault/tgDrive） | 大文件体验 | 中 |

---

## 8. 待用户决策项

1. **用户身份模型**（§6.2）：匿名 token / 注册账号 / API Key 分租户？—— 影响 P2-2 全部设计。
2. **S3 后端**（§4.5）：现在实现还是先「全挂 503+告警」后置？—— 影响 P2-1 是否进 P0。
3. **病毒扫描**（§6.1）：是否引入 ClamAV sidecar？—— 影响部署复杂度。
4. **审核机制**（§6.3）：匿名上传是否默认「未发布待审核」？—— 影响分享流程。
5. **配额策略**（§6.4）：per-tenant 并发上限具体值？

---

## 9. 不做的事（YAGNI）

- 不重写为微服务（单 binary 够 500 并发）。
- 不引入消息队列（SQLite + Redis 够，S3 回迁用内部 job）。
- 不做多频道分片（先监控消息量，触发阈值再分）。
- 不动桌面 Tauri 代码（用户已说无所谓）。
- 不加 Actor 模型（C1 修好后 Pentaract Actor 收益递减，P1 可选）。

---

*本方案承接三轮侦察结论。审批后按 §7 分期实施，每期 TDD + 缺口复测。*
