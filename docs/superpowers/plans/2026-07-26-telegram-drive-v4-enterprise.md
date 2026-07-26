# Telegram Drive v4.0.0-python 企业版实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 实现 Telegram Drive 企业级分布式与 AI 原生化能力，闭环结构化错误、指标监控、WebDAV 暴力破解防护、异步存储及多端体验。

**架构：**
1. 异常与指标切面化：全局捕获 `TelegramDriveError` 并暴露 Prometheus 监控端点。
2. 异步存储（aiosqlite）：读写锁分离，消除同步阻塞。
3. 安全机制轮换：支持 key 版本化的预签名 URL 与防爆锁注入。
4. UI/UX 精品化：前端无刷新 htmx SPA，适配 PWA 离线 Service Worker，支持拼音搜索。

**技术栈：** Python 3.11, FastAPI, aiosqlite, prometheus_client, htmx, Alpine.js

---

### 任务 1：结构化异常拦截闭环 (V3-01)

**文件：**
- 修改：`backend/app/main.py`
- 修改：`backend/app/routers/files.py`
- 测试：`backend/tests/test_errors.py`

- [ ] **步骤 1：编写全局异常测试**

在 `backend/tests/test_errors.py` 中增加对路由抛出 structured 异常的拦截验证：
```python
def test_global_exception_handler(client):
    response = client.get("/api/v1/files/nonexistent_id_for_test")
    assert response.status_code == 404
    data = response.json()
    assert data["error"]["code"] == "NOT_FOUND"
```

- [ ] **步骤 2：运行测试验证失败**

运行：`pytest backend/tests/test_errors.py -k test_global_exception_handler`
预期：FAIL (返回 500 或 400 字符串)

- [ ] **步骤 3：编写 main.py 拦截器实现**

在 `backend/app/main.py` 的 `create_app` 之前，添加拦截器并在 `create_app` 内部进行注册：
```python
from app.errors import TelegramDriveError

@app.exception_handler(TelegramDriveError)
async def telegram_drive_exception_handler(request: Request, exc: TelegramDriveError):
    return JSONResponse(
        status_code=exc.status_code,
        content=exc.as_dict()
    )
```

- [ ] **步骤 4：运行测试验证通过**

运行：`pytest backend/tests/test_errors.py`
预期：PASS

- [ ] **步骤 5：Commit**

```bash
git add backend/app/main.py backend/app/routers/files.py backend/tests/test_errors.py
git commit -m "feat(auth): integrate structured exception handling globally"
```

---

### 任务 2：Prometheus 监控端点闭环 (V3-02)

**文件：**
- 修改：`backend/app/routers/health.py`
- 修改：`backend/app/main.py`
- 测试：`backend/tests/test_metrics.py`

- [ ] **步骤 1：编写 Metrics 响应测试**

```python
def test_metrics_endpoint(client):
    response = client.get("/api/v1/metrics")
    assert response.status_code == 200
    assert b"telegram_drive_requests_total" in response.content
```

- [ ] **步骤 2：运行测试验证失败**

运行：`pytest backend/tests/test_metrics.py`
预期：FAIL (返回 404)

- [ ] **步骤 3：实现端点并注入中间件**

在 `backend/app/routers/health.py` 中：
```python
from app.metrics import get_metrics

@router.get("/metrics")
async def metrics_endpoint():
    return Response(content=get_metrics(), media_type="text/plain")
```
在 `backend/app/main.py` 中添加监控中间件记录吞吐与时长：
```python
from app.metrics import get_registry

@app.middleware("http")
async def metrics_middleware(request: Request, call_next):
    start = time.time()
    response = await call_next(request)
    dur = time.time() - start
    reg = get_registry()
    reg.requests_total.labels(
        method=request.method,
        path=request.url.path,
        status_code=str(response.status_code)
    ).inc()
    reg.request_duration_seconds.labels(
        method=request.method,
        path=request.url.path
    ).observe(dur)
    return response
```

- [ ] **步骤 4：运行测试验证通过**

运行：`pytest backend/tests/test_metrics.py`
预期：PASS

- [ ] **步骤 5：Commit**

```bash
git add backend/app/routers/health.py backend/app/main.py backend/tests/test_metrics.py
git commit -m "feat(ops): enable prometheus HTTP metrics endpoint and middleware"
```

---

### 任务 3：下载签名密钥多版本轮换 (V3-10)

**文件：**
- 修改：`backend/app/config.py`
- 修改：`backend/app/security.py`
- 修改：`backend/app/links.py`
- 测试：`backend/tests/test_api_integration.py`

- [ ] **步骤 1：编写多版本签名测试**

```python
def test_key_rotation_signing():
    secrets = ["new_active_key", "old_retired_key"]
    payload = {"message_id": 123}
    token = sign_url(payload, secrets, key_id=0)
    assert token.startswith("v4.0.")
    # 验证使用旧 key 签名的也能解密
    old_token = sign_url(payload, secrets, key_id=1)
    decoded = unsign_url(old_token, secrets)
    assert decoded["message_id"] == 123
```

- [ ] **步骤 2：运行测试验证失败**

运行：`pytest backend/tests/test_api_integration.py -k test_key_rotation_signing`
预期：FAIL (NameError 或 AttributeError)

- [ ] **步骤 3：实现 key_id 版本化签名**

在 `backend/app/security.py` 中编写 `sign_url` 与 `unsign_url`：
```python
import itsdangerous
from itsdangerous import BadSignature

def sign_url(payload: dict, secrets: list[str], key_id: int = 0) -> str:
    s = itsdangerous.URLSafeSerializer(secrets[key_id])
    return f"v4.{key_id}.{s.dumps(payload)}"

def unsign_url(token: str, secrets: list[str]) -> dict:
    parts = token.split(".", 2)
    key_id = int(parts[1])
    s = itsdangerous.URLSafeSerializer(secrets[key_id])
    return s.loads(parts[2])
```

- [ ] **步骤 4：运行测试验证通过**

运行：`pytest backend/tests/test_api_integration.py`
预期：PASS

- [ ] **步骤 5：Commit**

```bash
git add backend/app/config.py backend/app/security.py backend/app/links.py
git commit -m "feat(security): support key rotation and versioning for pre-signed URLs"
```

---

### 任务 4：WebDAV Basic Auth 暴力破解防护 (V3-11)

**文件：**
- 修改：`backend/app/routers/webdav.py`
- 修改：`backend/app/auth.py`
- 测试：`backend/tests/test_webdav.py`

- [ ] **步骤 1：编写 WebDAV 锁机制测试**

```python
def test_webdav_brute_force_lock(client):
    # 连续失败 9 次
    for _ in range(9):
        client.get("/webdav/", headers={"Authorization": "Basic wrong_credentials"})
    # 第 10 次预期返回 429 Locked Out
    response = client.get("/webdav/", headers={"Authorization": "Basic wrong_credentials"})
    assert response.status_code == 429
```

- [ ] **步骤 2：运行测试验证失败**

运行：`pytest backend/tests/test_webdav.py`
预期：FAIL (全部返回 401 Unauthorized)

- [ ] **步骤 3：在 WebDAV 路由拦截验证失败计数**

在 `backend/app/routers/webdav.py` 的授权前，调用 `authenticator.guard.check(ip)`。若凭证错误，调用 `guard.record_failure(ip)`。

- [ ] **步骤 4：运行测试验证通过**

运行：`pytest backend/tests/test_webdav.py`
预期：PASS

- [ ] **步骤 5：Commit**

```bash
git add backend/app/routers/webdav.py backend/app/auth.py backend/tests/test_webdav.py
git commit -m "security(webdav): integrate AccessGuard to prevent brute-force on Basic Auth"
```

---

### 任务 5：aiosqlite 异步与连接隔离层实现 (V3-05)

**文件：**
- 修改：`backend/app/storage.py`
- 测试：`backend/tests/test_storage_extended.py`

- [ ] **步骤 1：编写异步并发查询测试**

```python
@pytest.mark.asyncio
async def test_concurrent_read_no_deadlock():
    storage = AsyncStorage(db_path=":memory:")
    # 模拟并发读写
```

- [ ] **步骤 2：运行测试验证失败**

运行：`pytest backend/tests/test_storage_extended.py`
预期：FAIL

- [ ] **步骤 3：重构存储类支持异步 aiosqlite 并发模型**

在 `backend/app/storage.py` 中引入 `aiosqlite` 连接，隔离读写。只读连接开启 `PRAGMA query_only=ON`，写锁执行 `asyncio.Lock()`。

- [ ] **步骤 4：运行测试验证通过**

运行：`pytest backend/tests/test_storage_extended.py`
预期：PASS

- [ ] **步骤 5：Commit**

```bash
git add backend/app/storage.py
git commit -m "perf(db): refactor Storage layer using aiosqlite read-write isolation"
```

---

### 任务 6：FTS5 拼音搜索与去重新规 (V3-16)

**文件：**
- 修改：`backend/app/storage.py`
- 修改：`backend/app/routers/files.py`
- 测试：`backend/tests/test_api_integration.py`

- [ ] **步骤 1：编写全文检索测试**

```python
def test_pinyin_fts_search():
    # 测试通过拼音缩写 xl 搜索 寻雷
```

- [ ] **步骤 2：运行测试验证失败**

预期：FAIL

- [ ] **步骤 3：在 DB 初始化脚本中建立 FTS5 虚拟表并同步拼音首字母**

在 `storage.py` 的 Schema 脚本中创建 `file_fts`，并引入拼音缩写和全拼转换算法写入。

- [ ] **步骤 4：运行测试验证通过**

运行：`pytest backend/tests/test_api_integration.py`
预期：PASS

- [ ] **步骤 5：Commit**

```bash
git add backend/app/storage.py backend/app/routers/files.py
git commit -m "feat(search): implement FTS5 search with pinyin abbreviation matching"
```

---

### 任务 7：前端 htmx SPA 骨架屏与通知全局集成 (V3-23)

**文件：**
- 修改：`deploy/web/*.html`
- 修改：`deploy/web/assets/admin.css`

- [ ] **步骤 1：集成 htmx 静态库与 CSS 样式**

在所有的 HTML 文件 `<head>` 区域引入 htmx:
```html
<script src="https://unpkg.com/htmx.org@1.9.10"></script>
```
在 `admin.css` 中设计骨架屏动画：
```css
@keyframes shimmer {
  0% { background-position: -200% 0; }
  100% { background-position: 200% 0; }
}
.skeleton-loader {
  background: linear-gradient(90deg, var(--surface-1) 25%, var(--surface-2) 50%, var(--surface-1) 75%);
  background-size: 200% 100%;
  animation: shimmer 1.5s infinite;
}
```

- [ ] **步骤 2：注册全局通知组件**

把 `notifications.js` 作为默认载入项，改写路由导航。

- [ ] **步骤 3：本地渲染验证**

预期：在网络延迟加载时呈现骨架屏，通知均调用 `TdToast` 库。

- [ ] **步骤 4：Commit**

```bash
git add deploy/web/
git commit -m "ui(spa): migrate layout to htmx SPA with shimmer skeleton loaders"
```

---

### 任务 8：PWA Service Worker 与快捷键系统 (V3-25)

**文件：**
- 创建：`deploy/web/manifest.json`
- 创建：`deploy/web/sw.js`
- 创建：`deploy/web/assets/keyboard.js`

- [ ] **步骤 1：编写键盘快捷键捕获脚本**

在 `keyboard.js` 中拦截按键 `?` 弹出键盘指令面板。

- [ ] **步骤 2：注册 Service Worker 并测试离线缓存**

- [ ] **步骤 3：验证 PWA 完整度**

预期：Chrome 浏览器地址栏出现“安装为应用”标志，离线状态下静态页面可加载。

- [ ] **步骤 4：Commit**

```bash
git add deploy/web/manifest.json deploy/web/sw.js deploy/web/assets/keyboard.js
git commit -m "feat(ux): support PWA installer and keyboard shortcut helper panels"
```

---

### 任务 9：旧产物清理与架构收尾 (V3-35)

**文件：**
- 删除：`app/src-tauri/`
- 删除：`app/src/`
- 删除：根目录无用备份 `*.tmp` 和旧批处理脚本

- [ ] **步骤 1：安全移除遗留 Rust 与 React 依赖文件夹**

- [ ] **步骤 2：清除未通过 mypy 静态扫描的旧 Python 脚本**

- [ ] **步骤 3：运行完整的 pytest 检查保障 90%+ 覆盖率**

- [ ] **步骤 4：Commit**

```bash
git commit -m "chore(cleanup): prune deprecated rust tauri and react frontend codebase"
```
