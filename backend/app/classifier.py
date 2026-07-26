"""File classifier — categorizes files by extension and MIME type.

Provides smart file classification for the frontend gallery/filter UI.
Used by the files router to add category metadata to file listings.
"""

from __future__ import annotations

from pathlib import Path
from typing import Optional

CATEGORY_RULES: dict[str, dict[str, set[str]]] = {
    "image": {
        "extensions": {".jpg", ".jpeg", ".png", ".gif", ".webp", ".svg", ".bmp", ".ico", ".tiff", ".tif", ".heic", ".heif"},
        "mime_prefix": "image/",
    },
    "video": {
        "extensions": {".mp4", ".mkv", ".webm", ".mov", ".avi", ".flv", ".wmv", ".m4v", ".mpg", ".mpeg", ".3gp"},
        "mime_prefix": "video/",
    },
    "document": {
        "extensions": {".pdf", ".doc", ".docx", ".xls", ".xlsx", ".ppt", ".pptx", ".odt", ".ods", ".odp", ".rtf", ".pages", ".numbers", ".key"},
        "mime_prefix": "application/",
    },
    "audio": {
        "extensions": {".mp3", ".flac", ".wav", ".ogg", ".m4a", ".aac", ".wma", ".opus", ".aiff", ".alac"},
        "mime_prefix": "audio/",
    },
    "archive": {
        "extensions": {".zip", ".rar", ".7z", ".tar", ".gz", ".bz2", ".xz", ".tgz", ".tbz2", ".txz", ".zst"},
        "mime_prefix": "",
    },
    "code": {
        "extensions": {".py", ".js", ".ts", ".tsx", ".jsx", ".go", ".rs", ".java", ".cpp", ".c", ".h", ".hpp", ".rb", ".php", ".swift", ".kt", ".scala", ".sh", ".bash", ".ps1", ".sql", ".html", ".css", ".scss", ".less", ".json", ".xml", ".yaml", ".yml", ".toml", ".ini", ".cfg", ".env", ".md", ".rst"},
        "mime_prefix": "text/",
    },
    "font": {
        "extensions": {".ttf", ".otf", ".woff", ".woff2", ".eot"},
        "mime_prefix": "font/",
    },
}

CATEGORY_LABELS: dict[str, str] = {
    "image": "图片",
    "video": "视频",
    "document": "文档",
    "audio": "音频",
    "archive": "压缩包",
    "code": "代码",
    "font": "字体",
    "other": "其他",
}

CATEGORY_ICONS: dict[str, str] = {
    "image": "🖼",
    "video": "🎬",
    "document": "📄",
    "audio": "🎵",
    "archive": "📦",
    "code": "💻",
    "font": "🔤",
    "other": "📁",
}


def classify_file(filename: str, mime_type: Optional[str] = None) -> str:
    """Classify a file into a category based on extension and MIME type.

    Returns the category key (e.g. 'image', 'video', 'document', 'other').
    """
    ext = Path(filename).suffix.lower()
    for category, rules in CATEGORY_RULES.items():
        if ext in rules["extensions"]:
            return category
        if mime_type and rules["mime_prefix"] and mime_type.startswith(rules["mime_prefix"]):
            return category
    return "other"


def classify_batch(
    files: list[tuple[str, Optional[str]]]
) -> dict[str, int]:
    """Classify a batch of files and return category counts.

    Args:
        files: list of (filename, mime_type) tuples.

    Returns:
        dict mapping category key to count.
    """
    counts: dict[str, int] = {}
    for filename, mime_type in files:
        cat = classify_file(filename, mime_type)
        counts[cat] = counts.get(cat, 0) + 1
    return counts


def category_label(category: str) -> str:
    """Return the human-readable label for a category."""
    return CATEGORY_LABELS.get(category, "其他")


def category_icon(category: str) -> str:
    """Return the emoji icon for a category."""
    return CATEGORY_ICONS.get(category, "📁")