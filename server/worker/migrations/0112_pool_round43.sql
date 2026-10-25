-- Round 43 (circles phase C, docs/circles-plan.md D12): "hide keys from viewers". `circle_banks.hide_key`: the bank's
-- author (while an editor or the owner) or the circle's owner chose to serve the bank's VIEWERS share-core's stripped
-- form of every version (the stored bodies are not changed; editors and the owner read the full text).
-- `circle_versions.strip_opts`: the strip decisions recorded for that version (JSON {labels, order_acks,
-- ack_formatting, image_ok, image_shas}: the author's label decisions, preserved-order acknowledgements, low-risk
-- formatting acknowledgements, the positions whose image the viewers' copy keeps and those images' hashes), written
-- by a send to a bank that hides its keys, by the PATCH that turns it on (for the head) and copied by a rollback;
-- NULL: none recorded (the version is stripped with none, and refused to viewers where the gate needs one).
ALTER TABLE circle_banks ADD COLUMN hide_key INTEGER NOT NULL DEFAULT 0 CHECK (hide_key IN (0, 1));
ALTER TABLE circle_versions ADD COLUMN strip_opts TEXT;
-- Two columns with defaults; no existing row changes. The next is 0113.
