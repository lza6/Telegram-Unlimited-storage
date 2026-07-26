/* PWA Service Worker registration — include on every page before </body> */
(function () {
  'use strict';
  if ('serviceWorker' in navigator) {
    window.addEventListener('load', function () {
      navigator.serviceWorker.register('/service-worker.js', { scope: '/' })
        .then(function (reg) {
          console.log('SW registered:', reg.scope);
          // Listen for updates
          reg.addEventListener('updatefound', function () {
            var newWorker = reg.installing;
            if (!newWorker) return;
            newWorker.addEventListener('statechange', function () {
              if (newWorker.state === 'installed' && navigator.serviceWorker.controller) {
                // New version available — notify user
                if (typeof TdToast !== 'undefined') {
                  TdToast.info('新版本已就绪，刷新页面即可使用');
                }
              }
            });
          });
        })
        .catch(function (err) {
          console.warn('SW registration failed:', err);
        });
    });
  }
})();