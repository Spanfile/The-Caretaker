CREATE TYPE module_kind AS ENUM (
    'mass_ping',
    'crosspost',
    'dynamic_slowmode',
    'emoji_spam',
    'mention_spam',
    'selfbot'
);

CREATE TYPE action_kind AS ENUM (
    'remove_message',
    'notify'
);

CREATE TABLE "module_settings" (
    "guild" BIGINT NOT NULL,
    "module" module_kind NOT NULL,
    "enabled" BOOLEAN NOT NULL,
    PRIMARY KEY ("guild", "module")
);

CREATE TABLE "actions" (
    "id" SERIAL PRIMARY KEY,
    "guild" BIGINT NOT NULL,
    "module" module_kind NOT NULL,
    "action" action_kind NOT NULL,
    "in_channel" BIGINT,
    "message" TEXT
);
