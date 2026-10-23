// Disposable virtual-host-only fixture: no credentials or network provider.
const params = new URLSearchParams(location.search);
const redirect = params.get('redirect_uri');
const state = params.get('state');
const acceptedRedirect = 'https://synthetic.cos.test/oauth-callback.html';
if (redirect !== acceptedRedirect || !state || state.length > 128) {
  document.querySelector('#approve').disabled = true;
  document.querySelector('#deny').disabled = true;
} else {
  for (const [button, approved] of [['approve', true], ['deny', false]]) {
    document.querySelector(`#${button}`).addEventListener('click', () => {
      const callback = new URL(acceptedRedirect);
      callback.searchParams.set('state', state);
      if (approved) callback.searchParams.set('code', 'synthetic-code-only');
      else callback.searchParams.set('error', 'access_denied');
      location.assign(callback.href);
    });
  }
  // Page title can be observed before this deferred external script has wired
  // its click handlers. The native probe must wait for this readiness signal.
  document.documentElement.dataset.mockConsentReady = 'ready';
}
