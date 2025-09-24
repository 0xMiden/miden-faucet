(() => {
  var __create = Object.create;
  var __defProp = Object.defineProperty;
  var __getOwnPropDesc = Object.getOwnPropertyDescriptor;
  var __getOwnPropNames = Object.getOwnPropertyNames;
  var __getProtoOf = Object.getPrototypeOf;
  var __hasOwnProp = Object.prototype.hasOwnProperty;
  var __commonJS = (cb, mod) => function __require() {
    return mod || (0, cb[__getOwnPropNames(cb)[0]])((mod = { exports: {} }).exports, mod), mod.exports;
  };
  var __copyProps = (to, from, except, desc) => {
    if (from && typeof from === "object" || typeof from === "function") {
      for (let key of __getOwnPropNames(from))
        if (!__hasOwnProp.call(to, key) && key !== except)
          __defProp(to, key, { get: () => from[key], enumerable: !(desc = __getOwnPropDesc(from, key)) || desc.enumerable });
    }
    return to;
  };
  var __toESM = (mod, isNodeMode, target) => (target = mod != null ? __create(__getProtoOf(mod)) : {}, __copyProps(
    // If the importer is in node compatibility mode or this is not an ESM
    // file that has been converted to a CommonJS file using a Babel-
    // compatible transform (i.e. "__esModule" has not been set), then set
    // "default" to the CommonJS "module.exports" for node compatibility.
    isNodeMode || !mod || !mod.__esModule ? __defProp(target, "default", { value: mod, enumerable: true }) : target,
    mod
  ));

  // node_modules/eventemitter3/index.js
  var require_eventemitter3 = __commonJS({
    "node_modules/eventemitter3/index.js"(exports, module) {
      "use strict";
      var has = Object.prototype.hasOwnProperty;
      var prefix = "~";
      function Events() {
      }
      if (Object.create) {
        Events.prototype = /* @__PURE__ */ Object.create(null);
        if (!new Events().__proto__)
          prefix = false;
      }
      function EE(fn, context, once) {
        this.fn = fn;
        this.context = context;
        this.once = once || false;
      }
      function addListener(emitter, event, fn, context, once) {
        if (typeof fn !== "function") {
          throw new TypeError("The listener must be a function");
        }
        var listener = new EE(fn, context || emitter, once), evt = prefix ? prefix + event : event;
        if (!emitter._events[evt])
          emitter._events[evt] = listener, emitter._eventsCount++;
        else if (!emitter._events[evt].fn)
          emitter._events[evt].push(listener);
        else
          emitter._events[evt] = [emitter._events[evt], listener];
        return emitter;
      }
      function clearEvent(emitter, evt) {
        if (--emitter._eventsCount === 0)
          emitter._events = new Events();
        else
          delete emitter._events[evt];
      }
      function EventEmitter2() {
        this._events = new Events();
        this._eventsCount = 0;
      }
      EventEmitter2.prototype.eventNames = function eventNames() {
        var names = [], events, name;
        if (this._eventsCount === 0)
          return names;
        for (name in events = this._events) {
          if (has.call(events, name))
            names.push(prefix ? name.slice(1) : name);
        }
        if (Object.getOwnPropertySymbols) {
          return names.concat(Object.getOwnPropertySymbols(events));
        }
        return names;
      };
      EventEmitter2.prototype.listeners = function listeners(event) {
        var evt = prefix ? prefix + event : event, handlers = this._events[evt];
        if (!handlers)
          return [];
        if (handlers.fn)
          return [handlers.fn];
        for (var i = 0, l = handlers.length, ee = new Array(l); i < l; i++) {
          ee[i] = handlers[i].fn;
        }
        return ee;
      };
      EventEmitter2.prototype.listenerCount = function listenerCount(event) {
        var evt = prefix ? prefix + event : event, listeners = this._events[evt];
        if (!listeners)
          return 0;
        if (listeners.fn)
          return 1;
        return listeners.length;
      };
      EventEmitter2.prototype.emit = function emit(event, a1, a2, a3, a4, a5) {
        var evt = prefix ? prefix + event : event;
        if (!this._events[evt])
          return false;
        var listeners = this._events[evt], len = arguments.length, args, i;
        if (listeners.fn) {
          if (listeners.once)
            this.removeListener(event, listeners.fn, void 0, true);
          switch (len) {
            case 1:
              return listeners.fn.call(listeners.context), true;
            case 2:
              return listeners.fn.call(listeners.context, a1), true;
            case 3:
              return listeners.fn.call(listeners.context, a1, a2), true;
            case 4:
              return listeners.fn.call(listeners.context, a1, a2, a3), true;
            case 5:
              return listeners.fn.call(listeners.context, a1, a2, a3, a4), true;
            case 6:
              return listeners.fn.call(listeners.context, a1, a2, a3, a4, a5), true;
          }
          for (i = 1, args = new Array(len - 1); i < len; i++) {
            args[i - 1] = arguments[i];
          }
          listeners.fn.apply(listeners.context, args);
        } else {
          var length = listeners.length, j;
          for (i = 0; i < length; i++) {
            if (listeners[i].once)
              this.removeListener(event, listeners[i].fn, void 0, true);
            switch (len) {
              case 1:
                listeners[i].fn.call(listeners[i].context);
                break;
              case 2:
                listeners[i].fn.call(listeners[i].context, a1);
                break;
              case 3:
                listeners[i].fn.call(listeners[i].context, a1, a2);
                break;
              case 4:
                listeners[i].fn.call(listeners[i].context, a1, a2, a3);
                break;
              default:
                if (!args)
                  for (j = 1, args = new Array(len - 1); j < len; j++) {
                    args[j - 1] = arguments[j];
                  }
                listeners[i].fn.apply(listeners[i].context, args);
            }
          }
        }
        return true;
      };
      EventEmitter2.prototype.on = function on(event, fn, context) {
        return addListener(this, event, fn, context, false);
      };
      EventEmitter2.prototype.once = function once(event, fn, context) {
        return addListener(this, event, fn, context, true);
      };
      EventEmitter2.prototype.removeListener = function removeListener(event, fn, context, once) {
        var evt = prefix ? prefix + event : event;
        if (!this._events[evt])
          return this;
        if (!fn) {
          clearEvent(this, evt);
          return this;
        }
        var listeners = this._events[evt];
        if (listeners.fn) {
          if (listeners.fn === fn && (!once || listeners.once) && (!context || listeners.context === context)) {
            clearEvent(this, evt);
          }
        } else {
          for (var i = 0, events = [], length = listeners.length; i < length; i++) {
            if (listeners[i].fn !== fn || once && !listeners[i].once || context && listeners[i].context !== context) {
              events.push(listeners[i]);
            }
          }
          if (events.length)
            this._events[evt] = events.length === 1 ? events[0] : events;
          else
            clearEvent(this, evt);
        }
        return this;
      };
      EventEmitter2.prototype.removeAllListeners = function removeAllListeners(event) {
        var evt;
        if (event) {
          evt = prefix ? prefix + event : event;
          if (this._events[evt])
            clearEvent(this, evt);
        } else {
          this._events = new Events();
          this._eventsCount = 0;
        }
        return this;
      };
      EventEmitter2.prototype.off = EventEmitter2.prototype.removeListener;
      EventEmitter2.prototype.addListener = EventEmitter2.prototype.on;
      EventEmitter2.prefixed = prefix;
      EventEmitter2.EventEmitter = EventEmitter2;
      if ("undefined" !== typeof module) {
        module.exports = EventEmitter2;
      }
    }
  });

  // node_modules/eventemitter3/index.mjs
  var import_index = __toESM(require_eventemitter3(), 1);
  var eventemitter3_default = import_index.default;

  // node_modules/@demox-labs/miden-wallet-adapter-base/dist/adapter.js
  var WalletReadyState;
  (function(WalletReadyState2) {
    WalletReadyState2["Installed"] = "Installed";
    WalletReadyState2["NotDetected"] = "NotDetected";
    WalletReadyState2["Loadable"] = "Loadable";
    WalletReadyState2["Unsupported"] = "Unsupported";
  })(WalletReadyState || (WalletReadyState = {}));
  var BaseWalletAdapter = class extends eventemitter3_default {
    get connected() {
      return !!this.accountId;
    }
  };
  function scopePollingDetectionStrategy(detect) {
    if (typeof window === "undefined" || typeof document === "undefined")
      return;
    const disposers = [];
    function detectAndDispose() {
      const detected = detect();
      if (detected) {
        for (const dispose of disposers) {
          dispose();
        }
      }
    }
    const interval = (
      // TODO: #334 Replace with idle callback strategy.
      setInterval(detectAndDispose, 1e3)
    );
    disposers.push(() => clearInterval(interval));
    if (
      // Implies that `DOMContentLoaded` has not yet fired.
      document.readyState === "loading"
    ) {
      document.addEventListener("DOMContentLoaded", detectAndDispose, {
        once: true
      });
      disposers.push(() => document.removeEventListener("DOMContentLoaded", detectAndDispose));
    }
    if (
      // If the `complete` state has been reached, we're too late.
      document.readyState !== "complete"
    ) {
      window.addEventListener("load", detectAndDispose, { once: true });
      disposers.push(() => window.removeEventListener("load", detectAndDispose));
    }
    detectAndDispose();
  }

  // node_modules/@demox-labs/miden-wallet-adapter-base/dist/errors.js
  var WalletError = class extends Error {
    error;
    // eslint-disable-next-line @typescript-eslint/explicit-module-boundary-types
    constructor(message, error) {
      super(message);
      this.error = error;
    }
  };
  var WalletNotReadyError = class extends WalletError {
    name = "WalletNotReadyError";
  };
  var WalletConnectionError = class extends WalletError {
    name = "WalletConnectionError";
  };
  var WalletDisconnectionError = class extends WalletError {
    name = "WalletDisconnectionError";
  };
  var WalletNotConnectedError = class extends WalletError {
    name = "WalletNotConnectedError";
  };
  var WalletTransactionError = class extends WalletError {
    name = "WalletTransactionError";
  };

  // node_modules/@demox-labs/miden-wallet-adapter-base/dist/signer.js
  var BaseSignerWalletAdapter = class extends BaseWalletAdapter {
  };
  var BaseMessageSignerWalletAdapter = class extends BaseSignerWalletAdapter {
  };

  // node_modules/@demox-labs/miden-wallet-adapter-base/dist/types.js
  var WalletAdapterNetwork;
  (function(WalletAdapterNetwork2) {
    WalletAdapterNetwork2["Testnet"] = "testnet";
    WalletAdapterNetwork2["Localnet"] = "localnet";
  })(WalletAdapterNetwork || (WalletAdapterNetwork = {}));
  var PrivateDataPermission;
  (function(PrivateDataPermission2) {
    PrivateDataPermission2["UponRequest"] = "UPON_REQUEST";
    PrivateDataPermission2["Auto"] = "AUTO";
  })(PrivateDataPermission || (PrivateDataPermission = {}));
  var AllowedPrivateData;
  (function(AllowedPrivateData2) {
    AllowedPrivateData2[AllowedPrivateData2["None"] = 0] = "None";
    AllowedPrivateData2[AllowedPrivateData2["Assets"] = 1] = "Assets";
    AllowedPrivateData2[AllowedPrivateData2["Notes"] = 2] = "Notes";
    AllowedPrivateData2[AllowedPrivateData2["Storage"] = 4] = "Storage";
    AllowedPrivateData2[AllowedPrivateData2["All"] = 65535] = "All";
  })(AllowedPrivateData || (AllowedPrivateData = {}));

  // node_modules/@demox-labs/miden-wallet-adapter-base/dist/transaction.js
  var TransactionType;
  (function(TransactionType2) {
    TransactionType2["Send"] = "send";
    TransactionType2["Consume"] = "consume";
    TransactionType2["Custom"] = "custom";
  })(TransactionType || (TransactionType = {}));

  // node_modules/@demox-labs/miden-wallet-adapter-miden/dist/adapter.js
  var MidenWalletName = "Miden Wallet";
  var MidenWalletAdapter = class extends BaseMessageSignerWalletAdapter {
    name = MidenWalletName;
    url = "https://miden.fi/";
    icon = "data:image/png;base64,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+d+QHitReQ6CQfjdhRVkpcTp36GifqVUhNH+vdtWzsYryF2lxqmT9yAYhMM8KVVVG5gaFRv9k2l97Qd+pYKEeZoSU9T2DS1wYlW1XvEJzVFtJO6oIFlVTWkudTA37aogRf4ul82mp/WKKokE1XakItqyCW+Ujq4KlP0uw+meiUhn1XekDyqYS2j11y1h+9rem8b2DXNUpcIVP/p6pTPi+8oSflav35WIZIJq1a3w+aIteKS0Mln89lU62zedigpEjKACESCoQAQIKhABggpEgKACESCoQAQIKhABggpEgKACESCoQAQIKhABggpEgKACESCoQAQIKhABggpEgKACESCoQAQIKhABggpEgKACESCoQAQIKhABggpEgKACESCoQAQIKhABggpEgKACESCoQAQIKhABggpEgKACESCoQAQIKhABggpEgKACESCoQAQIKhABggpEgKACESCoQAQIKhCBmoBIDWra/ofUUQIIKqLV2XH79rCvBND6AhEgqEAECCoQAYIKRICgAhEgqEAECCoQAYIKRICgAhEgqEAECCoQAYIKRICgAnP4qeUbj1q+roLx1zPAhBDGj0O1nHRDXmveq+6cGuG5TwN7scf1215y2rfn+/Zu3z73gz12v6ypt33wVz0LRVAB88Nt37Tg3bX4tS2MdXfsOefO+SILsL1eCy/2dS17fPLnIAtx1zu9+ldVO4sKLUFF0h7c8W3ndd+C1rTgLUrTvmfTQvvL+h3fuVbV5s87rq8rIKhIkgUoVMAtC2dDefJZhW5fNbAsJiEpYf5premuBeh17iE97iCwew++9Y90CQQVyVj/zj/8NNSeQpu7HGFRaisMFGHAmOcLCSqSYHPRLY20PV4AWramDRi7P37j12b9AoKKUgvHPG1Fd88Wdy7VcubG2u5RRXtW5e/P8ukEFaUVQvrXQLv+4BDKahqpM0tYCSpKKYqQHpohrAQVpfTHUE+iCOkhmz9fNGclqCidB7f9k5Wbk05XH1X1un3OecQEFaUSDns4aUMxsgWm6sCO756BoKJUwmEPxa0ZTmuc/CBBRWmElrfQs41yYm371mQLTFBRClnL69RWOdRrg5NzbIKKUrCWtxTV9JCtWD88XlUJKqI3Pm+2qXI5UVUJKqL3cZj9LWlDJROq6uHbBBXRq/jPO3TJ1LMrT4igInKh7Y3qDKQ5eafs1EKCiqhlbW+Z+exaTAQVcXPSXZVbPZwDTFARu9K2vYes/W0SVEQrO85YwtXeSRbUBkFFtOw4Y+mracYTVETMKs0qXP8ofxXdIKiIli0kNZQCrzpBRbxW44qCRSCoQAwIKhABgop4hdsepmGfoCJao2EaQbVFsz5BRbSqX6inNFBREa9Pyu72XXpe+kBQEa1OuJu3L39YvVOPoCJuFf2mkqsOCSoiZwst5Z6nOvWf/eoIKuI2qKqjcuuGVwQVUcvmqeOduYyc16vwSFARPef0RmVkbe/zd64b3iSoiF7W/pbwLCU7LLN5+DZBRfRC+2st4lOViVXTUfVzS09QUQqDmrbLVFXD3NQGoP7h+wQVpRCq6vFWMWoHc9ON4x8iqCiNl2/dtkqwAnzWgENQUSqVmh5H3QI7dWzA6Ux+mKCiVJ7tuF60LbC1vMOqDTRnIKgonXELHNcqsHUBFtJb4xM4TiGoKKUX71y4t2hXkRiHtH/e8wQVpTWs6Z4iOGm/4vS9hfTC/ydBRWmFNtLCekvj82VXjrW7Trr17IzFo0kEFaUWwvrivWu7VVtgOlg4unl4Lu80BBVJCCcQeGsxQ0C0bE47IaQXzUknEVQkIxyfDIs2FpTltMLW6obB4sVbd++81d3zEFQkJVQxC0pohW8VVl0P5qKbNkhcfznDfPQsNQEJGs8Nr6/f8S3n9dBLTS1aCKjX00FV2/NW0EkEFUmz6rpjDzvtlm9UhmpVvO77q9zF3GXX4N3xttJsAe1dNaCHCCqgg5bYHsIZTdshtNWh1qwarjmnGxa6hg93jnPHbvMYziceWcV02UtvKH2oVtV9NuV4KAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAs1d/ArZRIBz8JggAAAABJRU5ErkJggg==";
    supportedTransactionVersions = null;
    _connecting;
    _wallet;
    _accountId;
    _publicKey;
    _privateDataPermission;
    _readyState = typeof window === "undefined" || typeof document === "undefined" ? WalletReadyState.Unsupported : WalletReadyState.NotDetected;
    constructor({ appName = "sample" } = {}) {
      super();
      this._connecting = false;
      this._wallet = null;
      this._accountId = null;
      this._publicKey = null;
      this._privateDataPermission = PrivateDataPermission.UponRequest;
      if (this._readyState !== WalletReadyState.Unsupported) {
        scopePollingDetectionStrategy(() => {
          if (window?.midenWallet || window?.miden) {
            this._readyState = WalletReadyState.Installed;
            this.emit("readyStateChange", this._readyState);
            return true;
          }
          return false;
        });
      }
    }
    get accountId() {
      return this._accountId;
    }
    get publicKey() {
      return this._publicKey;
    }
    get privateDataPermission() {
      return this._privateDataPermission;
    }
    get connecting() {
      return this._connecting;
    }
    get readyState() {
      return this._readyState;
    }
    set readyState(readyState) {
      this._readyState = readyState;
    }
    async requestSend(transaction) {
      try {
        const wallet = this._wallet;
        if (!wallet || !this.accountId)
          throw new WalletNotConnectedError();
        try {
          const result = await wallet.requestSend(transaction);
          return result.transactionId;
        } catch (error) {
          throw new WalletTransactionError(error?.message, error);
        }
      } catch (error) {
        this.emit("error", error);
        throw error;
      }
    }
    async requestConsume(transaction) {
      try {
        const wallet = this._wallet;
        if (!wallet || !this.accountId)
          throw new WalletNotConnectedError();
        try {
          const result = await wallet.requestConsume(transaction);
          return result.transactionId;
        } catch (error) {
          throw new WalletTransactionError(error?.message, error);
        }
      } catch (error) {
        this.emit("error", error);
        throw error;
      }
    }
    async requestTransaction(transaction) {
      try {
        const wallet = this._wallet;
        if (!wallet || !this.accountId)
          throw new WalletNotConnectedError();
        try {
          const result = await wallet.requestTransaction(transaction);
          return result.transactionId;
        } catch (error) {
          throw new WalletTransactionError(error?.message, error);
        }
      } catch (error) {
        this.emit("error", error);
        throw error;
      }
    }
    async requestAssets() {
      try {
        const wallet = this._wallet;
        if (!wallet || !this.accountId)
          throw new WalletNotConnectedError();
        try {
          const result = await wallet.requestAssets();
          return result.assets;
        } catch (error) {
          throw new WalletTransactionError(error?.message, error);
        }
      } catch (error) {
        this.emit("error", error);
        throw error;
      }
    }
    async requestPrivateNotes() {
      try {
        const wallet = this._wallet;
        if (!wallet || !this.accountId)
          throw new WalletNotConnectedError();
        try {
          const result = await wallet.requestPrivateNotes();
          return result.privateNotes;
        } catch (error) {
          throw new WalletTransactionError(error?.message, error);
        }
      } catch (error) {
        this.emit("error", error);
        throw error;
      }
    }
    async signMessage(message) {
      try {
        const wallet = this._wallet;
        if (!wallet || !this.accountId)
          throw new WalletNotConnectedError();
        try {
          const result = await wallet.signMessage(message);
          return result.signature;
        } catch (error) {
          throw new WalletTransactionError(error?.message, error);
        }
      } catch (error) {
        this.emit("error", error);
        throw error;
      }
    }
    async connect(privateDataPermission, network, allowedPrivateData) {
      try {
        if (this.connected || this.connecting)
          return;
        if (this._readyState !== WalletReadyState.Installed)
          throw new WalletNotReadyError();
        this._connecting = true;
        const wallet = window.midenWallet || window.miden;
        try {
          await wallet.connect(privateDataPermission, network, allowedPrivateData);
          if (!wallet?.accountId) {
            throw new WalletConnectionError();
          }
          this._accountId = wallet.accountId;
          this._publicKey = wallet.publicKey;
        } catch (error) {
          throw new WalletConnectionError(error?.message, error);
        }
        this._wallet = wallet;
        this._privateDataPermission = privateDataPermission;
        this.emit("connect", this._accountId);
      } catch (error) {
        this.emit("error", error);
        throw error;
      } finally {
        this._connecting = false;
      }
    }
    async disconnect() {
      const wallet = this._wallet;
      if (wallet) {
        this._wallet = null;
        this._accountId = null;
        this._publicKey = null;
        try {
          await wallet.disconnect();
        } catch (error) {
          this.emit("error", new WalletDisconnectionError(error?.message, error));
        }
      }
      this.emit("disconnect");
    }
  };

  // index.js
  var MidenFaucet = class {
    constructor() {
      this.recipientInput = document.getElementById("recipient-address");
      this.tokenSelect = document.getElementById("token-amount");
      this.privateButton = document.getElementById("send-private-button");
      this.publicButton = document.getElementById("send-public-button");
      this.walletConnectButton = document.getElementById("wallet-connect-button");
      this.faucetAddress = document.getElementById("faucet-address");
      this.progressFill = document.getElementById("progress-fill");
      this.issuance = document.getElementById("issuance");
      this.tokensSupply = document.getElementById("tokens-supply");
      this.tokenAmountOptions = [100, 500, 1e3];
      this.explorer_url = null;
      this.metadataInitialized = false;
      if (!window.crypto || !window.crypto.subtle) {
        console.error("Web Crypto API not available");
        this.showError("Web Crypto API not available. Please use a modern browser.");
      }
      this.startMetadataPolling();
      this.privateButton.addEventListener("click", () => this.handleSendTokens(true));
      this.publicButton.addEventListener("click", () => this.handleSendTokens(false));
      this.walletConnectButton.addEventListener("click", () => this.handleWalletConnect());
      this.walletAdapter = new MidenWalletAdapter({ appName: "Miden Faucet" });
    }
    async handleWalletConnect() {
      try {
        await this.walletAdapter.connect(PrivateDataPermission.UponRequest, WalletAdapterNetwork.Testnet);
        if (this.walletAdapter.accountId) {
          this.recipientInput.value = this.walletAdapter.accountId;
        }
      } catch (error) {
        console.error("WalletConnectionError:", error);
        this.showError("Failed to connect wallet.");
      }
    }
    async handleSendTokens(isPrivateNote) {
      const recipient = this.recipientInput.value.trim();
      const amount = this.tokenSelect.value;
      const amountAsTokens = this.tokenSelect[this.tokenSelect.selectedIndex].textContent;
      if (!recipient) {
        this.showError("Recipient address is required.");
        return;
      }
      if (!amount || amount === "0") {
        this.showError("Amount is required.");
        return;
      }
      if (!Utils.validateAddress(recipient)) {
        this.showError("Please enter a valid recipient address.");
        return;
      }
      this.hideMessages();
      this.showMintingModal(recipient, amountAsTokens, isPrivateNote);
      this.updateProgressBar(0);
      this.updateMintingTitle("PREPARING THE REQUEST");
      const powData = await this.getPowChallenge(recipient);
      if (!powData) {
        this.hideModals();
        return;
      }
      const nonce = await Utils.findValidNonce(powData.challenge, powData.target);
      this.updateMintingTitle("MINTING TOKENS");
      this.updateProgressBar(50);
      try {
        await this.getTokens(powData.challenge, nonce, recipient, amount, amountAsTokens, isPrivateNote);
      } catch (error) {
        this.showError(`Failed to send tokens: ${error.message}`);
      }
    }
    startMetadataPolling() {
      this.fetchMetadata();
      this.metadataInterval = setInterval(() => {
        this.fetchMetadata();
      }, 2e3);
    }
    async fetchMetadata() {
      fetch(window.location.origin + "/get_metadata").then((response) => response.json()).then((data) => {
        if (!this.metadataInitialized) {
          this.faucetAddress.textContent = data.id;
          this.explorer_url = data.explorer_url;
          this.tokenSelect.innerHTML = "";
          for (const amount of this.tokenAmountOptions) {
            const option = document.createElement("option");
            option.value = Utils.tokensToBaseUnits(amount, data.decimals);
            option.textContent = amount;
            this.tokenSelect.appendChild(option);
          }
          this.metadataInitialized = true;
        }
        this.issuance.textContent = Utils.baseUnitsToTokens(data.issuance, data.decimals);
        this.tokensSupply.textContent = Utils.baseUnitsToTokens(data.max_supply, data.decimals);
        this.progressFill.style.width = data.issuance / data.max_supply * 100 + "%";
      }).catch((error) => {
        console.error("Error fetching metadata:", error);
        this.showError("Failed to load metadata. Please try again.");
      });
    }
    async getPowChallenge(recipient) {
      let powResponse;
      try {
        powResponse = await fetch(window.location.origin + "/pow?" + new URLSearchParams({
          account_id: recipient
        }), {
          method: "GET"
        });
      } catch (error) {
        this.showError("Connection failed.");
        return;
      }
      if (!powResponse.ok) {
        const message = await powResponse.text();
        this.showError(message);
        return;
      }
      return await powResponse.json();
    }
    async getTokens(challenge, nonce, recipient, amount, amountAsTokens, isPrivateNote) {
      const params = {
        account_id: recipient,
        is_private_note: isPrivateNote,
        asset_amount: parseInt(amount),
        challenge,
        nonce
      };
      let response;
      try {
        response = await fetch(window.location.origin + "/get_tokens?" + new URLSearchParams(params), {
          method: "GET"
        });
      } catch (error) {
        this.showError("Connection failed.");
        console.error(error);
        return;
      }
      if (!response.ok) {
        const message = await response.text();
        this.showError("Failed to receive tokens: " + message);
        return;
      }
      let data = await response.json();
      this.showCompletedModal(recipient, amountAsTokens, isPrivateNote, data);
    }
    async requestNote(noteId) {
      this.hidePrivateModalError();
      let response;
      try {
        response = await fetch(window.location.origin + "/get_note?" + new URLSearchParams({
          note_id: noteId
        }));
      } catch (error) {
        this.showPrivateModalError("Connection failed.");
        return;
      }
      if (!response.ok) {
        this.showPrivateModalError("Failed to download note: " + await response.text());
        return;
      }
      const data = await response.json();
      const binaryString = atob(data.data_base64);
      const byteArray = new Uint8Array(binaryString.length);
      for (let i = 0; i < binaryString.length; i++) {
        byteArray[i] = binaryString.charCodeAt(i);
      }
      const blob = new Blob([byteArray], { type: "application/octet-stream" });
      Utils.downloadBlob(blob, "note.mno");
      this.showNoteDownloadedMessage();
    }
    showNoteDownloadedMessage() {
      const continueText = document.getElementById("private-continue-text");
      continueText.style.visibility = "visible";
    }
    hideModals() {
      const mintingModal = document.getElementById("minting-modal");
      mintingModal.classList.remove("active");
      const completedPrivateModal = document.getElementById("completed-private-modal");
      completedPrivateModal.classList.remove("active");
      const completedPublicModal = document.getElementById("completed-public-modal");
      completedPublicModal.classList.remove("active");
      this.hideProgressBar();
    }
    showMintingModal(recipient, amountAsTokens, isPrivateNote) {
      const modal = document.getElementById("minting-modal");
      const tokenAmount = document.getElementById("modal-token-amount");
      const recipientAddress = document.getElementById("modal-recipient-address");
      const noteType = document.getElementById("modal-note-type");
      tokenAmount.textContent = amountAsTokens;
      recipientAddress.textContent = recipient;
      noteType.textContent = isPrivateNote ? "PRIVATE" : "PUBLIC";
      modal.classList.add("active");
    }
    showCompletedModal(recipient, amountAsTokens, isPrivateNote, mintingData) {
      const mintingModal = document.getElementById("minting-modal");
      mintingModal.classList.remove("active");
      document.getElementById("completed-public-token-amount").textContent = amountAsTokens;
      document.getElementById("completed-public-recipient-address").textContent = recipient;
      document.getElementById("completed-private-token-amount").textContent = amountAsTokens;
      document.getElementById("completed-private-recipient-address").textContent = recipient;
      this.updateMintingTitle("TOKENS MINTED!");
      const completedPrivateModal = document.getElementById("completed-private-modal");
      const completedPublicModal = document.getElementById("completed-public-modal");
      this.updateProgressBar(100);
      if (isPrivateNote) {
        completedPrivateModal.classList.add("active");
        const downloadButton = document.getElementById("download-button");
        downloadButton.onclick = async () => {
          await this.requestNote(mintingData.note_id);
          const closeButton = document.getElementById("private-close-button");
          closeButton.style.display = "block";
          closeButton.onclick = () => {
            closeButton.style.display = "none";
            this.hideMessages();
            this.hideModals();
            this.resetForm();
          };
        };
      } else {
        completedPublicModal.classList.add("active");
        const explorerButton = document.getElementById("explorer-button");
        if (this.explorer_url) {
          explorerButton.style.display = "block";
          explorerButton.onclick = () => window.open(this.explorer_url + "tx/" + mintingData.tx_id, "_blank");
        } else {
          explorerButton.style.display = "none";
        }
        completedPublicModal.onclick = (e) => {
          const continueText = document.getElementById("public-continue-text");
          if (e.target === completedPublicModal || e.target === continueText) {
            this.hideModals();
            this.resetForm();
          }
        };
      }
    }
    updateMintingTitle(title) {
      const mintingTitle = document.getElementById("minting-title");
      mintingTitle.textContent = title;
    }
    showPublicModalError(message) {
      const publicModalError = document.getElementById("public-error-message");
      publicModalError.textContent = message;
      publicModalError.style.display = "block";
    }
    showPrivateModalError(message) {
      const privateModalError = document.getElementById("private-error-message");
      privateModalError.textContent = message;
      privateModalError.style.display = "block";
    }
    hidePrivateModalError() {
      const privateModalError = document.getElementById("private-error-message");
      privateModalError.style.display = "none";
    }
    showError(message) {
      this.hideModals();
      const errorMessage = document.getElementById("error-message");
      errorMessage.textContent = message;
      errorMessage.style.display = "block";
    }
    hideMessages() {
      const errorMessage = document.getElementById("error-message");
      errorMessage.style.display = "none";
      const privateModalError = document.getElementById("private-error-message");
      privateModalError.style.display = "none";
      const publicModalError = document.getElementById("public-error-message");
      publicModalError.style.display = "none";
      const continueText = document.getElementById("private-continue-text");
      continueText.style.visibility = "hidden";
    }
    resetForm() {
      this.recipientInput.value = "";
    }
    updateProgressBar(progress) {
      this.showProgressBar();
      const progressBarFill = document.getElementById("progress-bar-fill");
      progressBarFill.style.width = progress + "%";
    }
    showProgressBar() {
      const progressBarTotal = document.getElementById("progress-bar-total");
      progressBarTotal.classList.add("active");
    }
    hideProgressBar() {
      this.updateProgressBar(0);
      const progressBarTotal = document.getElementById("progress-bar-total");
      progressBarTotal.classList.remove("active");
    }
  };
  document.addEventListener("DOMContentLoaded", () => {
    new MidenFaucet();
  });
  var Utils = {
    validateAddress: (address) => {
      return /^(0x[0-9a-fA-F]{30}|[a-z]{1,4}1[a-z0-9]{35})$/i.test(address);
    },
    findValidNonce: async (challenge, target) => {
      let nonce = 0;
      let targetNum = BigInt(target);
      while (true) {
        nonce = Math.floor(Math.random() * Number.MAX_SAFE_INTEGER);
        try {
          const challengeBytes = new TextEncoder().encode(challenge);
          const nonceBytes = new ArrayBuffer(8);
          const nonceView = new DataView(nonceBytes);
          nonceView.setBigUint64(0, BigInt(nonce), false);
          const nonceByteArray = new Uint8Array(nonceBytes);
          const combined = new Uint8Array(challengeBytes.length + nonceByteArray.length);
          combined.set(challengeBytes);
          combined.set(nonceByteArray, challengeBytes.length);
          const hashBuffer = await window.crypto.subtle.digest("SHA-256", combined);
          const hashArray = new Uint8Array(hashBuffer);
          const first8Bytes = hashArray.slice(0, 8);
          const dataView = new DataView(first8Bytes.buffer);
          const digest = dataView.getBigUint64(0, false);
          if (digest < targetNum) {
            return nonce;
          }
        } catch (error) {
          console.error("Error computing hash:", error);
          throw new Error("Failed to compute hash: " + error.message);
        }
        if (nonce % 1e3 === 0) {
          await new Promise((resolve) => setTimeout(resolve, 0));
        }
      }
    },
    downloadBlob: (blob, filename) => {
      const url = window.URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.style.display = "none";
      a.href = url;
      a.download = filename;
      document.body.appendChild(a);
      a.click();
      a.remove();
      window.URL.revokeObjectURL(url);
    },
    base64ToBlob: (base64) => {
      const binaryString = atob(base64);
      const byteArray = new Uint8Array(binaryString.length);
      for (let i = 0; i < binaryString.length; i++) {
        byteArray[i] = binaryString.charCodeAt(i);
      }
      return new Blob([byteArray], { type: "application/octet-stream" });
    },
    baseUnitsToTokens: (baseUnits, decimals) => {
      return (baseUnits / 10 ** decimals).toLocaleString();
    },
    tokensToBaseUnits: (tokens, decimals) => {
      return tokens * 10 ** decimals;
    }
  };
})();
