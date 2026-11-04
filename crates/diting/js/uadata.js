(function () { // @@SCOPE

// fp-diff-1014 #11: Chrome's navigator.userAgentData has NavigatorUAData on
// its prototype chain — a plain object literal reads as constructor "Object"
// to the cheapest proto walk.
// 300015 P6: the interface is exposed on window (`new` → Illegal
// constructor), members sit on the prototype in Chrome's order (brands,
// mobile, platform, getHighEntropyValues, toJSON, constructor LAST —
// reorder pass at the bottom), and the instance carries ZERO own names
// (gOPN = []). mobile used to be a constructor-stashed own prop: hidden
// from Object.keys but visible to getOwnPropertyNames, and missing from
// the prototype. Desktop persona → always false, like Chrome on macOS.
class NavigatorUAData {
  constructor() { throw new TypeError("Failed to construct 'NavigatorUAData': Illegal constructor"); }
  get brands() {
    return __ditingBrandList(Number(__ditingChromeMajorFromUA()));
  }
  get mobile() { return false; }
  get platform() { return __ditingUADataPlatformFromUA(); }
  getHighEntropyValues(hints) {
    const plat = __ditingUADataPlatformFromUA();
    // Device faces come from the same persona draw as the WebGL renderer —
    // Apple M3 + architecture "x86" was a three-machines-in-one tell.
    const dev = _getFp();
    const maj = __ditingChromeMajorFromUA();
    // Majors outside the table are unsupported personas: reuse the nearest known major
    // instead of extrapolating a build that never shipped.
    const build = __DITING_FULL_BUILD[maj] || __ditingNearestBuild(Number(maj));
    const full = __ditingBrandList(Number(maj)).map(function (b) {
      return { brand: b.brand, version: b.brand === "Chromium" || b.brand === "Google Chrome" ? maj + ".0." + build : b.version + ".0.0.0" };
    });
    return Promise.resolve({
      architecture: dev.arch,
      bitness: dev.bitness,
      brands: __ditingBrandList(Number(maj)),
      fullVersionList: full,
      mobile: false,
      model: "",
      platform: plat,
      platformVersion: dev.platformVersion,
      uaFullVersion: maj + ".0." + build,
      wow64: false,
    });
  }
  toJSON() { return {brands:this.brands,mobile:this.mobile,platform:this.platform}; }
}
globalThis.NavigatorUAData = NavigatorUAData;


// (300015 P6) WebIDL layout: constructor LAST — the reorder pass runs
// here now (it used to live in bootstrap's bottom cluster, which loads
// before this file installs the class).
  __ditingReorderProto(NavigatorUAData.prototype, ['brands','mobile','platform','getHighEntropyValues','toJSON','constructor']);

// @@EXPORTS-BEGIN
// @@EXPORTS-END
}).call(globalThis); // @@SCOPE-END
