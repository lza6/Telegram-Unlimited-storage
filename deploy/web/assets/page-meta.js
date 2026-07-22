/* Sets absolute canonical / og:url / og:image — keep logic aligned with app/src/lib/webMetaPure.ts */
(function () {
  function abs(path) {
    var origin = location.origin.replace(/\/$/, '');
    if (/^https?:\/\//i.test(path)) return path;
    return origin + (path.charAt(0) === '/' ? path : '/' + path);
  }
  var canonical = document.querySelector('link[data-td-canonical]');
  var path =
    (canonical && canonical.getAttribute('data-td-path')) ||
    location.pathname ||
    '/';
  if (canonical) {
    canonical.setAttribute('href', abs(path));
  }
  var ogUrl = document.querySelector('meta[data-td-og-url]');
  if (ogUrl) {
    ogUrl.setAttribute('content', abs(path));
  }
  var ogImage = document.querySelector('meta[data-td-og-image]');
  if (ogImage) {
    ogImage.setAttribute('content', abs('/assets/logo.svg'));
  }
})();
