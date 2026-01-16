import { MidenFaucetApp } from './app.js';

// miden-sdk async import seems to interfere with this event. Related mdn:
// https://developer.mozilla.org/en-US/docs/Web/API/Document/DOMContentLoaded_event#checking_whether_loading_is_already_complete
if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", () => {
        new MidenFaucetApp();
    });
} else {
    new MidenFaucetApp();
}
