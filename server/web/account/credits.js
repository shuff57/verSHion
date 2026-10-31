// Buy credits: sign in with the API's cookie session, choose a pack or amount, pay with Stripe's Payment Element.
// The server prices everything; this page only says which pack or whole-dollar amount was picked.
const API = document.querySelector('meta[name="api-base"]').content.replace(/\/+$/, "");
const $ = (id) => document.getElementById(id);
const show = (id, on) => ($(id).hidden = !on);

let token = null;
let config = null; // { packs, custom, publishable_key }
let stripe = null;
let elements = null;
let before = 0; // purchased balance when the payment started

function say(text) {
  const s = $("status");
  s.textContent = text;
  s.hidden = !text;
}

async function api(path, init = {}) {
  const res = await fetch(API + path, {
    ...init,
    credentials: "include",
    headers: { ...(init.body ? { "content-type": "application/json" } : {}), ...(token ? { authorization: "Bearer " + token } : {}) },
  });
  let body = null;
  try {
    body = await res.json();
  } catch {}
  return { ok: res.ok, status: res.status, body };
}

async function refresh() {
  const r = await api("/auth/refresh", { method: "POST", body: "{}" });
  token = r.ok ? r.body.access_token : null;
  return token !== null;
}

async function loadBalance() {
  const r = await api("/ai/credits");
  if (!r.ok || r.body.purchased === undefined) return null;
  $("balance").textContent = `Purchased credits: ${r.body.purchased}. Free this week: ${r.body.left} of ${r.body.allowance}.`;
  show("balance", true);
  return r.body.purchased;
}

// ---- choosing what to buy ----------------------------------------------------------------------

function selection() {
  const pick = document.querySelector('input[name="pick"]:checked');
  if (!pick) return null;
  if (pick.value === "custom") {
    const usd = Number($("custom-usd").value);
    const { min_usd, max_usd, credits_per_usd } = config.custom;
    if (!Number.isInteger(usd) || usd < min_usd || usd > max_usd) return { error: `Enter a whole number of dollars from ${min_usd} to ${max_usd}.` };
    return { body: { amount_usd: usd }, usd, credits: usd * credits_per_usd };
  }
  const p = config.packs.find((x) => x.id === pick.value);
  return { body: { pack: p.id }, usd: p.usd, credits: p.credits };
}

function updateSummary() {
  const sel = selection();
  $("amount-error").hidden = true;
  $("summary").textContent = sel && !sel.error ? `${sel.credits} credits for $${sel.usd}.` : "";
  $("custom-usd").disabled = document.querySelector('input[name="pick"]:checked')?.value !== "custom";
}

function renderPacks() {
  const box = $("packs");
  box.replaceChildren();
  config.packs.forEach((p, i) => {
    const wrap = document.createElement("div");
    wrap.className = "pack";
    const input = Object.assign(document.createElement("input"), { type: "radio", name: "pick", id: "pick-" + p.id, value: p.id, checked: i === 0 });
    const label = document.createElement("label");
    label.htmlFor = input.id;
    const strong = document.createElement("strong");
    strong.textContent = `$${p.usd}`;
    label.append(strong, `${p.credits} credits`);
    wrap.append(input, label);
    box.append(wrap);
  });
  const { min_usd, max_usd } = config.custom;
  $("custom-usd").min = min_usd;
  $("custom-usd").max = max_usd;
  updateSummary();
}

// ---- paying ------------------------------------------------------------------------------------

function loadStripe() {
  if (window.Stripe) return Promise.resolve();
  return new Promise((resolve, reject) => {
    const s = Object.assign(document.createElement("script"), { src: "https://js.stripe.com/v3/", async: true });
    s.onload = resolve;
    s.onerror = () => reject(new Error("stripe.js did not load"));
    document.head.append(s);
  });
}

async function startPayment(sel) {
  $("continue").disabled = true;
  say("Preparing the payment...");
  const r = await api("/billing/intent", { method: "POST", body: JSON.stringify(sel.body) });
  $("continue").disabled = false;
  if (!r.ok) {
    say("");
    $("amount-error").textContent = r.body?.error?.message ?? "Could not start the payment. Try again.";
    $("amount-error").hidden = false;
    return;
  }
  try {
    await loadStripe();
  } catch {
    say("Stripe could not be loaded. Check your connection and try again.");
    return;
  }
  say("");
  before = (await loadBalance()) ?? 0;
  try {
    sessionStorage.setItem("credits-before", String(before)); // a redirecting payment method comes back to a fresh page
  } catch {}
  stripe =window.Stripe(r.body.publishable_key ?? config.publishable_key);
  const dark = matchMedia("(prefers-color-scheme: dark)").matches;
  elements = stripe.elements({ clientSecret: r.body.client_secret, appearance: { theme: dark ? "night" : "stripe" } });
  $("payment-element").replaceChildren();
  elements.create("payment").mount("#payment-element");
  $("pay-summary").textContent = `${r.body.credits} credits for $${r.body.amount_usd}.`;
  show("shop", false);
  show("pay", true);
  $("pay-button").focus();
}

async function pay(ev) {
  ev.preventDefault();
  $("pay-button").disabled = true;
  $("pay-error").hidden = true;
  const { error, paymentIntent } = await stripe.confirmPayment({
    elements,
    confirmParams: { return_url: location.origin + location.pathname },
    redirect: "if_required",
  });
  if (error) {
    $("pay-error").textContent = error.message ?? "The payment did not go through.";
    $("pay-error").hidden = false;
    $("pay-button").disabled = false;
    return;
  }
  await finish(paymentIntent.status);
}

// The webhook credits the ledger, a moment after Stripe confirms. Wait for the balance to move.
async function finish(status) {
  show("shop", false);
  show("pay", false);
  show("done", true);
  if (status !== "succeeded" && status !== "processing") {
    $("done-title").textContent = "Payment not completed";
    $("done-text").textContent = "Nothing was charged. You can go back and try again.";
    return;
  }
  $("done-title").textContent = status === "succeeded" ? "Payment received" : "Payment processing";
  $("done-text").textContent = "Adding your credits...";
  for (let i = 0; i < 20; i++) {
    const now = await loadBalance();
    if (now !== null && now > before) {
      $("done-text").textContent = `Your credits are ready. You have ${now} purchased credits.`;
      return;
    }
    await new Promise((r) => setTimeout(r, 1500));
  }
  $("done-text").textContent = "The payment went through, but the credits are taking longer than usual. They will appear in the app shortly; if not, contact us.";
}

// ---- start -------------------------------------------------------------------------------------

async function showShop() {
  const r = await api("/billing/packs");
  if (r.status === 404) {
    show("closed", true);
    await loadBalance();
    return;
  }
  if (!r.ok) {
    say("Could not load the credit packs. Try again later.");
    return;
  }
  config = r.body;
  renderPacks();
  show("shop", true);
  await loadBalance();
}

async function returnedFromRedirect() {
  const secret = new URLSearchParams(location.search).get("payment_intent_client_secret");
  if (!secret) return false;
  const pk = (await api("/billing/packs")).body?.publishable_key;
  if (!pk) return false;
  await loadStripe();
  const { paymentIntent } = await window.Stripe(pk).retrievePaymentIntent(secret);
  history.replaceState(null, "", location.pathname);
  try {
    before = Number(sessionStorage.getItem("credits-before") ?? "-1");
  } catch {
    before = -1;
  }
  await finish(paymentIntent?.status ?? "failed");
  return true;
}

async function start() {
  say("Loading...");
  if (!(await refresh())) {
    say("");
    show("signin", true);
    $("email").focus();
    return;
  }
  say("");
  show("signin", false);
  if (!(await returnedFromRedirect())) await showShop();
}

$("signin-form").addEventListener("submit", async (ev) => {
  ev.preventDefault();
  const r = await api("/auth/login", { method: "POST", body: JSON.stringify({ email: $("email").value.trim(), password: $("password").value }) });
  $("password").value = "";
  if (!r.ok || !r.body?.access_token) {
    say(r.status === 429 ? "Too many attempts. Wait a few minutes." : "Could not sign in. Check your email and password, or finish verifying your account in the app.");
    return;
  }
  token = r.body.access_token;
  say("");
  show("signin", false);
  await showShop();
});

document.addEventListener("change", (ev) => {
  if (ev.target.name === "pick") {
    updateSummary();
    if (ev.target.value === "custom") $("custom-usd").focus();
  }
});
$("custom-usd").addEventListener("input", () => {
  const sel = selection();
  $("summary").textContent = sel && !sel.error ? `${sel.credits} credits for $${sel.usd}.` : "";
});
$("amount-form").addEventListener("submit", (ev) => {
  ev.preventDefault();
  const sel = selection();
  if (!sel || sel.error) {
    $("amount-error").textContent = sel?.error ?? "Choose an amount.";
    $("amount-error").hidden = false;
    return;
  }
  startPayment(sel);
});
$("pay-form").addEventListener("submit", pay);
$("pay-back").addEventListener("click", () => {
  show("pay", false);
  show("shop", true);
  $("continue").focus();
});

start();
