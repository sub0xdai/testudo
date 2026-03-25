import{al as ht,am as mt,j as st,l as at,m as w,r as Tt,C as ft,an as F,a as gt,R as Ot,O as At,E as kt,t as Ct,W as Ft}from"./index-DZIsZQtF.js";import{U as vt,n as D,r as Q,c as ot}from"./index-my5TNqCY.js";import"./index-C9ALJoyu.js";import"./index-wtjm0wDZ.js";import{o as Nt}from"./if-defined-ecIJCIgS.js";import"./index-CVEtAJqt.js";import"./index-DStR61Q8.js";var Dt={exports:{}};(function(t,e){(function(i,r){t.exports=r()})(mt,function(){var i=1e3,r=6e4,s=36e5,n="millisecond",o="second",c="minute",g="hour",m="day",b="week",$="month",k="quarter",C="year",N="date",Y="Invalid Date",X=/^(\d{4})[-/]?(\d{1,2})?[-/]?(\d{0,2})[Tt\s]*(\d{1,2})?:?(\d{1,2})?:?(\d{1,2})?[.:]?(\d+)?$/,tt=/\[([^\]]+)]|Y{1,4}|M{1,4}|D{1,2}|d{1,4}|H{1,2}|h{1,2}|a|A|m{1,2}|s{1,2}|Z{1,2}|SSS/g,et={name:"en",weekdays:"Sunday_Monday_Tuesday_Wednesday_Thursday_Friday_Saturday".split("_"),months:"January_February_March_April_May_June_July_August_September_October_November_December".split("_"),ordinal:function(f){var l=["th","st","nd","rd"],a=f%100;return"["+f+(l[(a-20)%10]||l[a]||l[0])+"]"}},it=function(f,l,a){var d=String(f);return!d||d.length>=l?f:""+Array(l+1-d.length).join(a)+f},B={s:it,z:function(f){var l=-f.utcOffset(),a=Math.abs(l),d=Math.floor(a/60),u=a%60;return(l<=0?"+":"-")+it(d,2,"0")+":"+it(u,2,"0")},m:function f(l,a){if(l.date()<a.date())return-f(a,l);var d=12*(a.year()-l.year())+(a.month()-l.month()),u=l.clone().add(d,$),p=a-u<0,h=l.clone().add(d+(p?-1:1),$);return+(-(d+(a-u)/(p?u-h:h-u))||0)},a:function(f){return f<0?Math.ceil(f)||0:Math.floor(f)},p:function(f){return{M:$,y:C,w:b,d:m,D:N,h:g,m:c,s:o,ms:n,Q:k}[f]||String(f||"").toLowerCase().replace(/s$/,"")},u:function(f){return f===void 0}},M="en",S={};S[M]=et;var V="$isDayjsObject",W=function(f){return f instanceof lt||!(!f||!f[V])},ct=function f(l,a,d){var u;if(!l)return M;if(typeof l=="string"){var p=l.toLowerCase();S[p]&&(u=p),a&&(S[p]=a,u=p);var h=l.split("-");if(!u&&h.length>1)return f(h[0])}else{var x=l.name;S[x]=l,u=x}return!d&&u&&(M=u),u||!d&&M},I=function(f,l){if(W(f))return f.clone();var a=typeof l=="object"?l:{};return a.date=f,a.args=arguments,new lt(a)},y=B;y.l=ct,y.i=W,y.w=function(f,l){return I(f,{locale:l.$L,utc:l.$u,x:l.$x,$offset:l.$offset})};var lt=function(){function f(a){this.$L=ct(a.locale,null,!0),this.parse(a),this.$x=this.$x||a.x||{},this[V]=!0}var l=f.prototype;return l.parse=function(a){this.$d=function(d){var u=d.date,p=d.utc;if(u===null)return new Date(NaN);if(y.u(u))return new Date;if(u instanceof Date)return new Date(u);if(typeof u=="string"&&!/Z$/i.test(u)){var h=u.match(X);if(h){var x=h[2]-1||0,v=(h[7]||"0").substring(0,3);return p?new Date(Date.UTC(h[1],x,h[3]||1,h[4]||0,h[5]||0,h[6]||0,v)):new Date(h[1],x,h[3]||1,h[4]||0,h[5]||0,h[6]||0,v)}}return new Date(u)}(a),this.init()},l.init=function(){var a=this.$d;this.$y=a.getFullYear(),this.$M=a.getMonth(),this.$D=a.getDate(),this.$W=a.getDay(),this.$H=a.getHours(),this.$m=a.getMinutes(),this.$s=a.getSeconds(),this.$ms=a.getMilliseconds()},l.$utils=function(){return y},l.isValid=function(){return this.$d.toString()!==Y},l.isSame=function(a,d){var u=I(a);return this.startOf(d)<=u&&u<=this.endOf(d)},l.isAfter=function(a,d){return I(a)<this.startOf(d)},l.isBefore=function(a,d){return this.endOf(d)<I(a)},l.$g=function(a,d,u){return y.u(a)?this[d]:this.set(u,a)},l.unix=function(){return Math.floor(this.valueOf()/1e3)},l.valueOf=function(){return this.$d.getTime()},l.startOf=function(a,d){var u=this,p=!!y.u(d)||d,h=y.p(a),x=function(H,O){var z=y.w(u.$u?Date.UTC(u.$y,O,H):new Date(u.$y,O,H),u);return p?z:z.endOf(m)},v=function(H,O){return y.w(u.toDate()[H].apply(u.toDate("s"),(p?[0,0,0,0]:[23,59,59,999]).slice(O)),u)},T=this.$W,_=this.$M,A=this.$D,J="set"+(this.$u?"UTC":"");switch(h){case C:return p?x(1,0):x(31,11);case $:return p?x(1,_):x(0,_+1);case b:var P=this.$locale().weekStart||0,rt=(T<P?T+7:T)-P;return x(p?A-rt:A+(6-rt),_);case m:case N:return v(J+"Hours",0);case g:return v(J+"Minutes",1);case c:return v(J+"Seconds",2);case o:return v(J+"Milliseconds",3);default:return this.clone()}},l.endOf=function(a){return this.startOf(a,!1)},l.$set=function(a,d){var u,p=y.p(a),h="set"+(this.$u?"UTC":""),x=(u={},u[m]=h+"Date",u[N]=h+"Date",u[$]=h+"Month",u[C]=h+"FullYear",u[g]=h+"Hours",u[c]=h+"Minutes",u[o]=h+"Seconds",u[n]=h+"Milliseconds",u)[p],v=p===m?this.$D+(d-this.$W):d;if(p===$||p===C){var T=this.clone().set(N,1);T.$d[x](v),T.init(),this.$d=T.set(N,Math.min(this.$D,T.daysInMonth())).$d}else x&&this.$d[x](v);return this.init(),this},l.set=function(a,d){return this.clone().$set(a,d)},l.get=function(a){return this[y.p(a)]()},l.add=function(a,d){var u,p=this;a=Number(a);var h=y.p(d),x=function(_){var A=I(p);return y.w(A.date(A.date()+Math.round(_*a)),p)};if(h===$)return this.set($,this.$M+a);if(h===C)return this.set(C,this.$y+a);if(h===m)return x(1);if(h===b)return x(7);var v=(u={},u[c]=r,u[g]=s,u[o]=i,u)[h]||1,T=this.$d.getTime()+a*v;return y.w(T,this)},l.subtract=function(a,d){return this.add(-1*a,d)},l.format=function(a){var d=this,u=this.$locale();if(!this.isValid())return u.invalidDate||Y;var p=a||"YYYY-MM-DDTHH:mm:ssZ",h=y.z(this),x=this.$H,v=this.$m,T=this.$M,_=u.weekdays,A=u.months,J=u.meridiem,P=function(O,z,nt,dt){return O&&(O[z]||O(d,p))||nt[z].slice(0,dt)},rt=function(O){return y.s(x%12||12,O,"0")},H=J||function(O,z,nt){var dt=O<12?"AM":"PM";return nt?dt.toLowerCase():dt};return p.replace(tt,function(O,z){return z||function(nt){switch(nt){case"YY":return String(d.$y).slice(-2);case"YYYY":return y.s(d.$y,4,"0");case"M":return T+1;case"MM":return y.s(T+1,2,"0");case"MMM":return P(u.monthsShort,T,A,3);case"MMMM":return P(A,T);case"D":return d.$D;case"DD":return y.s(d.$D,2,"0");case"d":return String(d.$W);case"dd":return P(u.weekdaysMin,d.$W,_,2);case"ddd":return P(u.weekdaysShort,d.$W,_,3);case"dddd":return _[d.$W];case"H":return String(x);case"HH":return y.s(x,2,"0");case"h":return rt(1);case"hh":return rt(2);case"a":return H(x,v,!0);case"A":return H(x,v,!1);case"m":return String(v);case"mm":return y.s(v,2,"0");case"s":return String(d.$s);case"ss":return y.s(d.$s,2,"0");case"SSS":return y.s(d.$ms,3,"0");case"Z":return h}return null}(O)||h.replace(":","")})},l.utcOffset=function(){return 15*-Math.round(this.$d.getTimezoneOffset()/15)},l.diff=function(a,d,u){var p,h=this,x=y.p(d),v=I(a),T=(v.utcOffset()-this.utcOffset())*r,_=this-v,A=function(){return y.m(h,v)};switch(x){case C:p=A()/12;break;case $:p=A();break;case k:p=A()/3;break;case b:p=(_-T)/6048e5;break;case m:p=(_-T)/864e5;break;case g:p=_/s;break;case c:p=_/r;break;case o:p=_/i;break;default:p=_}return u?p:y.a(p)},l.daysInMonth=function(){return this.endOf($).$D},l.$locale=function(){return S[this.$L]},l.locale=function(a,d){if(!a)return this.$L;var u=this.clone(),p=ct(a,d,!0);return p&&(u.$L=p),u},l.clone=function(){return y.w(this.$d,this)},l.toDate=function(){return new Date(this.valueOf())},l.toJSON=function(){return this.isValid()?this.toISOString():null},l.toISOString=function(){return this.$d.toISOString()},l.toString=function(){return this.$d.toUTCString()},f}(),$t=lt.prototype;return I.prototype=$t,[["$ms",n],["$s",o],["$m",c],["$H",g],["$W",m],["$M",$],["$y",C],["$D",N]].forEach(function(f){$t[f[1]]=function(l){return this.$g(l,f[0],f[1])}}),I.extend=function(f,l){return f.$i||(f(l,lt,I),f.$i=!0),I},I.locale=ct,I.isDayjs=W,I.unix=function(f){return I(1e3*f)},I.en=S[M],I.Ls=S,I.p={},I})})(Dt);var Rt=Dt.exports;const K=ht(Rt);var _t={exports:{}};(function(t,e){(function(i,r){t.exports=r()})(mt,function(){return{name:"en",weekdays:"Sunday_Monday_Tuesday_Wednesday_Thursday_Friday_Saturday".split("_"),months:"January_February_March_April_May_June_July_August_September_October_November_December".split("_"),ordinal:function(i){var r=["th","st","nd","rd"],s=i%100;return"["+i+(r[(s-20)%10]||r[s]||r[0])+"]"}}})})(_t);var jt=_t.exports;const Lt=ht(jt);var Mt={exports:{}};(function(t,e){(function(i,r){t.exports=r()})(mt,function(){return function(i,r,s){i=i||{};var n=r.prototype,o={future:"in %s",past:"%s ago",s:"a few seconds",m:"a minute",mm:"%d minutes",h:"an hour",hh:"%d hours",d:"a day",dd:"%d days",M:"a month",MM:"%d months",y:"a year",yy:"%d years"};function c(m,b,$,k){return n.fromToBase(m,b,$,k)}s.en.relativeTime=o,n.fromToBase=function(m,b,$,k,C){for(var N,Y,X,tt=$.$locale().relativeTime||o,et=i.thresholds||[{l:"s",r:44,d:"second"},{l:"m",r:89},{l:"mm",r:44,d:"minute"},{l:"h",r:89},{l:"hh",r:21,d:"hour"},{l:"d",r:35},{l:"dd",r:25,d:"day"},{l:"M",r:45},{l:"MM",r:10,d:"month"},{l:"y",r:17},{l:"yy",d:"year"}],it=et.length,B=0;B<it;B+=1){var M=et[B];M.d&&(N=k?s(m).diff($,M.d,!0):$.diff(m,M.d,!0));var S=(i.rounding||Math.round)(Math.abs(N));if(X=N>0,S<=M.r||!M.r){S<=1&&B>0&&(M=et[B-1]);var V=tt[M.l];C&&(S=C(""+S)),Y=typeof V=="string"?V.replace("%d",S):V(S,b,M.l,X);break}}if(b)return Y;var W=X?tt.future:tt.past;return typeof W=="function"?W(Y):W.replace("%s",Y)},n.to=function(m,b){return c(m,b,this,!0)},n.from=function(m,b){return c(m,b,this)};var g=function(m){return m.$u?s.utc():s()};n.toNow=function(m){return this.to(g(this),m)},n.fromNow=function(m){return this.from(g(this),m)}}})})(Mt);var zt=Mt.exports;const Ut=ht(zt);var St={exports:{}};(function(t,e){(function(i,r){t.exports=r()})(mt,function(){return function(i,r,s){s.updateLocale=function(n,o){var c=s.Ls[n];if(c)return(o?Object.keys(o):[]).forEach(function(g){c[g]=o[g]}),c}}})})(St);var Et=St.exports;const Yt=ht(Et);K.extend(Ut);K.extend(Yt);const Bt={...Lt,name:"en-web3-modal",relativeTime:{future:"in %s",past:"%s ago",s:"%d sec",m:"1 min",mm:"%d min",h:"1 hr",hh:"%d hrs",d:"1 d",dd:"%d d",M:"1 mo",MM:"%d mo",y:"1 yr",yy:"%d yr"}},Wt=["January","February","March","April","May","June","July","August","September","October","November","December"];K.locale("en-web3-modal",Bt);const yt={getMonthNameByIndex(t){return Wt[t]},getYear(t=new Date().toISOString()){return K(t).year()},getRelativeDateFromNow(t){return K(t).locale("en-web3-modal").fromNow(!0)},formatDate(t,e="DD MMM"){return K(t).format(e)}},Pt=3,pt=.1,Ht=["receive","deposit","borrow","claim"],Gt=["withdraw","repay","burn"],Z={getTransactionGroupTitle(t,e){const i=yt.getYear(),r=yt.getMonthNameByIndex(e);return t===i?r:`${r} ${t}`},getTransactionImages(t){const[e]=t;return t?.length>1?t.map(r=>this.getTransactionImage(r)):[this.getTransactionImage(e)]},getTransactionImage(t){return{type:Z.getTransactionTransferTokenType(t),url:Z.getTransactionImageURL(t)}},getTransactionImageURL(t){let e;const i=!!t?.nft_info,r=!!t?.fungible_info;return t&&i?e=t?.nft_info?.content?.preview?.url:t&&r&&(e=t?.fungible_info?.icon?.url),e},getTransactionTransferTokenType(t){if(t?.fungible_info)return"FUNGIBLE";if(t?.nft_info)return"NFT"},getTransactionDescriptions(t,e){const i=t?.metadata?.operationType,r=e||t?.transfers,s=r&&r.length>0,n=r&&r.length>1,o=s&&r.every(k=>!!k?.fungible_info),[c,g]=r||[];let m=this.getTransferDescription(c),b=this.getTransferDescription(g);if(!s)return(i==="send"||i==="receive")&&o?(m=vt.getTruncateString({string:t?.metadata.sentFrom,charsStart:4,charsEnd:6,truncate:"middle"}),b=vt.getTruncateString({string:t?.metadata.sentTo,charsStart:4,charsEnd:6,truncate:"middle"}),[m,b]):[t.metadata.status];if(n)return r?.map(k=>this.getTransferDescription(k));let $="";return Ht.includes(i)?$="+":Gt.includes(i)&&($="-"),m=$.concat(m),[m]},getTransferDescription(t){let e="";return t&&(t?.nft_info?e=t?.nft_info?.name||"-":t?.fungible_info&&(e=this.getFungibleTransferDescription(t)||"-")),e},getFungibleTransferDescription(t){return t?[this.getQuantityFixedValue(t?.quantity.numeric),t?.fungible_info?.symbol].join(" ").trim():null},mergeTransfers(t){if(t?.length<=1)return t;const i=this.filterGasFeeTransfers(t).reduce((s,n)=>{const o=n?.fungible_info?.name,c=s.find(({fungible_info:g,direction:m})=>o&&o===g?.name&&m===n.direction);if(c){const g=Number(c.quantity.numeric)+Number(n.quantity.numeric);c.quantity.numeric=g.toString(),c.value=(c.value||0)+(n.value||0)}else s.push(n);return s},[]);let r=i;return i.length>2&&(r=i.sort((s,n)=>(n.value||0)-(s.value||0)).slice(0,2)),r=r.sort((s,n)=>s.direction==="out"&&n.direction==="in"?-1:s.direction==="in"&&n.direction==="out"?1:0),r},filterGasFeeTransfers(t){const e=t?.reduce((r,s)=>{const n=s?.fungible_info?.name;return n&&(r[n]||(r[n]=[]),r[n].push(s)),r},{}),i=[];return Object.values(e??{}).forEach(r=>{if(r.length===1){const s=r[0];s&&i.push(s)}else{const s=r.filter(o=>o.direction==="in"),n=r.filter(o=>o.direction==="out");if(s.length===1&&n.length===1){const o=s[0],c=n[0];let g=!1;if(o&&c){const m=Number(o.quantity.numeric),b=Number(c.quantity.numeric);b<m*pt?(i.push(o),g=!0):m<b*pt&&(i.push(c),g=!0)}g||i.push(...r)}else{const o=this.filterGasFeesFromTokenGroup(r);i.push(...o)}}}),t?.forEach(r=>{r?.fungible_info?.name||i.push(r)}),i},filterGasFeesFromTokenGroup(t){if(t.length<=1)return t;const e=t?.map(c=>Number(c.quantity.numeric)),i=Math.max(...e),r=Math.min(...e),s=.01;if(r<i*s)return t?.filter(g=>Number(g.quantity.numeric)>=i*s);const n=t?.filter(c=>c.direction==="in"),o=t?.filter(c=>c.direction==="out");if(n.length===1&&o.length===1){const c=n[0],g=o[0];if(c&&g){const m=Number(c.quantity.numeric),b=Number(g.quantity.numeric);if(b<m*pt)return[c];if(m<b*pt)return[g]}}return t},getQuantityFixedValue(t){return t?parseFloat(t).toFixed(Pt):null}};var xt;(function(t){t.approve="approved",t.bought="bought",t.borrow="borrowed",t.burn="burnt",t.cancel="canceled",t.claim="claimed",t.deploy="deployed",t.deposit="deposited",t.execute="executed",t.mint="minted",t.receive="received",t.repay="repaid",t.send="sent",t.sell="sold",t.stake="staked",t.trade="swapped",t.unstake="unstaked",t.withdraw="withdrawn"})(xt||(xt={}));const qt=st`
  :host > wui-flex {
    display: flex;
    justify-content: center;
    align-items: center;
    position: relative;
    width: 40px;
    height: 40px;
    box-shadow: inset 0 0 0 1px ${({tokens:t})=>t.core.glass010};
    background-color: ${({tokens:t})=>t.theme.foregroundPrimary};
  }

  :host([data-no-images='true']) > wui-flex {
    background-color: ${({tokens:t})=>t.theme.foregroundPrimary};
    border-radius: ${({borderRadius:t})=>t[3]} !important;
  }

  :host > wui-flex wui-image {
    display: block;
  }

  :host > wui-flex,
  :host > wui-flex wui-image,
  .swap-images-container,
  .swap-images-container.nft,
  wui-image.nft {
    border-top-left-radius: var(--local-left-border-radius);
    border-top-right-radius: var(--local-right-border-radius);
    border-bottom-left-radius: var(--local-left-border-radius);
    border-bottom-right-radius: var(--local-right-border-radius);
  }

  .swap-images-container {
    position: relative;
    width: 40px;
    height: 40px;
    overflow: hidden;
  }

  .swap-images-container wui-image:first-child {
    position: absolute;
    width: 40px;
    height: 40px;
    top: 0;
    left: 0%;
    clip-path: inset(0px calc(50% + 2px) 0px 0%);
  }

  .swap-images-container wui-image:last-child {
    clip-path: inset(0px 0px 0px calc(50% + 2px));
  }

  .swap-fallback-container {
    position: absolute;
    inset: 0;
    width: 40px;
    height: 40px;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .swap-fallback-container.first {
    clip-path: inset(0px calc(50% + 2px) 0px 0%);
  }

  .swap-fallback-container.last {
    clip-path: inset(0px 0px 0px calc(50% + 2px));
  }

  wui-flex.status-box {
    position: absolute;
    right: 0;
    bottom: 0;
    transform: translate(20%, 20%);
    border-radius: ${({borderRadius:t})=>t[4]};
    background-color: ${({tokens:t})=>t.theme.backgroundPrimary};
    box-shadow: 0 0 0 2px ${({tokens:t})=>t.theme.backgroundPrimary};
    overflow: hidden;
    width: 16px;
    height: 16px;
  }
`;var U=function(t,e,i,r){var s=arguments.length,n=s<3?e:r===null?r=Object.getOwnPropertyDescriptor(e,i):r,o;if(typeof Reflect=="object"&&typeof Reflect.decorate=="function")n=Reflect.decorate(t,e,i,r);else for(var c=t.length-1;c>=0;c--)(o=t[c])&&(n=(s<3?o(n):s>3?o(e,i,n):o(e,i))||n);return s>3&&n&&Object.defineProperty(e,i,n),n};let R=class extends at{constructor(){super(...arguments),this.images=[],this.secondImage={type:void 0,url:""},this.failedImageUrls=new Set}handleImageError(e){return i=>{i.stopPropagation(),this.failedImageUrls.add(e),this.requestUpdate()}}render(){const[e,i]=this.images;this.images.length||(this.dataset.noImages="true");const r=e?.type==="NFT",s=i?.url?i.type==="NFT":r,n=r?"var(--apkt-borderRadius-3)":"var(--apkt-borderRadius-5)",o=s?"var(--apkt-borderRadius-3)":"var(--apkt-borderRadius-5)";return this.style.cssText=`
    --local-left-border-radius: ${n};
    --local-right-border-radius: ${o};
    `,w`<wui-flex> ${this.templateVisual()} ${this.templateIcon()} </wui-flex>`}templateVisual(){const[e,i]=this.images;return this.images.length===2&&(e?.url||i?.url)?this.renderSwapImages(e,i):e?.url&&!this.failedImageUrls.has(e.url)?this.renderSingleImage(e):e?.type==="NFT"?this.renderPlaceholderIcon("nftPlaceholder"):this.renderPlaceholderIcon("coinPlaceholder")}renderSwapImages(e,i){return w`<div class="swap-images-container">
      ${e?.url?this.renderImageOrFallback(e,"first",!0):null}
      ${i?.url?this.renderImageOrFallback(i,"last",!0):null}
    </div>`}renderSingleImage(e){return this.renderImageOrFallback(e,void 0,!1)}renderImageOrFallback(e,i,r=!1){return e.url?this.failedImageUrls.has(e.url)?r&&i?this.renderFallbackIconInContainer(i):this.renderFallbackIcon():w`<wui-image
      src=${e.url}
      alt="Transaction image"
      @onLoadError=${this.handleImageError(e.url)}
    ></wui-image>`:null}renderFallbackIconInContainer(e){return w`<div class="swap-fallback-container ${e}">${this.renderFallbackIcon()}</div>`}renderFallbackIcon(){return w`<wui-icon
      size="xl"
      weight="regular"
      color="default"
      name="networkPlaceholder"
    ></wui-icon>`}renderPlaceholderIcon(e){return w`<wui-icon size="xl" weight="regular" color="default" name=${e}></wui-icon>`}templateIcon(){let e="accent-primary",i;return i=this.getIcon(),this.status&&(e=this.getStatusColor()),i?w`
      <wui-flex alignItems="center" justifyContent="center" class="status-box">
        <wui-icon-box size="sm" color=${e} icon=${i}></wui-icon-box>
      </wui-flex>
    `:null}getDirectionIcon(){switch(this.direction){case"in":return"arrowBottom";case"out":return"arrowTop";default:return}}getIcon(){return this.onlyDirectionIcon?this.getDirectionIcon():this.type==="trade"?"swapHorizontal":this.type==="approve"?"checkmark":this.type==="cancel"?"close":this.getDirectionIcon()}getStatusColor(){switch(this.status){case"confirmed":return"success";case"failed":return"error";case"pending":return"inverse";default:return"accent-primary"}}};R.styles=[qt];U([D()],R.prototype,"type",void 0);U([D()],R.prototype,"status",void 0);U([D()],R.prototype,"direction",void 0);U([D({type:Boolean})],R.prototype,"onlyDirectionIcon",void 0);U([D({type:Array})],R.prototype,"images",void 0);U([D({type:Object})],R.prototype,"secondImage",void 0);U([Q()],R.prototype,"failedImageUrls",void 0);R=U([ot("wui-transaction-visual")],R);const Vt=st`
  :host {
    width: 100%;
  }

  :host > wui-flex:first-child {
    align-items: center;
    column-gap: ${({spacing:t})=>t[2]};
    padding: ${({spacing:t})=>t[1]} ${({spacing:t})=>t[2]};
    width: 100%;
  }

  :host > wui-flex:first-child wui-text:nth-child(1) {
    text-transform: capitalize;
  }

  wui-transaction-visual {
    width: 40px;
    height: 40px;
  }

  wui-flex {
    flex: 1;
  }

  :host wui-flex wui-flex {
    overflow: hidden;
  }

  :host .description-container wui-text span {
    word-break: break-all;
  }

  :host .description-container wui-text {
    overflow: hidden;
  }

  :host .description-separator-icon {
    margin: 0px 6px;
  }

  :host wui-text > span {
    overflow: hidden;
    display: -webkit-box;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 1;
  }
`;var E=function(t,e,i,r){var s=arguments.length,n=s<3?e:r===null?r=Object.getOwnPropertyDescriptor(e,i):r,o;if(typeof Reflect=="object"&&typeof Reflect.decorate=="function")n=Reflect.decorate(t,e,i,r);else for(var c=t.length-1;c>=0;c--)(o=t[c])&&(n=(s<3?o(n):s>3?o(e,i,n):o(e,i))||n);return s>3&&n&&Object.defineProperty(e,i,n),n};let j=class extends at{constructor(){super(...arguments),this.type="approve",this.onlyDirectionIcon=!1,this.images=[]}render(){return w`
      <wui-flex>
        <wui-transaction-visual
          .status=${this.status}
          direction=${Nt(this.direction)}
          type=${this.type}
          .onlyDirectionIcon=${this.onlyDirectionIcon}
          .images=${this.images}
        ></wui-transaction-visual>
        <wui-flex flexDirection="column" gap="1">
          <wui-text variant="lg-medium" color="primary">
            ${xt[this.type]||this.type}
          </wui-text>
          <wui-flex class="description-container">
            ${this.templateDescription()} ${this.templateSecondDescription()}
          </wui-flex>
        </wui-flex>
        <wui-text variant="sm-medium" color="secondary"><span>${this.date}</span></wui-text>
      </wui-flex>
    `}templateDescription(){const e=this.descriptions?.[0];return e?w`
          <wui-text variant="md-regular" color="secondary">
            <span>${e}</span>
          </wui-text>
        `:null}templateSecondDescription(){const e=this.descriptions?.[1];return e?w`
          <wui-icon class="description-separator-icon" size="sm" name="arrowRight"></wui-icon>
          <wui-text variant="md-regular" color="secondary">
            <span>${e}</span>
          </wui-text>
        `:null}};j.styles=[Tt,Vt];E([D()],j.prototype,"type",void 0);E([D({type:Array})],j.prototype,"descriptions",void 0);E([D()],j.prototype,"date",void 0);E([D({type:Boolean})],j.prototype,"onlyDirectionIcon",void 0);E([D()],j.prototype,"status",void 0);E([D()],j.prototype,"direction",void 0);E([D({type:Array})],j.prototype,"images",void 0);j=E([ot("wui-transaction-list-item")],j);const Jt=st`
  wui-flex {
    position: relative;
    display: inline-flex;
    justify-content: center;
    align-items: center;
  }

  wui-image {
    border-radius: ${({borderRadius:t})=>t[128]};
  }

  .fallback-icon {
    color: ${({tokens:t})=>t.theme.iconInverse};
    border-radius: ${({borderRadius:t})=>t[3]};
    background-color: ${({tokens:t})=>t.theme.foregroundPrimary};
  }

  .direction-icon,
  .status-image {
    position: absolute;
    right: 0;
    bottom: 0;
    border-radius: ${({borderRadius:t})=>t[128]};
    border: 2px solid ${({tokens:t})=>t.theme.backgroundPrimary};
  }

  .direction-icon {
    padding: ${({spacing:t})=>t["01"]};
    color: ${({tokens:t})=>t.core.iconSuccess};

    background-color: color-mix(
      in srgb,
      ${({tokens:t})=>t.core.textSuccess} 30%,
      ${({tokens:t})=>t.theme.backgroundPrimary} 70%
    );
  }

  /* -- Sizes --------------------------------------------------- */
  :host([data-size='sm']) > wui-image:not(.status-image),
  :host([data-size='sm']) > wui-flex {
    width: 24px;
    height: 24px;
  }

  :host([data-size='lg']) > wui-image:not(.status-image),
  :host([data-size='lg']) > wui-flex {
    width: 40px;
    height: 40px;
  }

  :host([data-size='sm']) .fallback-icon {
    height: 16px;
    width: 16px;
    padding: ${({spacing:t})=>t[1]};
  }

  :host([data-size='lg']) .fallback-icon {
    height: 32px;
    width: 32px;
    padding: ${({spacing:t})=>t[1]};
  }

  :host([data-size='sm']) .direction-icon,
  :host([data-size='sm']) .status-image {
    transform: translate(40%, 30%);
  }

  :host([data-size='lg']) .direction-icon,
  :host([data-size='lg']) .status-image {
    transform: translate(40%, 10%);
  }

  :host([data-size='sm']) .status-image {
    height: 14px;
    width: 14px;
  }

  :host([data-size='lg']) .status-image {
    height: 20px;
    width: 20px;
  }

  /* -- Crop effects --------------------------------------------------- */
  .swap-crop-left-image,
  .swap-crop-right-image {
    position: absolute;
    top: 0;
    bottom: 0;
  }

  .swap-crop-left-image {
    left: 0;
    clip-path: inset(0px calc(50% + 1.5px) 0px 0%);
  }

  .swap-crop-right-image {
    right: 0;
    clip-path: inset(0px 0px 0px calc(50% + 1.5px));
  }
`;var ut=function(t,e,i,r){var s=arguments.length,n=s<3?e:r===null?r=Object.getOwnPropertyDescriptor(e,i):r,o;if(typeof Reflect=="object"&&typeof Reflect.decorate=="function")n=Reflect.decorate(t,e,i,r);else for(var c=t.length-1;c>=0;c--)(o=t[c])&&(n=(s<3?o(n):s>3?o(e,i,n):o(e,i))||n);return s>3&&n&&Object.defineProperty(e,i,n),n};const wt={sm:"xxs",lg:"md"};let G=class extends at{constructor(){super(...arguments),this.type="approve",this.size="lg",this.statusImageUrl="",this.images=[]}render(){return w`<wui-flex>${this.templateVisual()} ${this.templateIcon()}</wui-flex>`}templateVisual(){switch(this.dataset.size=this.size,this.type){case"trade":return this.swapTemplate();case"fiat":return this.fiatTemplate();case"unknown":return this.unknownTemplate();default:return this.tokenTemplate()}}swapTemplate(){const[e,i]=this.images;return this.images.length===2&&(e||i)?w`
        <wui-image class="swap-crop-left-image" src=${e} alt="Swap image"></wui-image>
        <wui-image class="swap-crop-right-image" src=${i} alt="Swap image"></wui-image>
      `:e?w`<wui-image src=${e} alt="Swap image"></wui-image>`:null}fiatTemplate(){return w`<wui-icon
      class="fallback-icon"
      size=${wt[this.size]}
      name="dollar"
    ></wui-icon>`}unknownTemplate(){return w`<wui-icon
      class="fallback-icon"
      size=${wt[this.size]}
      name="questionMark"
    ></wui-icon>`}tokenTemplate(){const[e]=this.images;return e?w`<wui-image src=${e} alt="Token image"></wui-image> `:w`<wui-icon
      class="fallback-icon"
      name=${this.type==="nft"?"image":"coinPlaceholder"}
    ></wui-icon>`}templateIcon(){return this.statusImageUrl?w`<wui-image
        class="status-image"
        src=${this.statusImageUrl}
        alt="Status image"
      ></wui-image>`:w`<wui-icon
      class="direction-icon"
      size=${wt[this.size]}
      name=${this.getTemplateIcon()}
    ></wui-icon>`}getTemplateIcon(){return this.type==="trade"?"arrowClockWise":"arrowBottom"}};G.styles=[Jt];ut([D()],G.prototype,"type",void 0);ut([D()],G.prototype,"size",void 0);ut([D()],G.prototype,"statusImageUrl",void 0);ut([D({type:Array})],G.prototype,"images",void 0);G=ut([ot("wui-transaction-thumbnail")],G);const Zt=st`
  :host > wui-flex:first-child {
    gap: ${({spacing:t})=>t[2]};
    padding: ${({spacing:t})=>t[3]};
    width: 100%;
  }

  wui-flex {
    display: flex;
    flex: 1;
  }
`;var Kt=function(t,e,i,r){var s=arguments.length,n=s<3?e:r===null?r=Object.getOwnPropertyDescriptor(e,i):r,o;if(typeof Reflect=="object"&&typeof Reflect.decorate=="function")n=Reflect.decorate(t,e,i,r);else for(var c=t.length-1;c>=0;c--)(o=t[c])&&(n=(s<3?o(n):s>3?o(e,i,n):o(e,i))||n);return s>3&&n&&Object.defineProperty(e,i,n),n};let bt=class extends at{render(){return w`
      <wui-flex alignItems="center" .padding=${["1","2","1","2"]}>
        <wui-shimmer width="40px" height="40px" rounded></wui-shimmer>
        <wui-flex flexDirection="column" gap="1">
          <wui-shimmer width="124px" height="16px" rounded></wui-shimmer>
          <wui-shimmer width="60px" height="14px" rounded></wui-shimmer>
        </wui-flex>
        <wui-shimmer width="24px" height="12px" rounded></wui-shimmer>
      </wui-flex>
    `}};bt.styles=[Tt,Zt];bt=Kt([ot("wui-transaction-list-item-loader")],bt);const Qt=st`
  :host {
    min-height: 100%;
  }

  .group-container[last-group='true'] {
    padding-bottom: ${({spacing:t})=>t[3]};
  }

  .contentContainer {
    height: 280px;
  }

  .contentContainer > wui-icon-box {
    width: 40px;
    height: 40px;
    border-radius: ${({borderRadius:t})=>t[3]};
  }

  .contentContainer > .textContent {
    width: 65%;
  }

  .emptyContainer {
    height: 100%;
  }
`;var q=function(t,e,i,r){var s=arguments.length,n=s<3?e:r===null?r=Object.getOwnPropertyDescriptor(e,i):r,o;if(typeof Reflect=="object"&&typeof Reflect.decorate=="function")n=Reflect.decorate(t,e,i,r);else for(var c=t.length-1;c>=0;c--)(o=t[c])&&(n=(s<3?o(n):s>3?o(e,i,n):o(e,i))||n);return s>3&&n&&Object.defineProperty(e,i,n),n};const It="last-transaction",Xt=7;let L=class extends at{constructor(){super(),this.unsubscribe=[],this.paginationObserver=void 0,this.page="activity",this.caipAddress=ft.state.activeCaipAddress,this.transactionsByYear=F.state.transactionsByYear,this.loading=F.state.loading,this.empty=F.state.empty,this.next=F.state.next,F.clearCursor(),this.unsubscribe.push(ft.subscribeKey("activeCaipAddress",e=>{e&&this.caipAddress!==e&&(F.resetTransactions(),F.fetchTransactions(e)),this.caipAddress=e}),ft.subscribeKey("activeCaipNetwork",()=>{this.updateTransactionView()}),F.subscribe(e=>{this.transactionsByYear=e.transactionsByYear,this.loading=e.loading,this.empty=e.empty,this.next=e.next}))}firstUpdated(){this.updateTransactionView(),this.createPaginationObserver()}updated(){this.setPaginationObserver()}disconnectedCallback(){this.unsubscribe.forEach(e=>e())}render(){return w` ${this.empty?null:this.templateTransactionsByYear()}
    ${this.loading?this.templateLoading():null}
    ${!this.loading&&this.empty?this.templateEmpty():null}`}updateTransactionView(){F.resetTransactions(),this.caipAddress&&F.fetchTransactions(gt.getPlainAddress(this.caipAddress))}templateTransactionsByYear(){return Object.keys(this.transactionsByYear).sort().reverse().map(i=>{const r=parseInt(i,10),s=new Array(12).fill(null).map((n,o)=>{const c=Z.getTransactionGroupTitle(r,o),g=this.transactionsByYear[r]?.[o];return{groupTitle:c,transactions:g}}).filter(({transactions:n})=>n).reverse();return s.map(({groupTitle:n,transactions:o},c)=>{const g=c===s.length-1;return o?w`
          <wui-flex
            flexDirection="column"
            class="group-container"
            last-group="${g?"true":"false"}"
            data-testid="month-indexes"
          >
            <wui-flex
              alignItems="center"
              flexDirection="row"
              .padding=${["2","3","3","3"]}
            >
              <wui-text variant="md-medium" color="secondary" data-testid="group-title">
                ${n}
              </wui-text>
            </wui-flex>
            <wui-flex flexDirection="column" gap="2">
              ${this.templateTransactions(o,g)}
            </wui-flex>
          </wui-flex>
        `:null})})}templateRenderTransaction(e,i){const{date:r,descriptions:s,direction:n,images:o,status:c,type:g,transfers:m,isAllNFT:b}=this.getTransactionListItemProps(e);return w`
      <wui-transaction-list-item
        date=${r}
        .direction=${n}
        id=${i&&this.next?It:""}
        status=${c}
        type=${g}
        .images=${o}
        .onlyDirectionIcon=${b||m.length===1}
        .descriptions=${s}
      ></wui-transaction-list-item>
    `}templateTransactions(e,i){return e.map((r,s)=>{const n=i&&s===e.length-1;return w`${this.templateRenderTransaction(r,n)}`})}emptyStateActivity(){return w`<wui-flex
      class="emptyContainer"
      flexGrow="1"
      flexDirection="column"
      justifyContent="center"
      alignItems="center"
      .padding=${["10","5","10","5"]}
      gap="5"
      data-testid="empty-activity-state"
    >
      <wui-icon-box color="default" icon="wallet" size="xl"></wui-icon-box>
      <wui-flex flexDirection="column" alignItems="center" gap="2">
        <wui-text align="center" variant="lg-medium" color="primary">No Transactions yet</wui-text>
        <wui-text align="center" variant="lg-regular" color="secondary"
          >Start trading on dApps <br />
          to grow your wallet!</wui-text
        >
      </wui-flex>
    </wui-flex>`}emptyStateAccount(){return w`<wui-flex
      class="contentContainer"
      alignItems="center"
      justifyContent="center"
      flexDirection="column"
      gap="4"
      data-testid="empty-account-state"
    >
      <wui-icon-box icon="swapHorizontal" size="lg" color="default"></wui-icon-box>
      <wui-flex
        class="textContent"
        gap="2"
        flexDirection="column"
        justifyContent="center"
        flexDirection="column"
      >
        <wui-text variant="md-regular" align="center" color="primary">No activity yet</wui-text>
        <wui-text variant="sm-regular" align="center" color="secondary"
          >Your next transactions will appear here</wui-text
        >
      </wui-flex>
      <wui-link @click=${this.onReceiveClick.bind(this)}>Trade</wui-link>
    </wui-flex>`}templateEmpty(){return this.page==="account"?w`${this.emptyStateAccount()}`:w`${this.emptyStateActivity()}`}templateLoading(){return this.page==="activity"?w` <wui-flex flexDirection="column" width="100%">
        <wui-flex .padding=${["2","3","3","3"]}>
          <wui-shimmer width="70px" height="16px" rounded></wui-shimmer>
        </wui-flex>
        <wui-flex flexDirection="column" gap="2" width="100%">
          ${Array(Xt).fill(w` <wui-transaction-list-item-loader></wui-transaction-list-item-loader> `).map(e=>e)}
        </wui-flex>
      </wui-flex>`:null}onReceiveClick(){Ot.push("WalletReceive")}createPaginationObserver(){const{projectId:e}=At.state;this.paginationObserver=new IntersectionObserver(([i])=>{i?.isIntersecting&&!this.loading&&(F.fetchTransactions(gt.getPlainAddress(this.caipAddress)),kt.sendEvent({type:"track",event:"LOAD_MORE_TRANSACTIONS",properties:{address:gt.getPlainAddress(this.caipAddress),projectId:e,cursor:this.next,isSmartAccount:Ct(ft.state.activeChain)===Ft.ACCOUNT_TYPES.SMART_ACCOUNT}}))},{}),this.setPaginationObserver()}setPaginationObserver(){this.paginationObserver?.disconnect();const e=this.shadowRoot?.querySelector(`#${It}`);e&&this.paginationObserver?.observe(e)}getTransactionListItemProps(e){const i=yt.formatDate(e?.metadata?.minedAt),r=Z.mergeTransfers(e?.transfers||[]),s=Z.getTransactionDescriptions(e,r),n=r?.[0],o=!!n&&r?.every(g=>!!g.nft_info),c=Z.getTransactionImages(r);return{date:i,direction:n?.direction,descriptions:s,isAllNFT:o,images:c,status:e.metadata?.status,transfers:r,type:e.metadata?.operationType}}};L.styles=Qt;q([D()],L.prototype,"page",void 0);q([Q()],L.prototype,"caipAddress",void 0);q([Q()],L.prototype,"transactionsByYear",void 0);q([Q()],L.prototype,"loading",void 0);q([Q()],L.prototype,"empty",void 0);q([Q()],L.prototype,"next",void 0);L=q([ot("w3m-activity-list")],L);
