CREATE TABLE "actions" (
    "id" SERIAL PRIMARY KEY,
    "guild" BIGINT NOT NULL,
    "module" TEXT NOT NULL,
    "action" TEXT NOT NULL,
    "in_channel" BIGINT,
    "message" TEXT
);
