/**
 * Theme toggle — dark/light mode with system preference detection.
 * Persists choice in localStorage; falls back to prefers-color-scheme.
 * Include on every page that has a #theme-toggle button.
 */
(function () {
  'use strict';

  var STORAGE_KEY = 'td_theme';

  function getSystemTheme() {
    return window.matchMedia && window.matchMedia('(prefers-color-scheme: light)').matches
      ? 'light'
      : 'dark';
  }

  function getSavedTheme() {
    try {
      return localStorage.getItem(STORAGE_KEY);
    } catch (e) {
      return null;
    }
  }

  function applyTheme(theme) {
    var html = document.documentElement;
    if (theme === 'light') {
      html.setAttribute('data-theme', 'light');
    } else {
      html.removeAttribute('data-theme');
    }
    updateToggleButton(theme);
  }

  function updateToggleButton(theme) {
    var btn = document.getElementById('theme-toggle');
    if (!btn) return;
    var icon = btn.querySelector('.theme-toggle-icon');
    var label = btn.querySelector('.theme-toggle-label');
    if (theme === 'light') {
      if (icon) icon.textContent = '☀️';
      if (label) label.textContent = '亮色模式';
    } else {
      if (icon) icon.textContent = '🌙';
      if (label) label.textContent = '暗色模式';
    }
  }

  function currentTheme() {
    return document.documentElement.getAttribute('data-theme') === 'light' ? 'light' : 'dark';
  }

  function toggle() {
    var next = currentTheme() === 'light' ? 'dark' : 'light';
    applyTheme(next);
    try {
      localStorage.setItem(STORAGE_KEY, next);
    } catch (e) {
      /* storage unavailable — theme still works for this session */
    }
  }

  // Apply saved or system theme immediately (before DOMContentLoaded paint).
  var saved = getSavedTheme();
  applyTheme(saved || getSystemTheme());

  // Listen for system theme changes when no explicit choice is saved.
  if (window.matchMedia) {
    window.matchMedia('(prefers-color-scheme: light)').addEventListener('change', function (e) {
      if (!getSavedTheme()) {
        applyTheme(e.matches ? 'light' : 'dark');
      }
    });
  }

  // Bind toggle button once DOM is ready.
  function bind() {
    var btn = document.getElementById('theme-toggle');
    if (btn) {
      btn.addEventListener('click', toggle);
    }
  }

  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', bind);
  } else {
    bind();
  }
})();
