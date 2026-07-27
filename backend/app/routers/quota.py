"""Re-export of quota router for app.main include_router.

The actual router lives in app.quota to avoid circular imports with app.rbac.
"""

from __future__ import annotations

from ..quota import router

__all__ = ["router"]
