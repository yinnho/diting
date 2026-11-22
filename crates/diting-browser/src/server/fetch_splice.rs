//! Markdown splice probe + text-massage helpers for /fetch, split out of
//! fetch.rs (god-file ratchet, ARCHITECTURE.md §6 P2). Pure data and pure
//! functions; every consumer lives one module over (fetch.rs's
//! rendered_text / do_fetch).

///
/// `document.body.innerText` carries labels but never destinations, yet
/// markdown consumers (agents, list/detail extraction) score named
/// `[label](url)` records — and scoped contracts only count records inside
/// their own page section, so a links appendix at the end of the content
/// scores nothing. This pass walks `a[href]` in DOM order (visible anchors
/// only, computed-style display/visibility memoized up the ancestor chain),
/// finds each label's own position in the body text (whitespace-collapsed
/// view with an index map back into the raw string), and splices the
/// markdown link in place. Labels with no position in the text fall back to
/// a tail appendix.
/// Markdown face of a rendered page, spliced into `b` (the body innerText)
/// at each element's own text position — see `rendered_text`.
///
/// Two splice families share one edit list:
/// - `a[href]` anchors become `[label](url)` at the label's own position, so
///   scoped list sections keep owning their records. Labels drop
///   screen-reader-only runs (`(external, opens in a new tab)`) and hidden
///   anchors are skipped (computed-style walk, memoized with `__dlv`).
/// - `<table>`s become pipe tables that REPLACE the table's own text span
///   (anchored on the collapsed header-row and last-row text), because
///   innerText's cell-per-line face scores as neither a markdown table nor
///   a plain row — and appending them at the end puts them outside the
///   start/end scope most table contracts cut. A table whose span cannot
///   be located falls back to the tail appendix.
///
/// Edits apply back-to-front by raw offset (see the sort); table spans
/// occupy their interval first so link matches inside a table are not
/// spliced into its cells (the pipe table already carries the cell text).
pub(super) const MARKDOWN_SPLICE_JS: &str = r#"
try{if(!window.__dsOrig)window.__dsOrig=b;}catch(e){}
function __srText(n){
  // Parts join with no separator: innerText concatenates inline siblings
  // ("Lana24" + sr-only "(external…)" renders "Lana24(external…)"), and a
  // space-joined anchor can never match the marker-joined text.
  var parts=[];
  (function w(x){for(var i=0;i<x.childNodes.length;i++){var c=x.childNodes[i];
    if(c.nodeType===3){parts.push(c.nodeValue);continue;}
    if(c.nodeType!==1)continue;
    var cl=c.className;if(cl&&cl.baseVal!==undefined)cl=cl.baseVal;
    if(/sr-only|screen.?reader|visually-hidden/i.test(cl||'')||c.getAttribute('aria-hidden')==='true')continue;
    // Inline-hidden parts (tooltips: ej2 plants a display:none span inside
    // every grid header cell) render nowhere and innerText drops them — the
    // extract must too, or labels grow glue ("Order IDPress Alt Down to open
    // filter Menu…") and cell anchors stop matching the rendered body.
    var __st=c.getAttribute('style')||'';
    if(c.hasAttribute('hidden')||/display\s*:\s*none|visibility\s*:\s*(?:hidden|collapse)/i.test(__st))continue;
    w(c);}})(n);
  return parts.join('');
}
function __allText(n){
  // Every text node, sr-only included: the span window must budget what raw
  // innerText actually holds. EUI rows carry ~45 chars of sr-only copy text
  // per link cell; a DOM-visible-only estimate undershoots by hundreds of
  // chars over a 10-row table and the tail anchor then falls outside the
  // window (table gets tailed instead of replaced in place).
  var parts=[];
  (function w(x){for(var i=0;i<x.childNodes.length;i++){var c=x.childNodes[i];
    if(c.nodeType===3){parts.push(c.nodeValue);continue;}
    if(c.nodeType===1)w(c);}})(n);
  return parts.join('');
}
var __marks=[],__tables=[];
var __all=document.querySelectorAll('a[href]');
var __an=Math.min(__all.length,500);
for(var __i=0;__i<__an;__i++){
  var __a=__all[__i];
  var __h='';try{__h=__a.href||'';}catch(e){}
  if(__h.indexOf('http://')!==0&&__h.indexOf('https://')!==0){__h=(__a.getAttribute('href')||'').trim();if(!__h)continue;}
  var __lab=__srText(__a).replace(/\s+/g,' ').trim();
  if(!__lab)continue;
  // Symbol-only labels (#/¶/→ docs copy-anchors) carry no destination text.
  // Splicing them glues the marker onto the line — "[#](url)Server-side
  // paginate and sort" — and section boundaries that key on heading-shaped
  // lines ("^#?\s*Server-side…$") then miss the whole section. Skip the
  // link; the bare marker stays as plain text and the heading line matches.
  if(/^[\p{P}\p{S}\s]+$/u.test(__lab))continue;
  var __el=__a,__hid=false;
  while(__el&&__el.nodeType===1){
    if(__el.__dlv===1)break;
    if(__el.__dlv===2){__hid=true;break;}
    var __cs=null;try{__cs=getComputedStyle(__el);}catch(e){}
    if(__cs&&(__cs.display==='none'||__cs.visibility==='hidden'||__cs.visibility==='collapse')){__el.__dlv=2;__hid=true;break;}
    __el=__el.parentElement;
  }
  if(__hid)continue;
  if(__lab.length>160)__lab=__lab.slice(0,160);
  __marks.push([__lab,__h]);
}
var __TRIM=/^[\s←-⇿⌀-⏿■-◿☀-➿︎️\u200B-\u200F]+|[\s←-⇿⌀-⏿■-◿☀-➿︎️\u200B-\u200F]+$/g;
function __cellText(el){return __srText(el).replace(/\s+/g,' ').trim().replace(__TRIM,'').replace(/\|/g,'\\|');}
function __rowText(cells,trim){var p=[];for(var i=0;i<cells.length;i++){var t=__srText(cells[i]).replace(/\s+/g,' ').trim();p.push(trim?t.replace(__TRIM,''):t);}return p.join(' ');}
function __hiddenTree(el){
  while(el&&el.nodeType===1){
    var cs=null;try{cs=getComputedStyle(el);}catch(e){}
    if(cs&&(cs.display==='none'||cs.visibility==='hidden'||cs.visibility==='collapse'))return true;
    el=el.parentElement;
  }
  return false;
}
function __ownCells(row){
  var cells=[];
  var all=row.querySelectorAll('th,td,[role=cell],[role=columnheader]');
  for(var i=0;i<all.length&&cells.length<40;i++){
    var c=all[i],p=c.parentElement,nested=false;
    while(p&&p!==row){
      if(p.tagName==='TD'||p.tagName==='TH'||p.getAttribute('role')==='cell'){nested=true;break;}
      p=p.parentElement;
    }
    if(!nested)cells.push(c);
  }
  return cells;
}
var __tabs=document.querySelectorAll('table,[role=table],[role=grid]');
var __emitted=0;
for(var __t=0;__t<__tabs.length&&__emitted<20;__t++){
  var __tb=__tabs[__t],__p=__tb.parentElement,__nested=false;
  while(__p){
    if(__p.tagName==='TABLE'||__p.getAttribute('role')==='table'||__p.getAttribute('role')==='grid'){__nested=true;break;}
    __p=__p.parentElement;
  }
  if(__nested||__hiddenTree(__tb))continue;
  var __rows=__tb.querySelectorAll('tr,[role=row]');
  if(!__rows.length)continue;
  var __headCells=__ownCells(__rows[0]);
  function __ci(__c2){var __v=__c2.getAttribute('aria-colindex');return __v?(parseInt(__v,10)||0):0;}
  // ARIA grids (ej2/syncfusion class) carry aria-colindex on header AND
  // data cells while data rows still lead with structural no-index cells
  // (the group indent). Keying columns by index keeps headers aligned with
  // data where plain cell order drifts one over, and drops the structural
  // cells outright. Engage only when both sides actually carry indexes.
  var __hci=0;
  for(var __c=0;__c<__headCells.length;__c++)if(__ci(__headCells[__c])>0)__hci++;
  var __useIndex=false;
  if(__hci>=2){
    for(var __r2=1;__r2<__rows.length&&!__useIndex;__r2++){
      var __q3=__rows[__r2].parentElement,__ih2=false;
      while(__q3&&__q3!==__tb){if(__q3.tagName==='THEAD'){__ih2=true;break;}__q3=__q3.parentElement;}
      if(__ih2)continue;
      var __dc=__ownCells(__rows[__r2]);
      for(var __v2=0;__v2<__dc.length;__v2++)if(__ci(__dc[__v2])>0){__useIndex=true;break;}
    }
  }
  function __place(__cells){
    var __w=0,__out=[],__i2;
    for(__i2=0;__i2<__cells.length;__i2++){var __x=__ci(__cells[__i2]);if(__x>__w)__w=__x;}
    for(__i2=0;__i2<__w;__i2++)__out.push('');
    for(__i2=0;__i2<__cells.length;__i2++){
      var __txt=__cellText(__cells[__i2]);
      if(!__txt)continue;
      var __x2=__ci(__cells[__i2]);
      if(__x2>0&&__x2<=__w)__out[__x2-1]=__txt;else __out.push(__txt);
    }
    return __out;
  }
  var __headers=__useIndex?__place(__headCells):[];
  for(var __c=0;!__useIndex&&__c<__headCells.length;__c++)__headers.push(__cellText(__headCells[__c]));
  if(!__headers.join('').trim())continue;
  var __lines=['| '+__headers.join(' | ')+' |'];
  var __sep=[];for(var __s=0;__s<__headers.length;__s++)__sep.push('---');
  __lines.push('| '+__sep.join(' | ')+' |');
  var __data=0,__span=__allText(__rows[0]).length+600,__lastCells=null;
  for(var __r=1;__r<__rows.length&&__data<100;__r++){
    var __tr=__rows[__r],__q=__tr.parentElement,__inHead=false;
    while(__q&&__q!==__tb){
      if(__q.tagName==='THEAD'){__inHead=true;break;}
      __q=__q.parentElement;
    }
    if(__inHead)continue;
    var __cells=__ownCells(__tr);
    if(!__cells.length)continue;
    var __vals;
    if(__useIndex){
      // Caption/detail rows carry no indexed cells at all; a placement that
      // comes out empty is the grid's own blank spacer row.
      var __any=0;
      for(var __v=0;__v<__cells.length;__v++)if(__ci(__cells[__v])>0){__any=1;break;}
      if(!__any)continue;
      __vals=__place(__cells);
      if(!__vals.join('').trim())continue;
    }else{
      __vals=[];
      for(var __v=0;__v<__cells.length;__v++)__vals.push(__cellText(__cells[__v]));
    }
    __lines.push('| '+__vals.join(' | ')+' |');
    __span+=__allText(__tr).length+32;
    // Anchor on the last row that carries text: virtual/structural grids
    // (vuetify v-virtual-scroll, ej2 spacers) end in a blank sentinel row,
    // and a lastIndexOf("") tail match returns the window limit — the
    // verify then fails and the whole table drops to the tail appendix.
    if(__vals.join('').trim())__lastCells=__cells;
    __data++;
  }
  if(__data>0&&__lastCells){
    __tables.push({md:__lines.join('\n'),
      head:__rowText(__headCells,false).trim(),head2:__rowText(__headCells,true).trim(),
      tail:__rowText(__lastCells,false).trim(),tail2:__rowText(__lastCells,true).trim(),
      span:__span});
    __emitted++;
  }
}
if((__marks.length||__tables.length)&&b.length<500000){
  // The match view collapses whitespace runs to single spaces. EUI-class
  // grids also plant sr-only copy markers (↦ between cells, ↵ after rows)
  // that innerText carries — collapse those as separators too, or a
  // space-joined anchor can never match the marker-joined text.
  var __nb='',__map=[],__run=false;
  for(var __j=0;__j<b.length;__j++){
    var __c2=b.charAt(__j);
    var __w=/[\s↦↵]/.test(__c2);
    if(__w){if(__run)continue;__run=true;}else __run=false;
    __nb+=__w?' ':__c2;__map.push(__j);
  }
  var __used=[],__edit=[],__tail=[],__seen={},__tabFrom=0;
  for(var __ti=0;__ti<__tables.length;__ti++){
    var __t2=__tables[__ti];
    var __h0=__nb.indexOf(__t2.head,__tabFrom);
    if(__h0<0)__h0=__nb.indexOf(__t2.head2,__tabFrom);
    var __tEnd=-1,__tLen=0;
    if(__h0>=0){
      var __lim=Math.min(__nb.length,__h0+__t2.span);
      __tEnd=__nb.lastIndexOf(__t2.tail,__lim);
      if(__tEnd>__h0)__tLen=__t2.tail.length;
      if(__tEnd<0){__tEnd=__nb.lastIndexOf(__t2.tail2,__lim);if(__tEnd>__h0)__tLen=__t2.tail2.length;}
      // Sibling tables reuse the same markup (docs pages repeat one demo):
      // with a generous span budget the backward tail search latches onto
      // the NEXT table's identical last row and the splice then swallows
      // everything between — headings included. A table's own span can
      // never legitimately contain a second run of its header row text
      // (nested tables are never collected), so tighten at every header
      // hit between the anchors and re-search.
      var __next=__nb.indexOf(__t2.head,__h0+1);
      while(__next>__h0&&__next<__tEnd){
        __lim=__next;
        var __e2=__nb.lastIndexOf(__t2.tail,__lim);
        if(__e2>__h0){__tEnd=__e2;__tLen=__t2.tail.length;}
        else{
          __e2=__nb.lastIndexOf(__t2.tail2,__lim);
          if(__e2>__h0){__tEnd=__e2;__tLen=__t2.tail2.length;}
          else{__tEnd=-1;break;}
        }
        __next=__nb.indexOf(__t2.head,__tEnd+1);
      }
    }
    if(__h0>=0&&__tEnd>__h0){
      var __ts0=__map[__h0],__ts1=__map[__tEnd+__tLen-1]+1;
      // Swallow the row-end marker run right after the last cell, and
      // verify the replaced slice really starts/ends with the anchors —
      // an offset mismatch degrades to the tail appendix, never a splice
      // torn out of neighbouring text.
      while(__ts1<b.length&&/[\s↦↵]/.test(b.charAt(__ts1)))__ts1++;
      var __chk=b.slice(__ts0,__ts1).replace(/[\s↦↵]+/g,' ').replace(/^\s+|\s+$/g,'');
      if(__chk.indexOf(__t2.head)===0&&__chk.slice(-__t2.tail.length)===__t2.tail){
        __edit.push([__ts0,__ts1,__t2.md,null]);
        __used.push([__h0,__tEnd+__tLen]);
        __tabFrom=__tEnd+__tLen;
        continue;
      }
    }
    __tail.push(__t2.md);
  }
  // Code blocks (moli-html2md parity): a visible <pre> becomes a fenced block
  // replacing its own text span. Without delimiters the example text is
  // indistinguishable from prose — docs pages whose JSX samples carry paired
  // tags (heroui: "<Table>…</Table>") reparse downstream as real markup, and
  // the consumer's literal-table detector then owns the page start, hiding
  // the true Markdown pipe table from its scoped row parser. Inline <code>
  // stays bare: link labels built on code text (MDN method references) must
  // keep splicing. Links inside a fenced block fall to the tail appendix —
  // the destination survives, the example text keeps its literal face.
  var __pres=document.querySelectorAll('pre');
  for(var __pi=0;__pi<__pres.length&&__pi<100;__pi++){
    var __pre=__pres[__pi];
    if(__hiddenTree(__pre))continue;
    var __pa=__pre.parentElement,__panc=false;
    while(__pa){if(__pa.tagName==='A'){__panc=true;break;}__pa=__pa.parentElement;}
    if(__panc)continue;
    var __pt='';try{__pt=__pre.innerText||'';}catch(e){}
    if(!__pt.replace(/\s+/g,''))continue;
    // Locate the block in the same collapsed view every splice matches in.
    var __pn='',__pr=false;
    for(var __pj=0;__pj<__pt.length;__pj++){
      var __pch=__pt.charAt(__pj);
      var __pw=/[\s↦↵]/.test(__pch);
      if(__pw){if(__pr)continue;__pr=true;}else __pr=false;
      __pn+=__pw?' ':__pch;
    }
    __pn=__pn.replace(/^ +| +$/g,'');
    if(!__pn)continue;
    var __from2=0,__pidx=-1;
    while(true){
      __pidx=__nb.indexOf(__pn,__from2);
      if(__pidx<0)break;
      var __pok=true;
      for(var __pq=0;__pq<__used.length;__pq++){
        if(__pidx<__used[__pq][1]&&__used[__pq][0]<__pidx+__pn.length){__pok=false;break;}
      }
      if(__pok)break;
      __from2=__pidx+1;
    }
    if(__pidx<0)continue;
    var __ps0=__map[__pidx],__ps1=__map[__pidx+__pn.length-1]+1;
    __used.push([__pidx,__pidx+__pn.length]);
    __edit.push([__ps0,__ps1,'```\n'+b.slice(__ps0,__ps1)+'\n```',0]);
  }
  // Splice a multi-word or path-shaped label only where it stands at a word
  // boundary in the rendered text. A label that is the prefix of a longer
  // word ("Prestigio SmartB" inside "Prestigio SmartBook 133S…") tears the
  // word when linked — "[Prestigio SmartB](url)ook" — and consumers that
  // restore a boundary around such labels then read "SmartB ook". innerText
  // has no boundary there, so the occurrence stays plain text. Single-word
  // labels stay exempt: consumers join them back into the word anyway, so
  // linking them is only ever a lost destination, never torn text.
  // Word char ≈ letters/digits (ASCII plus Latin-1 letters, then every
  // script from Greek up — CJK included), mirroring the consumer side.
  function __wb(c){if(!c)return false;var x=c.charCodeAt(0);
    return (x>=48&&x<=57)||(x>=65&&x<=90)||(x>=97&&x<=122)||(x>=0xC0&&x<=0x24F)||x>=0x370;}
  for(var __m=0;__m<__marks.length;__m++){
    var __lab2=__marks[__m][0],__u=__marks[__m][1],__k=__lab2+'\u0000'+__u;
    if(__seen[__k])continue;__seen[__k]=1;
    var __sep=/^[/#]/.test(__lab2)||/\s/.test(__lab2);
    var __from=0,__idx=-1,__dirty=false;
    while(true){
      __idx=__nb.indexOf(__lab2,__from);
      if(__idx<0)break;
      var __ok=true;
      for(var __q2=0;__q2<__used.length;__q2++){
        if(__idx<__used[__q2][1]&&__used[__q2][0]<__idx+__lab2.length){__ok=false;break;}
      }
      if(__ok&&__sep&&(__wb(__idx>0?__nb.charAt(__idx-1):'')||__wb(__nb.charAt(__idx+__lab2.length)))){__ok=false;__dirty=true;}
      if(__ok)break;
      __from=__idx+1;
    }
    if(__idx<0){__tail.push('['+__lab2.replace(/([\\[\]])/g,'\\$1')+']('+__u.replace(/([()])/g,'\\$1')+')');continue;}
    if(!__ok){
      // Every occurrence sat mid-word: the label is already in the text as
      // part of a longer word, and tailing it would duplicate the fragment.
      // Occurrences merely consumed by tables or earlier links keep the
      // historical appendix so the destination is not lost.
      if(!__dirty)__tail.push('['+__lab2.replace(/([\\[\]])/g,'\\$1')+']('+__u.replace(/([()])/g,'\\$1')+')');
      continue;
    }
    __used.push([__idx,__idx+__lab2.length]);
    __edit.push([__map[__idx],__map[__idx+__lab2.length-1]+1,__lab2,__u]);
  }
  // `__marks`/`__tables` follow DOM order, but `b` follows innerText order —
  // they diverge on flex `order`/floats, and back-to-front splicing with a
  // stale offset lands one link inside another's URL (unparseable markdown).
  __edit.sort(function(__p,__q){return __p[0]-__q[0];});
  for(var __e=__edit.length-1;__e>=0;__e--){
    var __ed=__edit[__e];
    // Table replacements carry their own trailing newline — the swallowed
    // marker run was the separator to whatever follows the table. Code-fence
    // replacements (sentinel 0) are already complete: the raw slice kept its
    // own leading/trailing block boundaries.
    var __rep=__ed[3]===null?__ed[2]+'\n':(__ed[3]===0?__ed[2]:'['+__ed[2].replace(/([\\[\]])/g,'\\$1')+']('+__ed[3].replace(/([()])/g,'\\$1')+')');
    b=b.slice(0,__ed[0])+__rep+b.slice(__ed[1]);
  }
  if(__tail.length)b+='\n'+__tail.join('\n');
}
"#;

/// Drop blank lines, turn a no-break space into a normal space, and write
/// table-cell tabs as markdown pipes.
///
/// CSS 2.1's `<title>` is `CSS&nbsp;2.1`. The decoded heading contains
/// U+00A0, so a reader looking for `CSS 2.1` misses the title that is
/// already on the page. Rendered tables separate cells with a tab plus
/// the space the walk leaves between cells (`Company\t Contact\t
/// Country`); the readable row is `Company | Contact | Country`.
pub(super) fn collapse_whitespace(s: &str) -> String {
    s.lines()
        .map(|l| normalize_fetch_line(l.trim()))
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn normalize_fetch_line(line: &str) -> String {
    // Installment typography: React price splits plant the amount in its own
    // element with no whitespace node (`<span>or 9 x</span><b>$1.21</b>`), so
    // innerText faithfully reads "or 9 x$1.21" — while element-joining
    // readers, and the installment wording itself, read "or 9 x $1.21".
    // Restore the boundary on this one unambiguous shape.
    let owned;
    let line = if let Some(fixed) = installment_space(line) {
        owned = fixed;
        owned.as_str()
    } else {
        line
    };
    let mut out = String::with_capacity(line.len());
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\u{00a0}' | '\u{202f}' | '\u{2007}' | '\u{feff}' => out.push(' '),
            '\t' => {
                if matches!(chars.peek(), Some(' ' | '\u{00a0}')) {
                    chars.next();
                }
                out.push_str(" | ");
            }
            other => out.push(other),
        }
    }
    out
}

/// "or 9 x$1.21" → "or 9 x $1.21" — only after a literal `or <digits> x$`.
fn installment_space(line: &str) -> Option<String> {
    let rest = line.strip_prefix("or ")?;
    let digits = rest.bytes().take_while(|b| b.is_ascii_digit()).count();
    if digits == 0 {
        return None;
    }
    let tail = &rest[digits..];
    let after = tail.strip_prefix(" x$")?;
    let next = after.as_bytes().first()?;
    if !next.is_ascii_digit() {
        return None;
    }
    let cut = 3 + digits + 2; // "or " + digits + " x"
    Some(format!("{} {}", &line[..cut], &line[cut..]))
}
