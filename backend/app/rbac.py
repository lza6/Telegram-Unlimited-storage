"""Role-Based Access Control (TASK-P1-03).

Provides scope-based authorization for multi-tenant deployments.
"""

from __future__ import annotations

from typing import TYPE_CHECKING

from fastapi import HTTPException, Request

from .auth import CallerIdentity

if TYPE_CHECKING:
    from .state import AppState

# Canonical scope set for the system.
SCOPES = frozenset({"read", "write", "delete", "share", "admin"})

# Scope hierarchy: higher scopes imply lower ones.
_SCOPE_IMPLIES = {
    "admin": {"read", "write", "delete", "share", "admin"},
    "delete": {"read", "write", "delete"},
    "write": {"read", "write"},
    "share": {"read", "share"},
    "read": {"read"},
}


def _effective_scopes(tenant_scopes: list[str]) -> set[str]:
    """Expand implied scopes (e.g., 'admin' implies all others)."""
    result: set[str] = set()
    for s in tenant_scopes:
        result |= _SCOPE_IMPLIES.get(s, {s})
    return result


def has_scope(identity: CallerIdentity, state: AppState, required_scope: str) -> bool:
    """Return True if the caller's scopes include the required scope."""
    if identity.kind in ("console", "api"):
        return True  # console / single-tenant api: full access

    if identity.kind != "tenant":
        return False

    tenant_scopes = state.storage.get_tenant_scopes(identity.tenant_id)
    if not tenant_scopes:
        return True  # empty scopes = full access (backward compat)

    effective = _effective_scopes(tenant_scopes)
    return required_scope in effective


def require_scope(required_scope: str):
    """FastAPI dependency: require a specific scope for the caller.

    Console and single-tenant API callers always pass.
    Multi-tenant callers must have the scope in their tenant's scopes list.
    """
    if required_scope not in SCOPES:
        raise ValueError(f"unknown scope: {required_scope}")

    def _dep(request: Request) -> CallerIdentity:
        state = request.app.state.app
        identity = state.authenticator.require_auth(request)
        if not has_scope(identity, state, required_scope):
            raise HTTPException(
                status_code=403,
                detail={
                    "code": "FORBIDDEN",
                    "message": f"Scope '{required_scope}' required",
                },
            )
        return identity

    return _dep


def assert_all_routes_scoped(app, protected_prefixes: tuple[str, ...] = ("/api/v1/files",)) -> list[str]:
    """Audit utility: ensure all routes under protected prefixes carry a scope dependency.

    Returns a list of unscoped route paths (empty if all good).
    """
    unscoped: list[str] = []
    for route in app.routes:
        path = getattr(route, "path", "")
        if not any(path.startswith(p) for p in protected_prefixes):
            continue
        methods = getattr(route, "methods", set()) or set()
        # Skip read-only GET routes (they need 'read' but we focus on mutating ones here)
        if methods and "GET" in methods and "POST" not in methods and "DELETE" not in methods and "PUT" not in methods:
            continue
        deps = getattr(route, "dependant", None)
        # Best-effort: if there's no scope dependency, flag it
        has_scope_dep = False
        if deps and hasattr(deps, "dependencies"):
            for d in deps.dependencies:
                call = getattr(d, "call", None)
                if call and "require_scope" in getattr(call, "__qualname__", ""):
                    has_scope_dep = True
                    break
        if not has_scope_dep:
            unscoped.append(f"{','.join(sorted(methods))} {path}")
    return unscoped
