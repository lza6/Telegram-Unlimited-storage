"""Download plumbing tests — Range parsing and Content-Disposition headers."""

from __future__ import annotations

from app.downloads import content_disposition, guess_mime, parse_range_header


class TestParseRangeHeader:
    def test_valid_range(self):
        assert parse_range_header("bytes=0-99", 1000) == (0, 99)

    def test_open_ended(self):
        assert parse_range_header("bytes=500-", 1000) == (500, 999)

    def test_end_clamped_to_total(self):
        assert parse_range_header("bytes=0-9999", 1000) == (0, 999)

    def test_invalid_start_gt_end(self):
        assert parse_range_header("bytes=500-499", 1000) is None

    def test_start_beyond_total(self):
        assert parse_range_header("bytes=1000-2000", 1000) is None

    def test_none_header(self):
        assert parse_range_header(None, 1000) is None

    def test_empty_header(self):
        assert parse_range_header("", 1000) is None

    def test_malformed(self):
        assert parse_range_header("invalid", 1000) is None

    def test_single_byte(self):
        assert parse_range_header("bytes=0-0", 1000) == (0, 0)

    def test_last_byte(self):
        assert parse_range_header("bytes=999-", 1000) == (999, 999)

    def test_zero_total(self):
        assert parse_range_header("bytes=0-0", 0) is None


class TestContentDisposition:
    def test_image_inline(self):
        cd = content_disposition("photo.jpg", "image/jpeg")
        assert cd.startswith("inline")

    def test_video_inline(self):
        cd = content_disposition("clip.mp4", "video/mp4")
        assert cd.startswith("inline")

    def test_audio_inline(self):
        cd = content_disposition("song.mp3", "audio/mpeg")
        assert cd.startswith("inline")

    def test_pdf_inline(self):
        cd = content_disposition("doc.pdf", "application/pdf")
        assert cd.startswith("inline")

    def test_binary_attachment(self):
        cd = content_disposition("data.bin", "application/octet-stream")
        assert cd.startswith("attachment")

    def test_zip_attachment(self):
        cd = content_disposition("archive.zip", "application/zip")
        assert cd.startswith("attachment")

    def test_ascii_filename(self):
        cd = content_disposition("readme.txt", "text/plain")
        assert 'filename="readme.txt"' in cd

    def test_unicode_filename_encoded(self):
        cd = content_disposition("文档.pdf", "application/pdf")
        assert "UTF-8''" in cd

    def test_filename_with_quotes_escaped(self):
        cd = content_disposition('file"name.txt', "text/plain")
        assert 'filename="file_name.txt"' in cd


class TestGuessMime:
    def test_guess_mime_jpeg(self):
        assert guess_mime("photo.jpg") == "image/jpeg"

    def test_guess_mime_png(self):
        assert guess_mime("image.png") == "image/png"

    def test_guess_mime_pdf(self):
        assert guess_mime("document.pdf") == "application/pdf"

    def test_guess_mime_unknown(self):
        assert guess_mime("file.xyz") == "application/octet-stream"

    def test_guess_mime_no_extension(self):
        assert guess_mime("README") == "application/octet-stream"

    def test_guess_mime_json(self):
        assert guess_mime("data.json") == "application/json"

    def test_guess_mime_html(self):
        assert guess_mime("index.html") == "text/html"
