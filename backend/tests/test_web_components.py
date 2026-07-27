"""Smoke tests for v8 Web Components (TASK-P1-02).

Verifies that <td-file-card> and <td-toast-host> register as custom elements
and render the expected DOM. Loads a self-contained HTML fixture via file://
so the component scripts run in a real browser context (set_content with
inline scripts is unreliable for large script bodies).

Skipped automatically when Playwright/chromium is unavailable.
"""

from __future__ import annotations

import tempfile
from pathlib import Path

import pytest

try:
    from playwright.sync_api import sync_playwright
    _HAS_PW = True
except ImportError:
    _HAS_PW = False

pytestmark = pytest.mark.skipif(not _HAS_PW, reason="playwright not installed")

_REPO = Path(__file__).resolve().parents[2]
_COMPONENTS = _REPO / "deploy" / "web" / "components"


def _fixture_dir() -> Path:
    """Build a temp dir with index.html + component scripts for file:// loading."""
    fc = (_COMPONENTS / "td-file-card.js").read_text(encoding="utf-8")
    toast = (_COMPONENTS / "td-toast.js").read_text(encoding="utf-8")
    html = (
        "<!DOCTYPE html><html><head><meta charset='utf-8'></head><body>"
        "<td-file-card data-id='100' data-name='report.pdf' data-size='4096' "
        "data-mime='application/pdf' data-category='document' "
        "data-download-url='/d/100'></td-file-card>"
        "<td-toast-host></td-toast-host>"
        "<script src='td-file-card.js'></script>"
        "<script src='td-toast.js'></script>"
        "</body></html>"
    )
    d = Path(tempfile.mkdtemp())
    (d / "index.html").write_text(html, encoding="utf-8")
    (d / "td-file-card.js").write_text(fc, encoding="utf-8")
    (d / "td-toast.js").write_text(toast, encoding="utf-8")
    return d


@pytest.fixture(scope="module")
def page():
    with sync_playwright() as p:
        browser = p.chromium.launch(headless=True)
        context = browser.new_context()
        page = context.new_page()
        yield page
        context.close()
        browser.close()


def _goto_fixture(page) -> Path:
    d = _fixture_dir()
    url = "file:///" + str(d / "index.html").replace("\\", "/")
    page.goto(url, wait_until="load")
    return d


def test_td_file_card_registers_and_renders(page):
    """<td-file-card> registers and renders thumbnail + meta."""
    _goto_fixture(page)
    page.wait_for_selector(".td-file-card", timeout=5000)
    thumb = page.query_selector(".td-file-card__thumb")
    assert thumb is not None
    assert "/api/v1/files/100/thumb" in thumb.get_attribute("src")
    name = page.query_selector(".td-file-card__name")
    assert name is not None
    assert name.inner_text() == "report.pdf"


def test_td_file_card_lightbox_opens_on_click(page):
    """Clicking the card opens the lightbox preview."""
    _goto_fixture(page)
    page.wait_for_selector(".td-file-card", timeout=5000)
    page.click(".td-file-card")
    page.wait_for_selector(".td-lightbox:not([hidden])", timeout=5000)
    embed = page.query_selector(".td-lightbox__content embed")
    assert embed is not None


def test_td_toast_host_renders_on_event(page):
    """Dispatching td:toast renders a toast with the message."""
    _goto_fixture(page)
    # Wait for the custom element to be defined (host may be empty/hidden).
    page.wait_for_function(
        "() => customElements.get('td-toast-host') !== undefined", timeout=5000
    )
    page.evaluate(
        "() => window.dispatchEvent(new CustomEvent('td:toast', "
        "{ detail: { type: 'success', message: 'hello' } }))"
    )
    page.wait_for_selector(".td-toast", timeout=5000)
    msg = page.query_selector(".td-toast__msg")
    assert msg is not None
    assert msg.inner_text() == "hello"
