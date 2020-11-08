CREATE TYPE module_kind_new AS ENUM (
    'mass_ping',
    'crosspost',
    'emoji_spam',
    'mention_spam',
    'selfbot',
    'invite_link',
    'channel_activity',
    'user_activity'
);

DELETE FROM module_settings WHERE module = 'user_slowmode' OR module = 'dynamic_slowmode';
DELETE FROM actions WHERE module = 'user_slowmode' OR module = 'dynamic_slowmode';
DELETE FROM modules WHERE module = 'user_slowmode' OR module = 'dynamic_slowmode';

ALTER TABLE module_settings ALTER COLUMN module TYPE module_kind_new USING (module::text::module_kind_new);
ALTER TABLE actions ALTER COLUMN module TYPE module_kind_new USING (module::text::module_kind_new);
ALTER TABLE modules ALTER COLUMN module TYPE module_kind_new USING (module::text::module_kind_new);

DROP TYPE module_kind;
ALTER TYPE module_kind_new RENAME TO module_kind;
