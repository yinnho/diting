// FontFace.width (CSS Fonts 4's font-stretch rename, new in 146..157) + the exposed FontFaceSet interface. Runs in Chrome AND the engine.
(function(){var R={};function t(n,f){try{R[n]=f()}catch(e){R[n]='THROW '+e.name+': '+e.message}}
t('def',function(){var f=new FontFace('a','local(x)');return [f.width,f.stretch]});
t('setStretch',function(){var f=new FontFace('a','local(x)');f.stretch='condensed';return [f.width,f.stretch]});
t('setWidth',function(){var f=new FontFace('a','local(x)');f.width='expanded';return [f.width,f.stretch]});
t('setWidthPct',function(){var f=new FontFace('a','local(x)');f.width='75%';return [f.width,f.stretch]});
t('descStretch',function(){var f=new FontFace('a','local(x)',{stretch:'condensed'});return [f.width,f.stretch,f.status]});
t('descWidth',function(){var f=new FontFace('a','local(x)',{width:'expanded'});return [f.width,f.stretch,f.status]});
t('descBoth',function(){var f=new FontFace('a','local(x)',{width:'expanded',stretch:'condensed'});return [f.width,f.stretch]});
t('badWidth',function(){var f=new FontFace('a','local(x)');try{f.width='zzz'}catch(e){return [e.name,e.message,f.width]}return ['ok',f.width]});
t('badDescWidth',function(){var f=new FontFace('a','local(x)',{width:'zzz'});return [f.status,f.width]});
t('sameAccessor',function(){var a=Object.getOwnPropertyDescriptor(FontFace.prototype,'width'),b=Object.getOwnPropertyDescriptor(FontFace.prototype,'stretch');return [a.get===b.get,a.get.name,a.set.name,a.enumerable,a.configurable]});
t('brand',function(){try{Object.getOwnPropertyDescriptor(FontFace.prototype,'width').get.call({})}catch(e){return e.name+': '+e.message}});
t('setBrand',function(){try{Object.getOwnPropertyDescriptor(FontFace.prototype,'width').set.call({},'x')}catch(e){return e.name+': '+e.message}});
t('ffsCtor',function(){var P=Object.getPrototypeOf(document.fonts);var d=Object.getOwnPropertyDescriptor(P,'constructor');return [d.value===FontFaceSet,d.writable,d.enumerable,d.configurable,FontFaceSet.length,FontFaceSet.name,document.fonts instanceof FontFaceSet]});
t('ffsNew',function(){try{new FontFaceSet([]);return 'ok'}catch(e){return e.name+': '+e.message}});
t('ffsWinDesc',function(){var d=Object.getOwnPropertyDescriptor(window,'FontFaceSet');return [d.writable,d.enumerable,d.configurable]});
return JSON.stringify(R)})()
