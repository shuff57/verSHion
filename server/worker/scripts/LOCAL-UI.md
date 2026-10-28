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
| `rig seed` | Adds 3 sample shared items and a circle with 2 banks (the first one at 3 versions, for the history and merge) from a throwaway account `b@example.test`, and prints 2 circle invite codes (see below). Run again, it adds only what is missing and prints 2 fresh codes. |
| `rig stop` | Stops the server. Accounts and items are kept for next time. |
| `rig reset` | Stops the server and deletes `.wrangler-ui/`, so the next start is empty. |

The script never sees a password. You type passwords only into the app.

### Real mail (optional)

By default the server prints each mail to `.wrangler-ui/dev.log` and `rig links` reads the
newest link out of it. To send through an SMTP server instead, make the file
`server/worker/.wrangler-ui/real-mail.env` (the directory is mode 700; keep the file 600):

```
SMTP_HOSTNAME=smtp.yourdomain.org
SMTP_PORT=465
SMTP_USERNAME=vershion@yourdomain.org
SMTP_PASSWORD=the-mailbox-password
SMTP_FROM_NAME="verSHion"
MAIL_FROM="verSHion <vershion@yourdomain.org>"
```

`MAIL_FROM` may be `Name <addr>` or a bare address; SMTP sends only the address in `MAIL FROM` (Stalwart refuses the name form with 501) and the name comes from `SMTP_FROM_NAME`.
`SMTP_PORT=465` means implicit TLS (what the owner's Stalwart publishes); 587 does STARTTLS.
`SMTP_USERNAME` is the mailbox that signs in, and the sender unless `MAIL_FROM` names a different
one. Make the file in your own editor or terminal, so the password never passes through a shell
command that is logged. Then `rig stop` and `rig start`: it reports `Mail: real, through SMTP
(real-mail.env)`. `rig status` says which mode is on. (Resend is still wired for a deployment that
prefers it: `MAIL_FROM` plus `RESEND_API_KEY` sends that way when no SMTP settings are present.)

In real-mail mode nothing is written to the log, so `rig links`, `rig approve` and `rig seed`
cannot find their links (`seed` refuses). Sign up in the app and use the message that arrives:
paste the whole link, or just the long code after `#token=`, into the app's **Link or code from
the email** box. The link's page is not served by the local server (there is no web app in this
repo), so opening it in a browser only gets a 404; pasting it into the app is the path that works.

`rig reset` deletes `real-mail.env` with the rest of `.wrangler-ui/`.

### Sample items (`rig seed`)

`rig seed` fills an empty pool with three public items, so Browse and Import have something to
show. It goes through the real API, as the app would: it signs up `b@example.test`, confirms it
with the link from the mail log, signs in, runs `rig approve b@example.test`, and publishes:

1. **Fractions and percentages quick check** (Mathematics): 8 multiple-choice questions, answer
   key hidden.
2. **Cells and energy review** (Science): multiple choice, one Matching and one Short answer
   question, answer key hidden.
3. **World capitals warm-up** (Geography): 5 questions, answer key shown.

All three are licensed CC BY 4.0. Then, still as `b@example.test`, it makes a circle, sends two
banks into it the way **Share to a circle** does (an own-work confirmation and a licence on each),
and makes two invite codes:

- Circle **Maths department**, owner Sample Teacher B.
  - **Linear equations starter** (6 questions, CC BY 4.0: members may share it publicly with credit).
  - **Angles exit ticket** (4 questions, All rights reserved: members may not share it publicly).
- One **editor** code (can add banks) and one **viewer** code (can only look and copy). Each works
  once, for 7 days. They are printed at the end, for example:

  ```
  Invite codes for the circle "Maths department" (each works once, for 7 days):
    editor (can add banks):          C-XXXX-XXXX-XXXX-XXXX-XXXX-XXXX
    viewer (can only look and copy): C-XXXX-XXXX-XXXX-XXXX-XXXX-XXXX
  ```

  Circle codes are not passwords; they are meant to be passed on. Redeem one as `a@example.test` in
  the app: **Shared**, **Circles** tab, **Enter a code...**, paste it, **Look up**, **Join**.

The script then lists what it made and signs out. The account's password is made up inside the
script and never shown or saved, so nobody can sign in as `b@example.test`. Use other addresses for
your own test accounts.

Running `rig seed` again publishes nothing public again. It takes `b@example.test` back with a
password-reset link from the mail log (again with a made-up password that is never shown), makes
the circle and its banks only if they are missing, withdraws the codes an earlier seed made that
nobody used, and prints two fresh ones. A reset link goes out at most once every 2 minutes (and
10 times an hour), so a second run within 2 minutes stops with a message saying so; wait and run
it again. `rig reset`
empties everything (and then `a@example.test` has to be made again, as in step 3).

## 2. Launch the app against the local server

Build and run a debug build with the local address built in:

```sh
cd <repo root>
VERSHION_POOL_URL=http://localhost:18791 cargo run --bin vershion
```

The app then talks only to `http://localhost:18791`. There is no server address box and no
"Send your password to this server?" question. To open the app with the Account window already
showing, add `VERSHION_SCREEN=account` in front of the command. `VERSHION_SCREEN=shared:circles`
opens the Shared page on the Circles tab (also `shared:circles:join`, `shared:circles:create`,
`shared:circle:<circle id>`, and `publish:circle` for the share window with a circle chosen).

Go to **Settings** in the left navigation. Under **Sharing account**, click **Open account…**.

## 3. Account A: sign up, confirm, approve

1. Click **Create an account**. Fill in a made-up address (for example `a@example.test`; nothing
   is actually sent to it), a password of at least 10 characters, a display name and a school,
   tick **I am an adult employee of a school**, and click **Create account**.
2. The window changes to **Check your email**. In a terminal, run
   `scripts/local-ui-rig.sh links a@example.test` and copy the URL on the `confirm` line.
3. Paste it into **Link or code from the email** and click **Continue**.
4. On **Finish your account**, set the password (typed twice), the name and the school, tick the
   box, and click **Confirm**. These replace what was typed at sign-up.
5. Sign in with the address and password. The window should show **Waiting for approval**, with
   an **Invite code** section below it.
6. Run `scripts/local-ui-rig.sh approve a@example.test`, then click **Check now**. The window
   should now say **Approved**, with **Browse shared questions** and **My shared items** buttons.

## 4. Account B, for import and merge tests

Account B runs in a second copy of the app with its own empty data folder:

```sh
VERSHION_STORE_DIR=/tmp/vershion-b VERSHION_POOL_URL=http://localhost:18791 cargo run --bin vershion
```

If running two copies at once is awkward, you can sign A out and sign B in within one app instead.
`rig seed` already uses `b@example.test`, so give your B another address, for example
`b2@example.test`, and use that address in the commands below.

Account B is the one to use for the invite-code tests:

1. Sign up as `b2@example.test`, confirm with `rig links b2@example.test`, and sign in, as in step 3.
2. **Before approval:** run `rig invite` and type the code into **Invite code**, then click
   **Redeem**. It should say **Redeemed. You can use the shared pool.** The status should still be
   **Waiting for approval**.
3. Run `rig approve b2@example.test` (B already has a redemption, so this only approves it), then
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

### Circles

Use `a@example.test` (step 3) and, for the owner-side tests, a second account in a second copy of
the app (`b2@example.test`, step 4). Circles are on the **Shared** page, **Circles** tab.

**Join the seeded circle (A)**
- [ ] With no circle yet, the tab says **You are not in any circle yet** under **Create a circle...**
      and **Enter a code...**.
- [ ] **Enter a code...**: paste the whole printed line (or a sentence around the code). It should
      say **Found a code: C-...**. Click **Look up**: it should name **Maths department**, the owner
      (Sample Teacher B) and the role, and say nothing is used up until you press **Join**.
- [ ] Click **Join**, then **Open the circle**. The circle page shows the role pill (**Editor** for
      the editor code), the two banks with their licence line, the members, and **What happened
      lately** (B made the circle, shared two banks, you joined).
- [ ] Look up the same code again: as you are already in, it says **You are already in "Maths
      department"** with **Open it**. For anyone else it is refused (each seed code works once). A
      made-up code such as `C-AAAA-BBBB-CCCC-DDDD-EEEE-FFFF` is refused as not valid; text with no code
      in it says **No circle code found yet** and **Look up** stays greyed out.

**Banks and copies (A)**
- [ ] **Open** a bank: it shows the questions without answers and says copies include the answer keys.
- [ ] **Get a copy...**, change the name if you like, **Get the copy**. It should say **Done** and
      the copy should appear under **Banks** and under **Things I imported** (with **Open in the
      circle**).
- [ ] **Report this bank...**: the form opens below; **Cancel** closes it again.
- [ ] As a viewer (redeem the viewer code with a third account), there is no **Add a bank...**, and
      **Share to a circle** says you are not in a circle you can add banks to.

**Share to a circle (A as editor, or any owner)**
- [ ] On a bank, **Share to pool...**, then **One of my circles**. Choose the circle (preselected if
      it is the only one), pick a licence (nothing is preselected), tick **Yes, I confirm this**, and
      click **Share to the circle**. It should say the bank is now in the circle, with **Open the
      circle**. The new bank shows up on the circle page and in **What happened lately**.
- [ ] **Add a bank...** on the circle page opens the same window with that circle chosen.
- [ ] **D13:** with a copy of **Angles exit ticket** (All rights reserved), try **Share to pool...**
      to the public pool. It should stop at **Needs your decision**, naming the circle bank, and say a
      written statement can not lift it for a bank you can still read. Nothing is published.

**Owner side (A makes a circle, B2 joins)**
- [ ] **Create a circle...**: a name is required (80 characters at most); **Create the circle** opens
      it with you as **Owner**.
- [ ] **Make a code...**: choose what they can do, how many people and days, a note. **Make the
      code** shows the code once (**Copy the code**, then **I have copied it**). The **Codes** list
      shows it as **Active**, never the code itself. **Withdraw** stops it.
- [ ] As B2, join with the code. As A, B2 appears under **Members**; **Make viewer** / **Make
      editor** changes B2's role; **Remove...** asks first.
- [ ] **Offer ownership...** to B2: A sees **Ownership offer pending** with **Withdraw the offer**;
      B2 sees **You were offered ownership** (also as a pill in the list) with **Accept ownership** and
      **Decline**. After accepting, B2 is the owner and A an editor.
- [ ] **Close this circle...**: everyone sees a red banner saying access ends in 14 days. The owner
      can **Reopen the circle** until then.
- [ ] **Leave this circle...** as a member, and as the owner (the circle passes to the member who has
      been there longest, or closes if nobody is left).

**Circle errors**
- [ ] Owning 5 circles (closed ones count for 14 days): **Create the circle** is refused with a
      plain message.
- [ ] `rig stop` while the Circles tab is open: the next action shows **Could not reach the pool**
      with **Try again**, not a hang. `rig start` and **Try again** works.
- [ ] After **Sign out**, the Circles tab asks you to sign in again; signing back in returns to it.

## 6. Clean up

```sh
scripts/local-ui-rig.sh stop     # keep the accounts for another session
scripts/local-ui-rig.sh reset    # or: delete everything (accounts, items, the throwaway keys, the mail log)
```

The app remembers the server address and your sign-in in its own settings and the system keychain.
Sign out (or click **Use a different server**) before pointing it at a real server.
