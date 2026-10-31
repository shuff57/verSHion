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

- Pack prices and the custom rate (`DEFAULT_PACKS`, 10 credits per dollar) are placeholders. Set them from the
  measured cost per action on real tests; Stripe takes about 3% + 30 cents, so keep the minimum at $5 or more.
- Terms and refund wording (`server/web/legal`), and a stored-value check with counsel.
- Account deletion must delete the account's `credit_ledger` rows (not built yet).
