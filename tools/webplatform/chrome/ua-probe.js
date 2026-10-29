(async function () {
  var n = navigator, R = {};
  R.userAgent = n.userAgent; R.appVersion = n.appVersion; R.platform = n.platform; R.vendor = n.vendor; R.productSub = n.productSub;
  R.language = n.language; R.languages = n.languages.join(','); R.webdriver = n.webdriver; R.pdfViewerEnabled = n.pdfViewerEnabled;
  R.hardwareConcurrency = n.hardwareConcurrency; R.deviceMemory = n.deviceMemory; R.maxTouchPoints = n.maxTouchPoints;
  R.pluginsLen = n.plugins ? n.plugins.length : null; R.mimeTypesLen = n.mimeTypes ? n.mimeTypes.length : null;
  if (n.userAgentData) {
    R.uad_brands = JSON.stringify(n.userAgentData.brands); R.uad_mobile = n.userAgentData.mobile; R.uad_platform = n.userAgentData.platform;
    try { var h = await n.userAgentData.getHighEntropyValues(['architecture','bitness','brands','fullVersionList','model','platformVersion','uaFullVersion','wow64','formFactors']); R.uad_high = JSON.stringify(h); } catch (e) { R.uad_high = 'ERR ' + e; }
  } else R.uad = 'absent';
  R.chromeKeys = window.chrome ? Object.keys(window.chrome).sort().join(',') : null;
  try { R.tz = Intl.DateTimeFormat().resolvedOptions().timeZone; } catch (e) {}
  return JSON.stringify(R);
})()
