-- Round 47 (circles phase C, critic 54b N1): the withheld versions of a circle, for the evidence bound every commit and
-- answer-key change keeps (circlesend.ts evidenceBound). A partial index: only withheld rows whose body is still kept, so
-- it stays small. Round 48 (critics 55a, 55b V6; edited in place, not applied anywhere yet): `IF NOT EXISTS`, and the
-- status in the key and the kept body in the filter, so the rows pruned over time are not read at every commit.
CREATE INDEX IF NOT EXISTS circle_versions_withheld ON circle_versions (circle_id, status) WHERE withheld_at IS NOT NULL AND full_key <> '';
-- The next is 0115.
