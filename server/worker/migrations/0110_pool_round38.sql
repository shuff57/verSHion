-- Round 38 (circles phase B, critics 45a/45b B1): confirmed student data is a property of a VERSION, not of a bank.
-- A version whose questions matched a confirmed student-data block at its commit (exactly or as a near-rewording)
-- is withheld from every member at every read (the version read, its history line's text, a rollback to it, a merge
-- from it, its images) until an administrator releases that version; a later clean version never lifts it. NULL: not
-- withheld.
ALTER TABLE circle_versions ADD COLUMN withheld_at INTEGER;
-- Round 39 (critic 46b N1): each version's listing fields (JSON {title, subject, course, grade}), so the bank's
-- listing always comes from its newest version that is not withheld, and a rollback restores the target's. NULL for
-- versions sent before this migration; each bank's current version takes its bank's fields now, unless it is withheld.
ALTER TABLE circle_versions ADD COLUMN listing TEXT;
UPDATE circle_versions SET listing = (SELECT json_object('title', b.title, 'subject', b.subject, 'course', b.course, 'grade', b.grade) FROM circle_banks b WHERE b.id = circle_versions.bank_id)
 WHERE status = 'current' AND withheld_at IS NULL;
-- Round 41 (critic 48 V4): a bank whose current version is withheld (only on a database that ran an earlier edit of
-- this migration) lists nothing until its next version; its fields could have come from that version.
UPDATE circle_banks SET title = '', subject = '', course = '', grade = ''
 WHERE head_version_id IN (SELECT id FROM circle_versions WHERE status = 'current' AND withheld_at IS NOT NULL);
-- Columns added, one backfill of the current versions, one blanking; no table rebuild, no other row changes. The next
-- is 0111.
