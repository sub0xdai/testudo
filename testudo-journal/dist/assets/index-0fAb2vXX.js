import{c as b,a as v,i as $,b as o,r as _,f as k,G as u,j as C,H as O,R as T,O as P,E as U,h as L,W as N,I as F}from"./vendor-wallet-CK2hLz_N.js";import{n as c,r as y}from"./index-Ci7tY-ie.js";import"./index-DAJyWO4a.js";import"./index-C0Yn_V68.js";import{o as B}from"./if-defined-BZKeYABq.js";import"./index-Dn-b9k98.js";import"./index-Cx3qzZAt.js";var D;(function(e){e.approve="approved",e.bought="bought",e.borrow="borrowed",e.burn="burnt",e.cancel="canceled",e.claim="claimed",e.deploy="deployed",e.deposit="deposited",e.execute="executed",e.mint="minted",e.receive="received",e.repay="repaid",e.send="sent",e.sell="sold",e.stake="staked",e.trade="swapped",e.unstake="unstaked",e.withdraw="withdrawn"})(D||(D={}));const E=b`
  :host > wui-flex {
    display: flex;
    justify-content: center;
    align-items: center;
    position: relative;
    width: 40px;
    height: 40px;
    box-shadow: inset 0 0 0 1px ${({tokens:e})=>e.core.glass010};
    background-color: ${({tokens:e})=>e.theme.foregroundPrimary};
  }

  :host([data-no-images='true']) > wui-flex {
    background-color: ${({tokens:e})=>e.theme.foregroundPrimary};
    border-radius: ${({borderRadius:e})=>e[3]} !important;
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
    border-radius: ${({borderRadius:e})=>e[4]};
    background-color: ${({tokens:e})=>e.theme.backgroundPrimary};
    box-shadow: 0 0 0 2px ${({tokens:e})=>e.theme.backgroundPrimary};
    overflow: hidden;
    width: 16px;
    height: 16px;
  }
`;var f=function(e,t,i,a){var n=arguments.length,r=n<3?t:a===null?a=Object.getOwnPropertyDescriptor(t,i):a,s;if(typeof Reflect=="object"&&typeof Reflect.decorate=="function")r=Reflect.decorate(e,t,i,a);else for(var l=e.length-1;l>=0;l--)(s=e[l])&&(r=(n<3?s(r):n>3?s(t,i,r):s(t,i))||r);return n>3&&r&&Object.defineProperty(t,i,r),r};let d=class extends ${constructor(){super(...arguments),this.images=[],this.secondImage={type:void 0,url:""},this.failedImageUrls=new Set}handleImageError(t){return i=>{i.stopPropagation(),this.failedImageUrls.add(t),this.requestUpdate()}}render(){const[t,i]=this.images;this.images.length||(this.dataset.noImages="true");const a=t?.type==="NFT",n=i?.url?i.type==="NFT":a,r=a?"var(--apkt-borderRadius-3)":"var(--apkt-borderRadius-5)",s=n?"var(--apkt-borderRadius-3)":"var(--apkt-borderRadius-5)";return this.style.cssText=`
    --local-left-border-radius: ${r};
    --local-right-border-radius: ${s};
    `,o`<wui-flex> ${this.templateVisual()} ${this.templateIcon()} </wui-flex>`}templateVisual(){const[t,i]=this.images;return this.images.length===2&&(t?.url||i?.url)?this.renderSwapImages(t,i):t?.url&&!this.failedImageUrls.has(t.url)?this.renderSingleImage(t):t?.type==="NFT"?this.renderPlaceholderIcon("nftPlaceholder"):this.renderPlaceholderIcon("coinPlaceholder")}renderSwapImages(t,i){return o`<div class="swap-images-container">
      ${t?.url?this.renderImageOrFallback(t,"first",!0):null}
      ${i?.url?this.renderImageOrFallback(i,"last",!0):null}
    </div>`}renderSingleImage(t){return this.renderImageOrFallback(t,void 0,!1)}renderImageOrFallback(t,i,a=!1){return t.url?this.failedImageUrls.has(t.url)?a&&i?this.renderFallbackIconInContainer(i):this.renderFallbackIcon():o`<wui-image
      src=${t.url}
      alt="Transaction image"
      @onLoadError=${this.handleImageError(t.url)}
    ></wui-image>`:null}renderFallbackIconInContainer(t){return o`<div class="swap-fallback-container ${t}">${this.renderFallbackIcon()}</div>`}renderFallbackIcon(){return o`<wui-icon
      size="xl"
      weight="regular"
      color="default"
      name="networkPlaceholder"
    ></wui-icon>`}renderPlaceholderIcon(t){return o`<wui-icon size="xl" weight="regular" color="default" name=${t}></wui-icon>`}templateIcon(){let t="accent-primary",i;return i=this.getIcon(),this.status&&(t=this.getStatusColor()),i?o`
      <wui-flex alignItems="center" justifyContent="center" class="status-box">
        <wui-icon-box size="sm" color=${t} icon=${i}></wui-icon-box>
      </wui-flex>
    `:null}getDirectionIcon(){switch(this.direction){case"in":return"arrowBottom";case"out":return"arrowTop";default:return}}getIcon(){return this.onlyDirectionIcon?this.getDirectionIcon():this.type==="trade"?"swapHorizontal":this.type==="approve"?"checkmark":this.type==="cancel"?"close":this.getDirectionIcon()}getStatusColor(){switch(this.status){case"confirmed":return"success";case"failed":return"error";case"pending":return"inverse";default:return"accent-primary"}}};d.styles=[E];f([c()],d.prototype,"type",void 0);f([c()],d.prototype,"status",void 0);f([c()],d.prototype,"direction",void 0);f([c({type:Boolean})],d.prototype,"onlyDirectionIcon",void 0);f([c({type:Array})],d.prototype,"images",void 0);f([c({type:Object})],d.prototype,"secondImage",void 0);f([y()],d.prototype,"failedImageUrls",void 0);d=f([v("wui-transaction-visual")],d);const W=b`
  :host {
    width: 100%;
  }

  :host > wui-flex:first-child {
    align-items: center;
    column-gap: ${({spacing:e})=>e[2]};
    padding: ${({spacing:e})=>e[1]} ${({spacing:e})=>e[2]};
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
`;var g=function(e,t,i,a){var n=arguments.length,r=n<3?t:a===null?a=Object.getOwnPropertyDescriptor(t,i):a,s;if(typeof Reflect=="object"&&typeof Reflect.decorate=="function")r=Reflect.decorate(e,t,i,a);else for(var l=e.length-1;l>=0;l--)(s=e[l])&&(r=(n<3?s(r):n>3?s(t,i,r):s(t,i))||r);return n>3&&r&&Object.defineProperty(t,i,r),r};let p=class extends ${constructor(){super(...arguments),this.type="approve",this.onlyDirectionIcon=!1,this.images=[]}render(){return o`
      <wui-flex>
        <wui-transaction-visual
          .status=${this.status}
          direction=${B(this.direction)}
          type=${this.type}
          .onlyDirectionIcon=${this.onlyDirectionIcon}
          .images=${this.images}
        ></wui-transaction-visual>
        <wui-flex flexDirection="column" gap="1">
          <wui-text variant="lg-medium" color="primary">
            ${D[this.type]||this.type}
          </wui-text>
          <wui-flex class="description-container">
            ${this.templateDescription()} ${this.templateSecondDescription()}
          </wui-flex>
        </wui-flex>
        <wui-text variant="sm-medium" color="secondary"><span>${this.date}</span></wui-text>
      </wui-flex>
    `}templateDescription(){const t=this.descriptions?.[0];return t?o`
          <wui-text variant="md-regular" color="secondary">
            <span>${t}</span>
          </wui-text>
        `:null}templateSecondDescription(){const t=this.descriptions?.[1];return t?o`
          <wui-icon class="description-separator-icon" size="sm" name="arrowRight"></wui-icon>
          <wui-text variant="md-regular" color="secondary">
            <span>${t}</span>
          </wui-text>
        `:null}};p.styles=[_,W];g([c()],p.prototype,"type",void 0);g([c({type:Array})],p.prototype,"descriptions",void 0);g([c()],p.prototype,"date",void 0);g([c({type:Boolean})],p.prototype,"onlyDirectionIcon",void 0);g([c()],p.prototype,"status",void 0);g([c()],p.prototype,"direction",void 0);g([c({type:Array})],p.prototype,"images",void 0);p=g([v("wui-transaction-list-item")],p);const Y=b`
  wui-flex {
    position: relative;
    display: inline-flex;
    justify-content: center;
    align-items: center;
  }

  wui-image {
    border-radius: ${({borderRadius:e})=>e[128]};
  }

  .fallback-icon {
    color: ${({tokens:e})=>e.theme.iconInverse};
    border-radius: ${({borderRadius:e})=>e[3]};
    background-color: ${({tokens:e})=>e.theme.foregroundPrimary};
  }

  .direction-icon,
  .status-image {
    position: absolute;
    right: 0;
    bottom: 0;
    border-radius: ${({borderRadius:e})=>e[128]};
    border: 2px solid ${({tokens:e})=>e.theme.backgroundPrimary};
  }

  .direction-icon {
    padding: ${({spacing:e})=>e["01"]};
    color: ${({tokens:e})=>e.core.iconSuccess};

    background-color: color-mix(
      in srgb,
      ${({tokens:e})=>e.core.textSuccess} 30%,
      ${({tokens:e})=>e.theme.backgroundPrimary} 70%
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
    padding: ${({spacing:e})=>e[1]};
  }

  :host([data-size='lg']) .fallback-icon {
    height: 32px;
    width: 32px;
    padding: ${({spacing:e})=>e[1]};
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
`;var I=function(e,t,i,a){var n=arguments.length,r=n<3?t:a===null?a=Object.getOwnPropertyDescriptor(t,i):a,s;if(typeof Reflect=="object"&&typeof Reflect.decorate=="function")r=Reflect.decorate(e,t,i,a);else for(var l=e.length-1;l>=0;l--)(s=e[l])&&(r=(n<3?s(r):n>3?s(t,i,r):s(t,i))||r);return n>3&&r&&Object.defineProperty(t,i,r),r};const A={sm:"xxs",lg:"md"};let w=class extends ${constructor(){super(...arguments),this.type="approve",this.size="lg",this.statusImageUrl="",this.images=[]}render(){return o`<wui-flex>${this.templateVisual()} ${this.templateIcon()}</wui-flex>`}templateVisual(){switch(this.dataset.size=this.size,this.type){case"trade":return this.swapTemplate();case"fiat":return this.fiatTemplate();case"unknown":return this.unknownTemplate();default:return this.tokenTemplate()}}swapTemplate(){const[t,i]=this.images;return this.images.length===2&&(t||i)?o`
        <wui-image class="swap-crop-left-image" src=${t} alt="Swap image"></wui-image>
        <wui-image class="swap-crop-right-image" src=${i} alt="Swap image"></wui-image>
      `:t?o`<wui-image src=${t} alt="Swap image"></wui-image>`:null}fiatTemplate(){return o`<wui-icon
      class="fallback-icon"
      size=${A[this.size]}
      name="dollar"
    ></wui-icon>`}unknownTemplate(){return o`<wui-icon
      class="fallback-icon"
      size=${A[this.size]}
      name="questionMark"
    ></wui-icon>`}tokenTemplate(){const[t]=this.images;return t?o`<wui-image src=${t} alt="Token image"></wui-image> `:o`<wui-icon
      class="fallback-icon"
      name=${this.type==="nft"?"image":"coinPlaceholder"}
    ></wui-icon>`}templateIcon(){return this.statusImageUrl?o`<wui-image
        class="status-image"
        src=${this.statusImageUrl}
        alt="Status image"
      ></wui-image>`:o`<wui-icon
      class="direction-icon"
      size=${A[this.size]}
      name=${this.getTemplateIcon()}
    ></wui-icon>`}getTemplateIcon(){return this.type==="trade"?"arrowClockWise":"arrowBottom"}};w.styles=[Y];I([c()],w.prototype,"type",void 0);I([c()],w.prototype,"size",void 0);I([c()],w.prototype,"statusImageUrl",void 0);I([c({type:Array})],w.prototype,"images",void 0);w=I([v("wui-transaction-thumbnail")],w);const V=b`
  :host > wui-flex:first-child {
    gap: ${({spacing:e})=>e[2]};
    padding: ${({spacing:e})=>e[3]};
    width: 100%;
  }

  wui-flex {
    display: flex;
    flex: 1;
  }
`;var G=function(e,t,i,a){var n=arguments.length,r=n<3?t:a===null?a=Object.getOwnPropertyDescriptor(t,i):a,s;if(typeof Reflect=="object"&&typeof Reflect.decorate=="function")r=Reflect.decorate(e,t,i,a);else for(var l=e.length-1;l>=0;l--)(s=e[l])&&(r=(n<3?s(r):n>3?s(t,i,r):s(t,i))||r);return n>3&&r&&Object.defineProperty(t,i,r),r};let R=class extends ${render(){return o`
      <wui-flex alignItems="center" .padding=${["1","2","1","2"]}>
        <wui-shimmer width="40px" height="40px" rounded></wui-shimmer>
        <wui-flex flexDirection="column" gap="1">
          <wui-shimmer width="124px" height="16px" rounded></wui-shimmer>
          <wui-shimmer width="60px" height="14px" rounded></wui-shimmer>
        </wui-flex>
        <wui-shimmer width="24px" height="12px" rounded></wui-shimmer>
      </wui-flex>
    `}};R.styles=[_,V];R=G([v("wui-transaction-list-item-loader")],R);const M=b`
  :host {
    min-height: 100%;
  }

  .group-container[last-group='true'] {
    padding-bottom: ${({spacing:e})=>e[3]};
  }

  .contentContainer {
    height: 280px;
  }

  .contentContainer > wui-icon-box {
    width: 40px;
    height: 40px;
    border-radius: ${({borderRadius:e})=>e[3]};
  }

  .contentContainer > .textContent {
    width: 65%;
  }

  .emptyContainer {
    height: 100%;
  }
`;var x=function(e,t,i,a){var n=arguments.length,r=n<3?t:a===null?a=Object.getOwnPropertyDescriptor(t,i):a,s;if(typeof Reflect=="object"&&typeof Reflect.decorate=="function")r=Reflect.decorate(e,t,i,a);else for(var l=e.length-1;l>=0;l--)(s=e[l])&&(r=(n<3?s(r):n>3?s(t,i,r):s(t,i))||r);return n>3&&r&&Object.defineProperty(t,i,r),r};const z="last-transaction",H=7;let h=class extends ${constructor(){super(),this.unsubscribe=[],this.paginationObserver=void 0,this.page="activity",this.caipAddress=k.state.activeCaipAddress,this.transactionsByYear=u.state.transactionsByYear,this.loading=u.state.loading,this.empty=u.state.empty,this.next=u.state.next,u.clearCursor(),this.unsubscribe.push(k.subscribeKey("activeCaipAddress",t=>{t&&this.caipAddress!==t&&(u.resetTransactions(),u.fetchTransactions(t)),this.caipAddress=t}),k.subscribeKey("activeCaipNetwork",()=>{this.updateTransactionView()}),u.subscribe(t=>{this.transactionsByYear=t.transactionsByYear,this.loading=t.loading,this.empty=t.empty,this.next=t.next}))}firstUpdated(){this.updateTransactionView(),this.createPaginationObserver()}updated(){this.setPaginationObserver()}disconnectedCallback(){this.unsubscribe.forEach(t=>t())}render(){return o` ${this.empty?null:this.templateTransactionsByYear()}
    ${this.loading?this.templateLoading():null}
    ${!this.loading&&this.empty?this.templateEmpty():null}`}updateTransactionView(){u.resetTransactions(),this.caipAddress&&u.fetchTransactions(C.getPlainAddress(this.caipAddress))}templateTransactionsByYear(){return Object.keys(this.transactionsByYear).sort().reverse().map(i=>{const a=parseInt(i,10),n=new Array(12).fill(null).map((r,s)=>{const l=O.getTransactionGroupTitle(a,s),m=this.transactionsByYear[a]?.[s];return{groupTitle:l,transactions:m}}).filter(({transactions:r})=>r).reverse();return n.map(({groupTitle:r,transactions:s},l)=>{const m=l===n.length-1;return s?o`
          <wui-flex
            flexDirection="column"
            class="group-container"
            last-group="${m?"true":"false"}"
            data-testid="month-indexes"
          >
            <wui-flex
              alignItems="center"
              flexDirection="row"
              .padding=${["2","3","3","3"]}
            >
              <wui-text variant="md-medium" color="secondary" data-testid="group-title">
                ${r}
              </wui-text>
            </wui-flex>
            <wui-flex flexDirection="column" gap="2">
              ${this.templateTransactions(s,m)}
            </wui-flex>
          </wui-flex>
        `:null})})}templateRenderTransaction(t,i){const{date:a,descriptions:n,direction:r,images:s,status:l,type:m,transfers:S,isAllNFT:j}=this.getTransactionListItemProps(t);return o`
      <wui-transaction-list-item
        date=${a}
        .direction=${r}
        id=${i&&this.next?z:""}
        status=${l}
        type=${m}
        .images=${s}
        .onlyDirectionIcon=${j||S.length===1}
        .descriptions=${n}
      ></wui-transaction-list-item>
    `}templateTransactions(t,i){return t.map((a,n)=>{const r=i&&n===t.length-1;return o`${this.templateRenderTransaction(a,r)}`})}emptyStateActivity(){return o`<wui-flex
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
    </wui-flex>`}emptyStateAccount(){return o`<wui-flex
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
    </wui-flex>`}templateEmpty(){return this.page==="account"?o`${this.emptyStateAccount()}`:o`${this.emptyStateActivity()}`}templateLoading(){return this.page==="activity"?o` <wui-flex flexDirection="column" width="100%">
        <wui-flex .padding=${["2","3","3","3"]}>
          <wui-shimmer width="70px" height="16px" rounded></wui-shimmer>
        </wui-flex>
        <wui-flex flexDirection="column" gap="2" width="100%">
          ${Array(H).fill(o` <wui-transaction-list-item-loader></wui-transaction-list-item-loader> `).map(t=>t)}
        </wui-flex>
      </wui-flex>`:null}onReceiveClick(){T.push("WalletReceive")}createPaginationObserver(){const{projectId:t}=P.state;this.paginationObserver=new IntersectionObserver(([i])=>{i?.isIntersecting&&!this.loading&&(u.fetchTransactions(C.getPlainAddress(this.caipAddress)),U.sendEvent({type:"track",event:"LOAD_MORE_TRANSACTIONS",properties:{address:C.getPlainAddress(this.caipAddress),projectId:t,cursor:this.next,isSmartAccount:L(k.state.activeChain)===N.ACCOUNT_TYPES.SMART_ACCOUNT}}))},{}),this.setPaginationObserver()}setPaginationObserver(){this.paginationObserver?.disconnect();const t=this.shadowRoot?.querySelector(`#${z}`);t&&this.paginationObserver?.observe(t)}getTransactionListItemProps(t){const i=F.formatDate(t?.metadata?.minedAt),a=O.mergeTransfers(t?.transfers||[]),n=O.getTransactionDescriptions(t,a),r=a?.[0],s=!!r&&a?.every(m=>!!m.nft_info),l=O.getTransactionImages(a);return{date:i,direction:r?.direction,descriptions:n,isAllNFT:s,images:l,status:t.metadata?.status,transfers:a,type:t.metadata?.operationType}}};h.styles=M;x([c()],h.prototype,"page",void 0);x([y()],h.prototype,"caipAddress",void 0);x([y()],h.prototype,"transactionsByYear",void 0);x([y()],h.prototype,"loading",void 0);x([y()],h.prototype,"empty",void 0);x([y()],h.prototype,"next",void 0);h=x([v("w3m-activity-list")],h);
