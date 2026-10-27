// A shared password gate for the OUTRAM PARK browser demos (gh:#986).
//
// ─────────────────────────────────────────────────────────────────────────
//  THIS IS NOT SECURITY. Read this before relying on it for anything.
// ─────────────────────────────────────────────────────────────────────────
//
// Everything a browser runs is visible to whoever is running it. This file,
// the hash below and the check itself all ship to the client, so anyone who
// opens the developer tools can skip the gate, and the wasm is fetched from
// the same public URL either way. What this *does* do is keep a demo off the
// open web for casual visitors and search engines — a "staff only" sign, not
// a lock.
//
// So: do not put anything behind it that would matter if it were read. The
// demos are already research-and-education material under RESPONSIBLE_USE.md
// with nothing sensitive in them, which is the only reason a sign is enough.
// If a demo ever needs a real lock, it needs a server that refuses to send
// the files — an HTTP auth in front of the site, not JavaScript inside it.
//
// ─── Changing the password ───────────────────────────────────────────────
//
// The password is stored as a SHA-256 hash so it is not sitting in the page
// in plain text. To change it, run:
//
//     printf '%s' 'the-new-password' | sha256sum
//
// and paste the hex into PASSWORD_SHA256 below. Nothing else changes, and a
// reader who has already unlocked this session keeps their access until the
// tab is closed (the flag lives in sessionStorage, not localStorage, so it
// does not persist across browser restarts).

/** SHA-256 of the demo password. Replace to rotate — see the header. */
const PASSWORD_SHA256 =
  "1b773ab7214c8b38e091fb4604efc082f96d74ba445b63971dd6d64d9658ac77";

/** sessionStorage key; bump the suffix to force everyone to re-enter. */
const UNLOCKED_KEY = "outram-park-demo-unlocked-v1";

async function sha256Hex(text) {
  const bytes = new TextEncoder().encode(text);
  const digest = await crypto.subtle.digest("SHA-256", bytes);
  return Array.from(new Uint8Array(digest))
    .map((b) => b.toString(16).padStart(2, "0"))
    .join("");
}

function alreadyUnlocked() {
  try {
    return sessionStorage.getItem(UNLOCKED_KEY) === "1";
  } catch {
    // Private mode, or storage blocked. Ask again rather than failing shut:
    // the gate is a sign, and a sign that cannot be read should not wedge
    // the page.
    return false;
  }
}

function remember() {
  try {
    sessionStorage.setItem(UNLOCKED_KEY, "1");
  } catch {
    /* nothing to do: the reader will be asked again next load */
  }
}

/**
 * Show the gate, resolving once the reader is through it.
 *
 * `title` names the demo on the card. Mobile-first, like everything else
 * here: the card is a single column that fits a phone, the input is
 * `type=password` with autocomplete off, and Enter submits.
 */
export function unlock(title) {
  return new Promise((resolve) => {
    if (alreadyUnlocked()) {
      resolve();
      return;
    }

    const wrap = document.createElement("div");
    wrap.id = "gate";
    wrap.innerHTML = `
      <div class="gate-card">
        <h1>${title}</h1>
        <p>This demo is not public. Enter the password to continue.</p>
        <form id="gate-form" autocomplete="off">
          <input id="gate-input" type="password" inputmode="text"
                 autocomplete="current-password" placeholder="Password"
                 aria-label="Password" />
          <button type="submit">Enter</button>
        </form>
        <p id="gate-error" role="alert" hidden>Not that one. Try again.</p>
        <p class="gate-note">Research and education only. An AI draft, not
           validated; not for operation, licensing or safety decisions.</p>
      </div>`;

    const style = document.createElement("style");
    style.textContent = `
      #gate { position: fixed; inset: 0; z-index: 9999; display: flex;
              align-items: center; justify-content: center; padding: 16px;
              background: #0e1014; color: #d8dce4;
              font: 15px/1.5 system-ui, -apple-system, "Segoe UI", sans-serif; }
      .gate-card { width: 100%; max-width: 26em; }
      .gate-card h1 { font-size: 1.25rem; margin: 0 0 .5em; }
      .gate-card p { margin: 0 0 1em; color: #aab2c0; }
      #gate-form { display: flex; gap: 8px; flex-wrap: wrap; }
      #gate-input { flex: 1 1 12em; min-width: 0; padding: .7em .8em;
                    font-size: 16px; /* 16px stops iOS zooming on focus */
                    border-radius: 8px; border: 1px solid #39414f;
                    background: #161a21; color: #e8ecf4; }
      #gate-form button { padding: .7em 1.2em; font-size: 16px;
                          border-radius: 8px; border: 0; cursor: pointer;
                          background: #3b6ea5; color: #fff; }
      #gate-error { color: #e48; margin-top: 1em; }
      .gate-note { font-size: .85rem; margin-top: 1.5em; color: #78808e; }`;

    document.head.appendChild(style);
    document.body.appendChild(wrap);

    const form = wrap.querySelector("#gate-form");
    const input = wrap.querySelector("#gate-input");
    const error = wrap.querySelector("#gate-error");
    input.focus();

    form.addEventListener("submit", async (ev) => {
      ev.preventDefault();
      const hex = await sha256Hex(input.value);
      if (hex === PASSWORD_SHA256) {
        remember();
        wrap.remove();
        style.remove();
        resolve();
      } else {
        error.hidden = false;
        input.value = "";
        input.focus();
      }
    });
  });
}
