import{c as p,r as f,e as h,a as b,i as w,N as d,b as m}from"./vendor-wallet-haM69aOm.js";import{n as i}from"./index-BZTiCSAG.js";import"./index-416g1Doe.js";const g=p`
  :host {
    width: 100%;
  }

  button {
    padding: ${({spacing:e})=>e[3]};
    display: flex;
    gap: ${({spacing:e})=>e[3]};
    justify-content: space-between;
    width: 100%;
    border-radius: ${({borderRadius:e})=>e[4]};
    background-color: transparent;
  }

  @media (hover: hover) {
    button:hover:enabled {
      background-color: ${({tokens:e})=>e.theme.foregroundSecondary};
    }
  }

  button:focus-visible:enabled {
    background-color: ${({tokens:e})=>e.theme.foregroundSecondary};
    box-shadow: 0 0 0 4px ${({tokens:e})=>e.core.foregroundAccent040};
  }

  button[data-clickable='false'] {
    pointer-events: none;
    background-color: transparent;
  }

  wui-image,
  wui-icon {
    width: ${({spacing:e})=>e[10]};
    height: ${({spacing:e})=>e[10]};
  }

  wui-image {
    border-radius: ${({borderRadius:e})=>e[16]};
  }

  .token-name-container {
    flex: 1;
  }
`;var n=function(e,r,a,l){var u=arguments.length,o=u<3?r:l===null?l=Object.getOwnPropertyDescriptor(r,a):l,c;if(typeof Reflect=="object"&&typeof Reflect.decorate=="function")o=Reflect.decorate(e,r,a,l);else for(var s=e.length-1;s>=0;s--)(c=e[s])&&(o=(u<3?c(o):u>3?c(r,a,o):c(r,a))||o);return u>3&&o&&Object.defineProperty(r,a,o),o};let t=class extends w{constructor(){super(...arguments),this.tokenName="",this.tokenImageUrl="",this.tokenValue=0,this.tokenAmount="0.0",this.tokenCurrency="",this.clickable=!1}render(){return m`
      <button data-clickable=${String(this.clickable)}>
        <wui-flex gap="2" alignItems="center">
          ${this.visualTemplate()}
          <wui-flex
            flexDirection="column"
            justifyContent="space-between"
            gap="1"
            class="token-name-container"
          >
            <wui-text variant="md-regular" color="primary" lineClamp="1">
              ${this.tokenName}
            </wui-text>
            <wui-text variant="sm-regular-mono" color="secondary">
              ${d.formatNumberToLocalString(this.tokenAmount,4)} ${this.tokenCurrency}
            </wui-text>
          </wui-flex>
        </wui-flex>
        <wui-flex
          flexDirection="column"
          justifyContent="space-between"
          gap="1"
          alignItems="flex-end"
          width="auto"
        >
          <wui-text variant="md-regular-mono" color="primary"
            >$${this.tokenValue.toFixed(2)}</wui-text
          >
          <wui-text variant="sm-regular-mono" color="secondary">
            ${d.formatNumberToLocalString(this.tokenAmount,4)}
          </wui-text>
        </wui-flex>
      </button>
    `}visualTemplate(){return this.tokenName&&this.tokenImageUrl?m`<wui-image alt=${this.tokenName} src=${this.tokenImageUrl}></wui-image>`:m`<wui-icon name="coinPlaceholder" color="default"></wui-icon>`}};t.styles=[f,h,g];n([i()],t.prototype,"tokenName",void 0);n([i()],t.prototype,"tokenImageUrl",void 0);n([i({type:Number})],t.prototype,"tokenValue",void 0);n([i()],t.prototype,"tokenAmount",void 0);n([i()],t.prototype,"tokenCurrency",void 0);n([i({type:Boolean})],t.prototype,"clickable",void 0);t=n([b("wui-list-token")],t);
