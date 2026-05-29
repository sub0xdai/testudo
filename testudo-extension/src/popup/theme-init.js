/** @anchor ui:ext-popup:theme-init
 * @tags ui
 */

let t = localStorage.getItem('testudo-theme');
if (t === 'soft-dark') { t = 'amoled'; localStorage.setItem('testudo-theme', t); }
if (t && t !== 'amoled') document.documentElement.setAttribute('data-theme', t);
if (typeof chrome !== 'undefined' && chrome.storage) {
  chrome.storage.local.get('testudo-theme', function(r) {
    var s = r['testudo-theme'];
    if (s && s !== 'amoled') document.documentElement.setAttribute('data-theme', s);
    else if (s === 'amoled') document.documentElement.removeAttribute('data-theme');
  });
}
