# Production backups and restore

## What runs
`vershion-backup.timer` runs `/usr/local/sbin/vershion-backup` at 03:15 UTC every night (a copy of the script is `server/api/staging/vershion-backup`).

1. A consistent copy of `/var/lib/vershion/pool.db` (sqlite backup API, integrity-checked) and a tarball of `/var/lib/vershion/blobs`, kept on the VPS in `/var/backups/vershion` (newest 14). Same disk as the live data, so this only covers mistakes, not a lost server.
2. The same two files encrypted with `age` and uploaded to the private Cloudflare R2 bucket `vershion-backups` (newest 30 of each kept). A failed upload makes the systemd unit fail: check `systemctl is-failed vershion-backup.service` and `journalctl -u vershion-backup.service`.

## Where the secrets are
- `/etc/vershion/backup-r2.env` (root, 600): R2 account id, bucket, and an access key limited to that one bucket (Object Read & Write). Token name in Cloudflare: `vershion-backup-writer (bucket vershion-backups only)`. To rotate: create a new one in R2 > Manage API tokens, replace the file, delete the old token.
- `/etc/vershion/backup-age.pub`: the age PUBLIC key. Encrypting needs only this.
- The age PRIVATE key is not on the server. The owner holds it at `~/.config/vershion/backup_age_key`; keep a copy offline together with `POOL_HASH_KEY`. Without it the off-site copies cannot be read.
- A restore also needs `POOL_HASH_KEY` (it is not in the backup), or the pool's hashes cannot be recomputed.

## Restore
1. Download the newest `pool-*.db.age` and `blobs-*.tar.gz.age` from the R2 bucket (dashboard, or `rclone` with the same endpoint settings the script uses).
2. `age -d -i backup_age_key pool-....db.age > pool.db` (and the same for the blobs tarball), then `sqlite3 pool.db "pragma integrity_check"` must print `ok`.
3. Stop `vershion-api`, put `pool.db` at `/var/lib/vershion/pool.db` (remove any `-wal`/`-shm` files next to it) and untar the blobs into `/var/lib/vershion/blobs`, start `vershion-api`.

Restore tested 2026-11-07: the off-site copy decrypted with the owner's key, passed `integrity_check`, and had the same user count as the live database.
