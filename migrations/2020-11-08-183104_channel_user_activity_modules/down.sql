CREATE TYPE module_kind_new AS ENUM (
    'mass_ping',
    'crosspost',
    'dynamic_slowmode',
    'user_slowmode',
    'emoji_spam',
    'mention_spam',
    'selfbot',
    'invite_link'
);

DELETE FROM module_settings WHERE module = 'channel_activity' OR module = 'user_activity';
DELETE FROM actions WHERE module = 'channel_activity' OR module = 'user_activity';
DELETE FROM modules WHERE module = 'channel_activity' OR module = 'user_activity';

ALTER TABLE module_settings ALTER COLUMN module TYPE module_kind_new USING (module::text::module_kind_new);
ALTER TABLE actions ALTER COLUMN module TYPE module_kind_new USING (module::text::module_kind_new);
ALTER TABLE modules ALTER COLUMN module TYPE module_kind_new USING (module::text::module_kind_new);

DROP TYPE module_kind;
ALTER TYPE module_kind_new RENAME TO module_kind;
