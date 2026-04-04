import{d as f,i as a,b as p,a as m}from"./vendor-wallet-wu6U4HC0.js";import"./index-Bp6jjra8.js";import"./index-Cykr1Clt.js";import"./index-CucsL0-Z.js";import"./if-defined-Z7ElDQHl.js";import"./index--U0jLELY.js";import"./index-jfdPCUBw.js";import"./index-Bx3vT0OA.js";const d=f`
  :host > wui-flex:first-child {
    height: 500px;
    overflow-y: auto;
    overflow-x: hidden;
    scrollbar-width: none;
  }

  :host > wui-flex:first-child::-webkit-scrollbar {
    display: none;
  }
`;var u=function(o,t,i,r){var n=arguments.length,e=n<3?t:r===null?r=Object.getOwnPropertyDescriptor(t,i):r,l;if(typeof Reflect=="object"&&typeof Reflect.decorate=="function")e=Reflect.decorate(o,t,i,r);else for(var s=o.length-1;s>=0;s--)(l=o[s])&&(e=(n<3?l(e):n>3?l(t,i,e):l(t,i))||e);return n>3&&e&&Object.defineProperty(t,i,e),e};let c=class extends a{render(){return p`
      <wui-flex flexDirection="column" .padding=${["0","3","3","3"]} gap="3">
        <w3m-activity-list page="activity"></w3m-activity-list>
      </wui-flex>
    `}};c.styles=d;c=u([m("w3m-transactions-view")],c);export{c as W3mTransactionsView};
