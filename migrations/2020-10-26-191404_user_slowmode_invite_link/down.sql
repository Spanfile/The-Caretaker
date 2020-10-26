CREATE TYPE module_kind_new AS ENUM (
    'mass_ping',
    'crosspost',
    'dynamic_slowmode',
    'emoji_spam',
    'mention_spam',
    'selfbot'
);

DELETE FROM module_settings WHERE module = 'user_slowmode' OR module = 'invite_link';
DELETE FROM actions WHERE module = 'user_slowmode' OR module = 'invite_link';

ALTER TABLE module_settings ALTER COLUMN module TYPE module_kind_new USING (module::text::module_kind_new);
ALTER TABLE actions ALTER COLUMN module TYPE module_kind_new USING (module::text::module_kind_new);

DROP TYPE module_kind;
ALTER TYPE module_kind_new RENAME TO module_kind;
