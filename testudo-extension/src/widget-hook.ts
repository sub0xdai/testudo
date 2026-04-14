// EXT-46: TradingView Widget Constructor Hook
// Runs in MAIN world at document_start — before any page JS executes.
// Intercepts TradingView.widget() constructor to capture widget instances
// that sites store in closures or under unknown variable names.
// The captured instance is stored on window.__TESTUDO_TV_WIDGET__ for
// consumption by page-bridge.ts findChartWidget().

(function () {
  const W = window as any;
  let _tv: any = W.TradingView;

  function patchConstructor(tv: any): void {
    if (!tv || typeof tv.widget !== "function" || tv.__testudo_patched__) return;
    const Orig = tv.widget;
    tv.widget = function (this: any, ...args: any[]) {
      const instance = new (Orig as any)(...args);
      W.__TESTUDO_TV_WIDGET__ = instance;
      return instance;
    };
    tv.widget.prototype = Orig.prototype;
    tv.__testudo_patched__ = true;
  }

  // If TradingView is already defined (unlikely at document_start), patch now
  if (_tv) {
    patchConstructor(_tv);
    return;
  }

  // Intercept future assignment of window.TradingView
  Object.defineProperty(W, "TradingView", {
    get() {
      return _tv;
    },
    set(val: any) {
      _tv = val;
      if (!val || typeof val !== "object") return;

      // If .widget is already a function, patch immediately
      if (typeof val.widget === "function") {
        patchConstructor(val);
        return;
      }

      // Otherwise, watch for .widget being assigned later (incremental pattern)
      let _widget: any = val.widget;
      Object.defineProperty(val, "widget", {
        get() {
          return _widget;
        },
        set(w: any) {
          _widget = w;
          if (typeof w === "function") patchConstructor(val);
        },
        configurable: true,
        enumerable: true,
      });
    },
    configurable: true,
    enumerable: true,
  });
})();
