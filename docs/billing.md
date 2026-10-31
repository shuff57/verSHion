# Buying credits

Teachers get a free weekly allowance (`AI_WEEKLY_CREDITS`). Purchased credits are spent after it, never expire,
and are deleted with the account. Server code: `server/worker/src/billing`, ledger and charging:
`server/worker/src/ai/store.ts`. Buy page: `server/web/account/credits.html`.

## Flow

1. The page (Cloudflare Pages, same registrable domain as the API) signs in through `POST /auth/refresh`.
2. `GET /billing/packs` lists the packs; `POST /billing/intent {pack}` or `{amount_usd}` creates a Stripe
   PaymentIntent. The server prices it; the client's credit count is never used.
3. The page mounts Stripe's Payment Element and confirms the payment.
4. Stripe calls `POST /billing/webhook` (signed). `payment_intent.succeeded` credits the ledger once
   (unique per payment id). `charge.refunded` and `charge.dispute.created` take credits back pro rata and
   may leave the balance negative, which blocks purchased spending but not the free week.

## Settings (all unset means off)

| Name | Kind | Purpose |
|---|---|---|
| `BILLING_ENABLED` | var | `"1"` turns buying on. The webhook works whenever its secret is set. |
| `STRIPE_SECRET_KEY` | secret | `sk_test_...` until launch |
| `STRIPE_WEBHOOK_SECRET` | secret | `whsec_...` of this Worker's own endpoint |
| `STRIPE_PUBLISHABLE_KEY` | var | `pk_test_...`, returned by `/billing/packs` |

Stripe dashboard: add a webhook endpoint `https://<api>/billing/webhook` for `payment_intent.succeeded`,
`charge.refunded`, `charge.dispute.created`. Use test mode first. Before the page goes live set `api-base`
in `credits.html` and the API origin in `server/web/_headers`, and add the page's origin to `ALLOWED_ORIGINS`.

## To decide before launch

- Prices: 25 credits per dollar (a credit is 4 cents), with packs $5 = 125, $15 = 400, $40 = 1,100 and a custom
  whole-dollar amount from $5 to $200 at the base rate. At those prices a handwriting page is 4 cents and a test
  import 8 cents. The model cost is a fraction of that (Gemma 4 26B is about $0.07 per million input tokens and
  $0.34 per million output tokens on OpenRouter, roughly a tenth of a cent per quiz page), so the margin is large
  even after Stripe's 2.9% + 30 cents. Those costs are estimates: check them on real tests (link `ai_usage` rows
  to their session first) before launch. The free weekly allowance stays at 40 credits, about $1.60 of value.
- Limits: 10 payments started per account per hour; the body reader requires Content-Length and caps sizes.
- Terms and refund wording (`server/web/legal`), and a stored-value check with counsel.
- Deleting an account erases its `credit_ledger`, `ai_sessions` and `ai_usage` rows (in the pool's erasure batch
  and its daily resweep), so unspent purchased credits are lost, as the owner chose. A payment that arrives after
  that credits nothing and is logged for a manual refund in Stripe.
