# Clicking through the sharing windows against a local server

This runs a private copy of the pool server on your own computer, at `http://localhost:18791`.
Staging and production are never contacted. Its accounts, shared items and settings are kept in
`server/worker/.wrangler-ui/`, which git ignores. Email is never sent: each message is written to
a log file, and `links` reads the links back out of it.

All commands below are run from `server/worker/`. `rig` is short for `scripts/local-ui-rig.sh`.

## 1. Start the server

```sh
cd server/worker
scripts/local-ui-rig.sh start
```

The first start builds the share-core wasm, so it can take a minute or two. It ends with
`Ready: http://localhost:18791`. It refuses to start if port 18791 (or 18792, for devtools) is
already in use, and it never stops another program to free the port.

Other commands:

| Command | What it does |
|---|---|
| `rig status` | Says whether the server is running. |
| `rig log` | Follows the server's output. Press Ctrl+C to stop following; the server keeps running. |
| `rig links [EMAIL]` | Prints the newest confirm, reset and device links for each address, or for one address. |
| `rig approve EMAIL` | Approves the account as a teacher and gives it pool access without an invite code. |
| `rig approve EMAIL --teacher-only` | Approves the account but gives no pool access. It still needs an invite code. |
| `rig invite` | Prints a new single-use invite code. |
| `rig stop` | Stops the server. Accounts and items are kept for next time. |
| `rig reset` | Stops the server and deletes `.wrangler-ui/`, so the next start is empty. |

The script never sees a password. You type passwords only into the app.

## 2. Launch the app and point it at the local server

Use a debug build. Only debug builds let you type a server address.

```sh
cd <repo root>
cargo run
```

To open the app with the Account window already showing, run
`VERSHION_SCREEN=account cargo run` instead.

1. Go to **Settings** in the left navigation. Under **Sharing account**, click **Open account…**.
   A red **DEVELOPMENT BUILD: custom server** banner is expected.
2. In **Server address**, type `http://localhost:18791` and click **Save**.
3. The window asks **Send your password to this server?** with `localhost:18791` shown. Click
   **Yes, it is my school's server**.

## 3. Account A: sign up, confirm, approve

1. Click **Create an account**. Fill in a made-up address (for example `a@example.test`; nothing
   is actually sent to it), a password of at least 10 characters typed twice, a display name, a
   school, and tick **I am an adult employee of a school**. Click **Create account**.
2. The window changes to **Check your email**. In a terminal, run
   `scripts/local-ui-rig.sh links a@example.test` and copy the URL on the `confirm` line.
3. Paste it into **Link or code from the email** and click **Continue**.
4. On **Finish your account**, type the password again (twice), fill in the name and school, tick
   the box, and click **Confirm**.
5. Sign in with the address and password. The window should show **Waiting for approval**, with
   an **Invite code** section below it.
6. Run `scripts/local-ui-rig.sh approve a@example.test`, then click **Check now**. The window
   should now say **Approved**, with **Browse shared questions** and **My shared items** buttons.

## 4. Account B, for import and merge tests

Account B runs in a second copy of the app with its own empty data folder:

```sh
VERSHION_STORE_DIR=/tmp/vershion-b cargo run
```

Enter the server address and confirm it again, as in step 2. If running two copies at once is
awkward, you can sign A out and sign B in within one app instead.

Account B is the one to use for the invite-code tests:

1. Sign up as `b@example.test`, confirm with `rig links b@example.test`, and sign in, as in step 3.
2. **Before approval:** run `rig invite` and type the code into **Invite code**, then click
   **Redeem**. It should say **Redeemed. You can use the shared pool.** The status should still be
   **Waiting for approval**.
3. Run `rig approve b@example.test` (B already has a redemption, so this only approves it), then
   click **Check now**.

To test the code **after** approval, use a third account C. Approve it with
`rig approve c@example.test --teacher-only`. Open **Browse shared questions**: it should say the
pool is invite-only. Then redeem a fresh `rig invite` code in the Account window. Browsing should
now work.

## 5. Click-through checklist

### Publish (account A)
- [ ] **From a bank:** open a bank that has questions and click **Share to pool…**. Choose the
      options, tick **Yes, I confirm this**, then click **Next: check what students will see**.
      Look through the preview, tick the review box, and share. The success page should offer
      **My shared items**.
- [ ] **From an exam:** open a saved exam and click **Share to pool…**. The button is greyed out
      until the exam is saved. Go through the same steps.
- [ ] Share the same bank again: the window should offer **Replace it with my version** or
      **Share as a new item**.
- [ ] With **Hide the answer key** ticked, check that the preview shows no answers.

### Browse and import (account B)
- [ ] Click **Browse shared questions** in the Account window, search, page through the results,
      and **Open** A's item.
- [ ] Click **Import...** to bring it in as a new bank. The bank should show up under Banks.
- [ ] Inside a bank, click **From pool…** to import into that bank.
- [ ] **Report...** is offered on A's item but not on B's own items.

### Update and merge
- [ ] As A, edit the shared bank (change one question, add one), then share again and choose
      **Replace it with my version**.
- [ ] As B, open the imported bank. Use **Update from pool…**, or go to Browse, then Imported, then
      **Check for updates** and **Update...**.
- [ ] Try **Add as a new bank...**. B's bank should not change.
- [ ] Try **Merge into this bank...**: check each row, then click **Apply merge**. Only the rows
      you chose should change.
- [ ] Clicking **Cancel** in a merge should change nothing.

### My shared items (account A)
- [ ] Open **My shared items** from the Account window and from the publish success page. A's
      items should be listed with their states, and **Open** should show one item.
- [ ] Click **Delete it** on an item: it should be marked as deleted, and B should no longer find
      it in Browse.
- [ ] Click **My data, account and legal pages**, then **Download my data**, then
      **Save to a file...**. A file should be saved.

### Error cases
- [ ] **Wrong invite code:** typing `0000-0000-0000-0000-0000-0000` should be refused as not
      valid. The typed code stays in the box so it can be fixed.
- [ ] **Malformed code** (too short, for example `ABCD`): should be refused with a plain message.
- [ ] **Invite needed:** on a `--teacher-only` account, Browse, Publish and My shared items should
      all say the pool is invite-only.
- [ ] **Not approved yet:** a pending account should have no Browse or My shared items buttons,
      and **Share to pool…** should say the account is waiting for approval and offer
      **Open account...**.
- [ ] **Sign out:** click **Sign out**. Browse, Publish and My shared items should then ask you to
      sign in (**Open account...**). Sign back in and they should work again.
- [ ] **Sign out everywhere:** sign out in one copy of the app; the other copy should be signed
      out on its next request.
- [ ] **Server down:** run `rig stop` while a window is open. The app should show an error and a
      **Try again** button, not hang. Run `rig start` and click **Try again**.
- [ ] **Wrong password** at sign-in: should be refused with a plain message.
- [ ] **Forgot password:** click **Forgot your password?** and send the reset link. Get it with
      `rig links`, paste it, and set a new password.

## 6. Clean up

```sh
scripts/local-ui-rig.sh stop     # keep the accounts for another session
scripts/local-ui-rig.sh reset    # or: delete everything (accounts, items, the throwaway keys, the mail log)
```

The app remembers the server address and your sign-in in its own settings and the system keychain.
Sign out (or click **Use a different server**) before pointing it at a real server.
